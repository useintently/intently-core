## 1) Wireframe completo da IDE (visão ideal)

### 1.1 Layout principal (modo Governança — default)

O usuário abre o projeto e vê o sistema, não arquivos.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top Bar: [Projeto] [Branch] [Modo: Governança|Implementação|Auditoria]        │
│          [Run Evidence] [Open PR] [Record Decision] [Ask LLM]                │
└──────────────────────────────────────────────────────────────────────────────┘

┌───────────────┬───────────────────────────────────────────┬──────────────────┐
│ Navigator      │ System Twin (Grafo Vivo)                 │ Impact & Risk     │
│ - Fluxos       │ - componentes/serviços                   │ - O que mudou?    │
│ - APIs         │ - dependências                           │ - Fluxos afetados │
│ - Dados        │ - fronteiras                             │ - APIs alteradas  │
│ - Políticas    │ - eventos/filas                          │ - Risco ↑/↓       │
│ - Evidências   │ - estados (state machines)               │ - Políticas ok?   │
│ - Decisões     │                                           │ - Contratos ok?   │
└───────────────┴───────────────────────────────────────────┴──────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Evidence Panel (gate de confiança)                                            │
│ ✅ Contratos: 18/18  ✅ Policies: 32/32  ✅ Tests: 120/120  ⚠ Perf: 1 regress  │
│  - Clique para ver “por quê” em linguagem humana + links para artefatos       │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Timeline (o filme do sistema)                                                 │
│  - Mudança #481 (LLM): “Refatorou payments” → Impacto: 2 fluxos, 1 policy     │
│  - Mudança #482 (Humano): “Ajustou invariantes de cancelamento”               │
│  - Gate: Aprovado por evidência + decisão registrada                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Painel “Ask LLM” (mas sem virar chat infinito)

O chat existe, porém acoplado a intenções e ações seguras:

* **Botões de ação**: “Propor mudança”, “Gerar testes”, “Explicar impacto”, “Sugerir contratos”
* **Respostas sempre com evidência**: “Fiz X, aqui estão os diffs semânticos + testes”

### 1.3 Visualizações essenciais (o humano revisa isso)

**A) Diff Semântico (não textual)**

* APIs: adicionou/removeu/alterou payload
* Fluxos: estados novos, transições removidas
* Dados: campos sensíveis tocados
* Permissões: mudou escopo? mudou auth?
* Custos: novas dependências externas? nova fan-out?

**B) Fluxo Crítico como State Machine**

* Uma view estilo “pedido → pagamento → envio → cancelamento”
* O humano enxerga “onde pode dar ruim”

**C) Mapa de Dados**

* origem → transformação → persistência → consumo
* marca PII / segredo / tokens

### 1.4 Modo Implementação (quando precisa ver código)

Um modo “VS Code-like”, mas com guias no topo:

* “Você está editando algo que impacta 2 invariantes e 1 policy”
* “Ao salvar, vou rodar evidência incremental”

### 1.5 Modo Auditoria (para produção e compliance)

* “Quais decisões liberaram isso?”
* “Quais evidências existiam no momento do merge?”
* “Qual policy foi relaxada e por quê?”

---

## 2) DSL mínima do Programa de Intenção (MVP realista)

A DSL tem que ser:

* **pequena**
* **fácil de versionar**
* **capaz de bloquear coisas perigosas**
* **capaz de gerar testes/evidência**

Sugestão: YAML com 6 blocos.

```yaml
# intent.yaml (MVP)
system:
  name: "Loved CRM Payments"
  domain: "crm"
  critical_flows: ["checkout", "refund", "subscription_renewal"]

interfaces:
  api:
    - name: "POST /v1/checkout"
      auth: ["user"]
      input_schema: "CheckoutRequest@v1"
      output_schema: "CheckoutResponse@v1"
      slo:
        p95_ms: 300
  events:
    - name: "payment.succeeded"
      schema: "PaymentSucceeded@v1"

data:
  entities:
    - name: "Payment"
      fields:
        - {name: "id", type: "uuid"}
        - {name: "user_id", type: "uuid", pii: false}
        - {name: "card_last4", type: "string", pii: true, storage: "forbidden"} # exemplo

invariants:
  - id: "INV-001"
    description: "Nenhuma cobrança duplicada para o mesmo idempotency_key"
    scope: ["POST /v1/checkout"]
  - id: "INV-002"
    description: "Refund nunca pode exceder total pago"
    scope: ["refund"]

policies:
  security:
    - id: "SEC-001"
      rule: "all_endpoints_must_require_auth"
    - id: "SEC-002"
      rule: "no_pii_in_logs"
  reliability:
    - id: "REL-001"
      rule: "external_calls_must_have_timeouts"
  architecture:
    - id: "ARC-001"
      rule: "payments_must_not_depend_on_marketing_service"

evidence:
  required:
    - "contract_tests:interfaces.api"
    - "property_tests:invariants"
    - "integration_tests:critical_flows"
  budgets:
    coverage_min: 0.75
    perf_regression_p95_max_ms: 30
```

### Por que essa DSL é “mínima e suficiente”

* Interfaces definem superfície (API/eventos)
* Invariants e policies definem limites
* Evidence define **o que é “passar”**
* Data define onde PII pode aparecer

A LLM é livre pra gerar qualquer coisa **desde que passe nisso**.

---

## 3) Arquitetura técnica da IDE (Engine + UI + LLM Sandbox)

### 3.1 Componentes (alto nível)

1. **UI Desktop/Web**

* renderer do System Twin
* diff semântico
* painel de evidência
* comandos de ação (LLM)

2. **Intent Engine**

* parser/validador do `intent.yaml`
* grafo canônico do sistema (IR = intermediate representation)
* comparação de IR antes/depois (diff semântico)

3. **Evidence Engine**

* runner incremental (só o que foi afetado)
* testes (unit/contract/integration/property)
* análise estática (lint, typecheck, SAST leve)
* performance smoke (micro-bench / endpoint p95)

4. **Policy Engine**

* avalia regras do bloco `policies`
* gera “violations” explicáveis

5. **LLM Orchestrator**

* recebe “tarefas” (não chat solto)
* aplica mudanças via patch
* exige justificativa/plan antes de tocar arquivos
* registra tudo (trilha de auditoria)

6. **Sandbox Runner**

* executa código gerado em ambiente controlado
* limita rede, disco, secrets
* fornece tool APIs (test runner, build, AST extract)

7. **System Twin Store**

* armazena IR + histórico (timeline)
* cada commit vira um snapshot do “sistema visto por cima”

### 3.2 Fluxo interno (a cada mudança)

1. Mudança no repo (LLM/humano)
2. Engine extrai IR (AST, OpenAPI/AsyncAPI se existir, dependências)
3. Diff semântico IR_old vs IR_new
4. Policy engine avalia regras
5. Evidence engine roda suíte incremental
6. UI atualiza: impacto + evidência + risco
7. Gate aprova/reprova

---

## 4) Como integrar com Git / CI sem quebrar tudo

O objetivo é **não reinventar Git**. A IDE só muda a forma de revisar.

### 4.1 Branch/PR continuam iguais

* dev trabalha em branch
* abre PR no GitHub/GitLab
* CI roda como sempre

### 4.2 O que muda: “Required Checks”

Você adiciona 1 check obrigatório no CI:

* `intent-check` (valida DSL + gera IR)
* `semantic-diff-check` (gera relatório semântico e publica artefato)
* `policy-check` (falha se policy violada)
* `evidence-check` (falha se evidência mínima não passou)

O PR ganha um comentário automático com:

* “impacto resumido”
* “policies ok/violadas”
* “invariants cobertos/não cobertos”
* links para relatórios

### 4.3 Artefatos de CI (fundamental)

CI publica:

* `system_twin.json` (IR)
* `semantic_diff.json`
* `evidence_report.json`
* `policy_report.json`

A IDE lê isso e mostra bonito.

### 4.4 Evitar travar o time (realismo)

* Evidence incremental por padrão
* Full suite só em merge para main/release
* Possibilidade de “exceção auditada” (override com justificativa)

---

## 5) Estratégia de adoção incremental (como isso entra no mundo real)

Sim: **começa como plugin**. Se tentar substituir IDE + Git + CI de cara, morre.

### Fase 0 — “Produto invisível” (1–2 semanas)

**GitHub App / CI Action**

* lê `intent.yaml`
* posta diff semântico e policy report no PR
* sem IDE nova ainda

Ganha valor rápido: **revisão por impacto** no próprio PR.

### Fase 1 — Plugin para VS Code / Cursor (MVP de IDE)

O plugin só faz:

* render do System Twin
* painel de evidência
* botões de “Ask LLM” (tarefas seguras)
* link pro PR/CI

Usuário continua no editor que ama.

### Fase 2 — Desktop IDE “modo governança”

Você lança uma IDE própria, mas:

* ela vira o “cockpit”
* o editor de código pode ser embutido (Monaco) ou abrir VS Code

### Fase 3 — “Intenção como fonte da verdade”

Times maduros passam a:

* criar features primeiro no `intent.yaml`
* gerar tarefas/testes a partir dele
* deixar a LLM implementar

---

## 6) O que torna essa IDE “ideal para humanos limitados”

Ela não pede atenção em linhas de código.

Ela responde perguntas humanas:

* “isso muda o quê?”
* “isso quebra o quê?”
* “por que eu devo confiar?”
* “qual risco eu estou aceitando?”
* “qual evidência garante isso?”

E ela faz a LLM operar num trilho:

* gerar → explicar impacto → provar com evidência → registrar decisão

---
