# Intently — Product Requirements Document v2

## Intent-Driven Development IDE

**Versão:** 2.0
**Data:** 2026-02-27
**Status:** Ideação

---

## 1. Visão do Produto

LLMs tornaram a geração de código trivial. Ferramentas como Claude Code, Codex, Cursor e Windsurf produzem centenas de linhas por minuto. Mas IDEs, Git e CI continuam operando no paradigma do código-fonte — arquivos, linhas, diffs textuais. O gargalo migrou de "escrever código" para "compreender, validar e governar mudanças".

**Intently propõe um novo paradigma:**

> Desenvolvedores não lêem código gerado por IA.
> Eles declaram intenção, revisam impacto, e governam com evidência.
> O código é artefato derivado. A intenção é a fonte da verdade.

Intently é uma **IDE analítica e proativa para desenvolvimento orientado a intenção**. Funciona como copiloto de ferramentas de geração de código baseadas em VSCode. O dev usa a ferramenta que preferir para gerar código. O Intently é onde ele vê, revisa, automatiza e governa — sem nunca precisar ler código.

### 1.1 Analogia

O Claude Code é o piloto automático que voa o avião. O Intently é o painel de instrumentos — altitude, velocidade, rota, alertas — e o comandante que decide se aceita o que o autopilot está fazendo ou intervém.

### 1.2 Posicionamento

O Intently não compete com ferramentas de geração de código. Ele senta ao lado delas. É a camada que falta entre "a IA gerou código" e "é seguro mergear".

---

## 2. Problema

### 2.1 O que mudou

LLMs alteraram fundamentalmente como código é produzido. A geração é instantânea. O volume e velocidade de mudanças ultrapassaram a capacidade de revisão humana.

### 2.2 O que não mudou

As ferramentas continuam centradas em código-fonte. IDEs mostram arquivos e linhas. Git mostra diffs textuais. CI roda testes sem contexto semântico. Code review opera no nível errado de abstração.

### 2.3 Problemas concretos

**A IA não tem memória do sistema.** Cada geração parte do zero. Não tem contexto prévio, não conhece o histórico, não sabe como os módulos se relacionam. É como o protagonista de Memento: acorda sem saber onde está. Enquanto o dev carrega um mapa mental de como tudo se conecta, a IA enxerga apenas uma massa de módulos desconectados.

**Feedback lento.** Se o codebase não oferece ciclos rápidos de feedback (testes, linting, type checking), a IA não consegue saber se o que ela mudou realmente fez o que pretendia. Ela opera no escuro.

**Burnout cognitivo.** Com um codebase desorganizado, o dev precisa compensar as limitações da IA: segurar na cabeça todas as relações entre módulos, revisar manualmente cada mudança, colar os pedaços. Um PR de 800 linhas gerado por LLM onde o dev precisa decidir se é seguro mergear.

**IDEs inadequadas.** Ferramentas atuais foram feitas para humanos escrevendo código, não para humanos governando IA que escreve código. A abstração de "arquivo/linha" é o nível errado quando o artefato primário passa a ser intenção.

**Shallow modules everywhere.** A maioria dos codebases é composta por muitos módulos pequenos e rasos, altamente interconectados. Qualquer mudança pode afetar qualquer lugar. É difícil testar unidades isoladas. É impossível manter tudo na cabeça.

### 2.4 Hipótese central

> O erro não está na LLM nem no humano — está na ausência de infraestrutura para o paradigma da intenção. As ferramentas foram construídas para o paradigma do código. Não existe infraestrutura para declarar intenção, visualizar impacto semântico, e governar com evidência.

### 2.5 Referências e fundamentação

Esta visão é sustentada por pesquisa e prática emergente:

- **Matt Pocock (AI Hero):** O codebase é a maior influência no output da IA. "Deep modules" com interfaces simples + implementação rica delegada à IA são o futuro. O humano cuida do design das interfaces; a IA cuida da implementação.
- **Intent-Driven Development (copyleftdev):** É possível especificar um sistema inteiro (122 eventos, 8 RFCs, 82 issues) sem escrever código. O projeto vira um knowledge base navegável por humanos e IA.
- **Ply (UC Berkeley, UIST'25):** Trigger-action programming com LLMs funciona quando o usuário decompõe intenção em layers. Cada layer tem visualização e parametrização gerada. O código é detalhe que o usuário nunca precisa ver.
- **MvTAP (Wu et al.):** Intenção do usuário é multi-dimensional (user view, developer view, knowledge view). Capturar intenção implícita — não só explícita — é o salto de qualidade.
- **Bollikonda & Kovi (2025):** Estamos na transição de "code generation" para "intent-driven synthesis". O modelo mental muda de "a IA escreve código para mim" para "a IA materializa minha intenção em software".

---

## 3. Objetivo

Criar uma IDE de alto nível que permita:

1. Desenvolvedores declararem intenção antes de gerar código (**Modo Intenção/Plan**)
2. Visualizar analiticamente o impacto de mudanças sem ler código (**Observabilidade de Desenvolvimento**)
3. Automatizar respostas a mudanças via triggers proativos (**Governance Triggers**)
4. LLMs operarem com alta velocidade enquanto humanos governam em nível de sistema

**Princípio norteador:** diminuir carga cognitiva do dev. O Intently é proativo, não reativo.

---

## 4. Usuários

### 4.1 ICP Primário

**Tech Lead / Staff Engineer / Product Engineer** em empresa de médio-grande porte (50–500 engenheiros) que já usa LLMs para gerar código e sente que as ferramentas atuais (VSCode + Git + PR review) não foram feitas para esse workflow. Pensam em nível de sistema — interfaces, fluxos, invariantes — e querem uma ferramenta que opere nesse nível.

**Trigger de compra:** O time começou a usar LLMs e o volume de mudanças ultrapassou a capacidade de revisão. Ou: um incidente em produção foi causado por mudança que "passou no review" mas violava uma regra implícita.

### 4.2 ICP Secundário

**Vibe Coder / Engenheiro de Produto** que consegue descrever o que um sistema deve fazer mas não quer gerenciar código linha por linha. O humano cuida do design das interfaces e conexões; a IA cuida da implementação. Essa pessoa hoje não tem ferramenta.

### 4.3 ICP Terciário

**Times inteiros** que querem adotar Intent-Driven Development como metodologia, onde `intent.yaml` é o artefato versionado, o código é gerado/validado automaticamente, e o review acontece no nível de intenção e evidência — nunca no nível de diff textual.

### 4.4 Persona compradora

VP/Director of Engineering ou Head of Platform. Preocupações: velocity sem sacrificar segurança, compliance auditável, redução de incidentes causados por mudanças implícitas.

---

## 5. Os Três Pilares

### Pilar 1 — Modo Intenção/Plan

O dev declara o que quer mudar no sistema. O Intently gera um preview de impacto mostrando mudanças nos artefatos (As Is → To Be) com loopback até aprovação. Depois, gera um roadmap de implementação — tasks estruturadas que as LLMs executam.

#### Fluxo

```
1. Dev declara intenção (natural language ou DSL)
   "Adicionar suporte a cancelamento no fluxo de checkout com reembolso proporcional"

2. Intently consulta System Twin (estado atual) e gera preview
   ┌─────────────────────────────────────────────────┐
   │  As Is                                          │
   │  Flow: checkout [cart → pending → paid → done]  │
   │  APIs: POST /v1/checkout (schema v2)            │
   │  Invariants: INV-001 (sem cobrança duplicada)   │
   ├─────────────────────────────────────────────────┤
   │  To Be                                          │
   │  Flow: checkout [...→ paid → refund_req → done] │
   │  APIs: + POST /v1/refund (novo)                 │
   │  Invariants: + INV-003 (refund ≤ total pago)    │
   ├─────────────────────────────────────────────────┤
   │  Delta                                          │
   │  +1 API, +2 estados, +1 invariante              │
   │  1 policy impactada (REL-002 idempotência)      │
   │  0 breaking changes                             │
   └─────────────────────────────────────────────────┘

3. Dev revisa, ajusta e aprova (loopback se necessário)

4. Intently gera roadmap de implementação
   Step 1: Criar endpoint POST /v1/refund       [llm_task]
   Step 2: Adicionar idempotência ao endpoint    [autofix_patch]
   Step 3: Gerar property test INV-003           [llm_task]
   Step 4: Rodar evidence incremental            [run_evidence]

5. Tasks executadas por Claude Code / Codex via VSCode

6. Observabilidade atualiza em tempo real a cada step
```

#### Princípios

- A intenção não precisa ser rígida — ela evolui durante o processo
- O preview é um loopback: o dev itera As Is → To Be até estar satisfeito
- O roadmap tem alta precisão porque parte de artefatos estruturados (System Twin + intent.yaml), não de prompt vago
- O código é consequência da intenção, não o contrário
- Segue o ciclo: Definir Intenção → Arquitetar → Operacionalizar → Implementar → Validar

### Pilar 2 — Observabilidade de Desenvolvimento (DevObs)

Conceitos de SRE aplicados ao ciclo de desenvolvimento. O Intently monitora a saúde do sistema em tempo real usando indicadores, objetivos e alertas — assim como um time de SRE monitora produção.

#### Analogia SRE → DevObs

| Conceito SRE | Equivalente Intently | Descrição |
|---|---|---|
| SLI (Service Level Indicator) | **DLI** — Development Level Indicator | Policy compliance %, evidence coverage %, drift score |
| SLO (Service Level Objective) | **DLO** — Development Level Objective | "Policies green antes de merge", "Coverage > 80% em critical flows" |
| Alert | **Trigger** | Quando DLI cai abaixo do DLO, Intently age proativamente |
| Dashboard | **System Cockpit** | Saúde do sistema em tempo real |
| Runbook | **Action Plan** | O que fazer quando um trigger dispara |
| Incident | **Governance Debt** | Override ativo, evidência faltante, policy relaxada |

#### Development Level Indicators (DLIs)

- **Policy Compliance Score** — % de policies satisfeitas por categoria (SEC, REL, ARC, PERF)
- **Evidence Coverage** — % de invariantes com evidência executável
- **Architectural Drift** — desvio do sistema real em relação à intenção declarada
- **Risk Score** — score composto (segurança × confiabilidade × arquitetura × performance)
- **Governance Debt** — overrides ativos + evidências faltantes + policies relaxadas
- **Intent Freshness** — tempo desde última atualização do intent.yaml vs. mudanças no código

#### Development Level Objectives (DLOs)

- Configuráveis por time, projeto e domínio
- Exemplos: "Zero violations SEC", "Evidence coverage > 90% critical flows", "Max 2 overrides simultâneos"
- Quando DLI cai abaixo do DLO → trigger dispara automaticamente

### Pilar 3 — Governance Triggers

Automações proativas que conectam observabilidade a ação. Quando um indicador muda, o Intently age. É o IFTTT para governança de software.

#### Triggers nativos (built-in)

| Trigger | Ação automática |
|---|---|
| Policy violation detectada | Aplicar patch determinístico ou gerar LLM task |
| Evidência faltante para invariante | Gerar property test via LLM |
| Nova dependência cross-context | Notificar tech lead + sugerir alternativa |
| PII detectada em logs | Aplicar patch de redação |
| Override expirado | Reativar policy + criar task de correção |
| Coverage abaixo do DLO | Gerar testes incrementais |
| API schema breaking change | Bloquear merge + gerar contract tests |
| Intent drift > threshold | Alertar + sugerir sync intent.yaml |

#### Triggers customizáveis

```yaml
# .Intently/triggers.yaml
triggers:
  - name: "Payments module protection"
    when:
      event: "files_changed"
      scope: "src/payments/**"
    then:
      - run_evidence: { scope: "full", target: "payments" }
      - notify: { channel: "slack", team: "payments-team" }

  - name: "API schema change guard"
    when:
      event: "api_schema_changed"
      severity: "breaking"
    then:
      - block_merge: true
      - generate_contract_tests: { target: "affected_apis" }
      - notify: { channel: "slack", user: "tech-lead" }

  - name: "Auto-fix security violations"
    when:
      event: "policy_violation"
      category: "security"
      autofix_available: true
    then:
      - apply_patch: { type: "deterministic" }
      - run_evidence: { scope: "incremental" }

  - name: "Evidence gap filler"
    when:
      event: "dli_below_dlo"
      indicator: "evidence_coverage"
    then:
      - generate_tests: { strategy: "impact_based" }
      - report: { format: "cockpit_update" }
```

#### Princípios dos Triggers

- O Intently é proativo, não reativo. A meta é diminuir carga cognitiva, não adicionar
- Triggers predefinidos cobrem 80% dos casos comuns
- Triggers customizáveis permitem extensibilidade infinita
- Toda ação automatizada é auditável e revertível
- O dev pode override qualquer trigger (com justificativa e expiração)
- Triggers encadeáveis: a saída de um pode ser entrada de outro

---

## 6. Conceitos Fundamentais

### 6.1 Intenção (Intent)

Declaração explícita do que deve ser verdade sobre o sistema. Representada por `intent.yaml`. Contém: serviços, interfaces (APIs, eventos), fluxos, dados, invariantes, policies, requisitos de evidência.

A intenção não é rígida — evolui com o sistema. Cada mudança é versionada e auditável. O intent.yaml é o artefato central do desenvolvimento orientado a intenção, assim como o código-fonte é o artefato central do desenvolvimento tradicional.

```yaml
# intent.yaml (exemplo)
services:
  - name: checkout-service
    framework: express
    language: typescript
    apis:
      - path: /v1/checkout
        methods: [POST]
        invariants:
          - id: INV-001
            description: "Sem cobrança duplicada"
            evidence: property-test
      - path: /v1/refund
        methods: [POST]
        invariants:
          - id: INV-003
            description: "Refund ≤ total pago"
            evidence: property-test

    flows:
      - name: checkout-flow
        states: [cart, pending, paid, refund_requested, refunded, fulfilled]
        transitions:
          - from: cart → to: pending
          - from: pending → to: paid
          - from: paid → to: refund_requested
          - from: refund_requested → to: refunded
          - from: paid → to: fulfilled

    policies:
      - SEC-001  # Auth obrigatório
      - REL-002  # Idempotência em mutações
      - ARC-003  # Sem dependência circular
```

### 6.2 System Twin (IR)

Representação intermediária do sistema como ele é agora. Componentes, dependências, contratos, fluxos, sinks. É o "modelo mental formalizado" — a memória que a IA não tem.

Gerado automaticamente a partir do codebase (AST, OpenAPI, imports, route analysis). Atualizado a cada mudança. O diff semântico é computado entre estados do System Twin.

### 6.3 Semantic Diff

Comparação entre estados do System Twin: o que mudou em comportamento, o que mudou em risco, o que não mudou. Substitui o diff textual como base de revisão.

O dev vê "1 API alterada, 2 fluxos afetados, PII tocada" — não "487 linhas adicionadas em 12 arquivos".

### 6.4 Policies

Regras verificáveis e acionáveis sobre o sistema. Quatro categorias: segurança (SEC), confiabilidade (REL), arquitetura (ARC), performance (PERF).

Cada policy: detecta violação → localiza → sugere fix → pode auto-corrigir. 10 policies iniciais no MVP, extensível via YAML.

### 6.5 Evidence

Testes e validações executáveis que provam: policies satisfeitas, invariantes mantidos, risco controlado. Evidência é obrigatória, incremental e explícita. Selecionada por Impact-Based Test Selection (IBTS).

### 6.6 Skills

Habilidades explícitas que agentes LLM podem executar. Sem skill registrada → ação proibida. Tasks estruturadas com contratos rígidos, não prompts livres. Princípio: "nada implícito".

---

## 7. Arquitetura

### 7.1 Visão Geral

```
┌──────────────────────────────────────────────────────────────┐
│                    VSCode Workspace                          │
│                                                              │
│  ┌─────────────────┐    ┌──────────────────────────────────┐ │
│  │  Editor          │    │  Intently Extension                  │ │
│  │  (Claude Code /  │    │  ┌────────────────────────────┐  │ │
│  │   Cursor /       │    │  │  Modo Intenção/Plan        │  │ │
│  │   Codex)         │    │  │  (declare → preview → go)  │  │ │
│  │                  │    │  ├────────────────────────────┤  │ │
│  │  Gera código     │    │  │  System Cockpit (DevObs)   │  │ │
│  │                  │    │  │  DLIs | DLOs | Debt        │  │ │
│  │                  │    │  ├────────────────────────────┤  │ │
│  │                  │    │  │  Trigger Notifications     │  │ │
│  │                  │    │  │  Actions | Alerts          │  │ │
│  └─────────────────┘    │  └────────────────────────────┘  │ │
│                          └──────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                    Intently Core Engine (Rust)                    │
│                                                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Intent   │ │ System   │ │ Policy   │ │ Evidence       │  │
│  │ Engine   │ │ Twin &   │ │ Engine   │ │ Engine         │  │
│  │          │ │ Sem Diff │ │          │ │ (IBTS)         │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘  │
│                                                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────────────┐ │
│  │ Planner  │ │ Trigger  │ │ LLM Orchestrator             │ │
│  │          │ │ Engine   │ │ (sandbox, skills, anti-loop)  │ │
│  └──────────┘ └──────────┘ └──────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 7.2 Integração com VSCode

Extensão VSCode que adiciona:

- **Painel lateral:** System Cockpit com DLIs, DLOs, governance debt em tempo real
- **Tab dedicada:** Modo Intenção/Plan — declarar intenção, preview As Is → To Be, aprovar roadmap
- **Inline notifications:** Alertas quando DLI cai abaixo de DLO ou trigger dispara
- **Commands:** `Intently: Declare Intent`, `Intently: Show Impact`, `Intently: Run Evidence`, `Intently: Fix Violation`
- **Status bar:** Health score do sistema (traffic light: green/yellow/red)

O dev continua usando Claude Code / Cursor no editor central. O Intently opera no painel ao lado. O dev vê sistema, não código.

### 7.3 Modelo de Integração (evolução)

O Intently começa como Observer e evolui para Orchestrator:

**Fase 1 — Observer:** Monitora git/filesystem. Detecta mudanças post-facto. Gera System Twin e semantic diff. Apresenta analytics no Cockpit. Zero fricção, funciona com qualquer ferramenta.

**Fase 2 — Observer com Contexto:** intent.yaml fornece contexto de intenção. Quando o dev faz commit, o Intently compara resultado contra intenção declarada. Triggers semi-ativos (sugere ações, dev aprova).

**Fase 3 — Orchestrator:** Modo Intenção/Plan ativo. O Intently orquestra o ciclo completo: dev declara intenção → Intently gera preview → dev aprova → Intently distribui tasks → ferramentas executam → Intently valida.

### 7.4 Core Engine

- Rust (CLI + Library)
- Executável em local e CI — mesmas regras, sem surpresas
- Determinístico e auditável
- tree-sitter para parsing multi-linguagem (Python, TypeScript no MVP; Go, Java em V2)
- ast-grep com YAML catalogs para pattern matching extensível

### 7.5 LLM Orchestrator

- Motor de execução de tarefas (não chat)
- Recebe `action_plan.json` do Planner
- Escolhe: patch determinístico (preferido) ou LLM task (fallback)
- Aplica mudanças como patch auditável
- Anti-loop: máximo 2 tentativas por task, depois escala para humano
- Sandbox isolado por task (container, sem rede, sem secrets)
- Typed state entre steps (LangGraph pattern), não conversation history

### 7.6 Trigger Engine

- Avalia condições contra DLIs em tempo real
- Triggers nativos: hardcoded, otimizados
- Triggers custom: interpretados de `.Intently/triggers.yaml`
- Ações: apply_patch, generate_tests, notify, block_merge, run_evidence, report
- Encadeáveis e combináveis
- Audit trail para toda ação executada

---

## 8. Fluxo Principal

```
 ┌─────────────┐
 │ 1. Declare   │  Dev declara intenção no Modo Plan
 │    Intent    │
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 2. Preview   │  Intently gera As Is → To Be com delta
 │    Impact    │  Dev revisa (loopback se necessário)
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 3. Generate  │  Intently gera roadmap de tasks
 │    Roadmap   │  (autofix_patch | llm_task | human_attention)
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 4. Execute   │  Claude Code / Codex executa tasks no VSCode
 │    Tasks     │  (ou Intently Orchestrator executa em sandbox)
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 5. Observe   │  System Twin atualiza. Semantic diff computa.
 │    & Assess  │  DLIs recalculam. Policies avaliam. Evidence verifica.
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 6. Trigger   │  Se DLI < DLO → triggers disparam automaticamente
 │    & Act     │  (gera teste, aplica patch, notifica, bloqueia)
 └──────┬───────┘
        ▼
 ┌─────────────┐
 │ 7. Govern    │  Dev vê Cockpit atualizado
 │              │  Decide: merge / ajustar / override
 └─────────────┘
```

---

## 9. Governança e Overrides

- Overrides são explícitos, temporários e auditáveis
- Override sem expiração → inválido
- Override expirado → trigger dispara automaticamente
- Override gera governance debt (visível no Cockpit como DLI)
- Compensações obrigatórias (tasks de correção associadas ao override)
- Decision Log automático para toda decisão de governança
- O Intently nunca bloqueia unilateralmente — o dev sempre tem a última palavra, mas toda decisão é registrada

---

## 10. Estratégia de Adoção

### Fase 0 — Zero Config (valor imediato)

- Extensão VSCode que monitora repo via git
- Gera System Twin e semantic diff automaticamente (sem intent.yaml)
- Mostra Cockpit com DLIs básicos
- Valor: "O que essa mudança realmente fez ao sistema?"
- Barreira de entrada: instalar extensão

### Fase 1 — Intent Bootstrapped

- CLI `Intently init` gera intent.yaml a partir do repo
- Detecção automática: linguagem, framework, APIs, dependências
- Cada item com confidence tag (high/medium/low)
- Items low confidence comentados por default
- Renovate-style: propõe, dev revisa e commita
- Policies básicas ativadas (SEC-001, REL-001, ARC-001)
- Primeiros triggers nativos

### Fase 2 — Intent Curado

- Dev refina intent.yaml: invariantes, SLOs, bounded contexts
- Modo Intenção/Plan ativo
- Triggers customizáveis (.Intently/triggers.yaml)
- DLOs configurados por time/projeto
- Observabilidade completa
- Re-scan com merge strategy (nunca sobrescreve manual)

### Fase 3 — IDE Cockpit Completa

- App Tauri standalone para sessões de governança profunda (opcional)
- Visualização de grafo do System Twin
- Decision Log completo com auditoria
- Governance Debt dashboard
- Integração CI com PR comments semânticos
- Multi-repo support

---

## 11. Não-Objetivos

O Intently **não**:

- Substitui ferramentas de geração de código (Claude Code, Cursor, Copilot)
- Substitui Git como sistema de versionamento
- Substitui testes tradicionais (complementa com evidence)
- Tenta provar correção formal total
- Tenta entender toda linguagem no dia 1 (MVP: Python + TypeScript)
- É um chat para conversar com LLM sobre código
- Gera código diretamente (quem gera são as ferramentas existentes)
- Toma decisões finais — o dev sempre tem a última palavra

---

## 12. Métricas de Sucesso

**Adoção:**
- Instalações da extensão VSCode
- % repos com intent.yaml commitado
- Triggers customizados criados por time

**Impacto:**
- Redução de tempo médio de review de PRs gerados por LLM
- Redução de regressões causadas por mudanças implícitas
- Triggers que preveniram violação antes do merge
- Aumento de confiança auto-reportada em PRs grandes (survey)

**Engajamento:**
- Frequência de uso do Modo Intenção/Plan
- DLIs monitorados por projeto
- Overrides com expiração cumprida vs. expirada

---

## 13. Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Falso senso de segurança | Alto | Evidence obrigatória, nunca só analytics |
| System Twin impreciso | Alto | Extração pragmática (AST + OpenAPI), progressivamente mais rica |
| Complexidade de adoção | Médio | Fase 0 zero-config, valor imediato sem intent.yaml |
| Overhead de intent.yaml | Médio | Bootstrapper automático, refinamento incremental |
| Lock-in | Médio | Core engine open-source, formatos abertos (YAML/JSON) |
| Resistência cultural | Médio | Adoção incremental, começa como extension VSCode |
| Performance do Core Engine | Baixo | Rust, computação local, cache agressivo |

---

## 14. Roadmap

### MVP (8 semanas)

- Core Engine (Rust CLI): intent parser, System Twin, semantic diff, policy engine (10 policies), evidence engine (IBTS)
- Extensão VSCode: System Cockpit com DLIs, semantic diff viewer, status bar
- `Intently init` bootstrapper com confidence tags
- 8 triggers nativos
- Linguagens: Python (FastAPI) + TypeScript (Express/Node)

### V2 (16 semanas)

- Modo Intenção/Plan completo (As Is → To Be → Roadmap)
- Triggers customizáveis (.Intently/triggers.yaml)
- LLM Orchestrator com sandbox e anti-loop
- Auto-fix patches (12 templates determinísticos)
- DLOs configuráveis por time/projeto
- Integração CI com PR comments semânticos
- Linguagens: + Go, Java

### V3 (24 semanas)

- App Tauri (cockpit standalone)
- Decision Log + auditoria completa
- Governance Debt dashboard
- Flow Studio (state machines executáveis)
- Multi-repo support
- Plugin API para triggers e policies customizados
- Marketplace de policies/triggers da comunidade

---

## 15. Princípio Unificador

> Ferramentas que escalam tratam configuração como dados, não como código.

YAML rules, não query languages proprietárias. Typed state, não conversation history. Declarative triggers, não imperative workflows. Confidence tags, não boolean pass/fail. Graduated intervention, não all-or-nothing.

O Intently internaliza este princípio em cada camada: policies são YAML, triggers são YAML, intent é YAML, patterns são YAML. Dados são mais fáceis de validar, versionar, e estender do que código.
