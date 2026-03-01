Perfeito — agora a gente entra na parte que separa “ideia legal” de **sistema que realmente funciona**:

1. **Como o Policy Engine executa regras (com precisão e baixo custo)**
2. **Como o Evidence Engine escolhe o mínimo de testes (incremental inteligente)**
3. **Como o UI renderiza o grafo sem virar um caos**

Vou te entregar isso como **spec implementável**.

---

## 10) Policy Engine — como implementar cada regra (MVP pragmático)

### 10.1 Filosofia do MVP

Nada de “análise formal de programa” no começo. O MVP usa três técnicas **rápidas e eficientes**:

1. **Extração do IR** (do `system_twin.json`)
2. **Scanners de sinks** (logs, rede, deps, secrets) com heurísticas boas
3. **“Taint-lite” opcional** (propagação simples de campos sensíveis)

E toda policy retorna:

* status (pass/fail/warn)
* lista de violations com localização
* sugestões de auto-fix (patch templates)

---

### 10.2 Implementação das 10 policies iniciais

#### SEC-001 — all_endpoints_must_require_auth

**Fonte**: IR.interfaces.api
**Regra**: endpoint sem `auth` → fail (exceto allowlist)

**Como**:

* se `auth` vazio ou ausente → violation
* allowlist em `.igdp/policy_overrides.yaml`

**Auto-fix**:

* inserir dependência/middleware padrão de auth
* ou marcar endpoint explicitamente como “public” no intent (exige decisão)

---

#### SEC-002 — no_pii_in_logs

**Fonte**: IR.sinks.logs + Data.entities.fields(pii=true)
**Regra**: qualquer log que inclua key/variável PII → fail

**Como (MVP)**:

* scanner procura padrões:

  * `logger.(info|debug|warn|error)(...)`
  * `print(...)`, `console.log(...)`
* extrai chaves literais e nomes comuns (`card_last4`, `cpf`, `email`, `phone`)
* cruza com `data.entities.fields` onde `pii: true`
* se achar: violation com file/line

**Auto-fix**:

* substituir `field` por `redact(field)`
* ou remover do payload de log
* ou `hash(field)` se permitido

---

#### SEC-003 — no_secrets_in_logs_or_exceptions

**Fonte**: scanner
**Como**:

* detecta strings com padrões de token (JWT, keys, “sk-”, etc.)
* detecta logs de `os.environ[...]`
* detecta exceptions que imprimem request headers inteiros

**Auto-fix**:

* sanitize headers
* mask tokens

---

#### SEC-004 — validate_inputs_on_public_endpoints

**Fonte**: IR.interfaces.api + AST light
**Como**:

* para FastAPI: verificar se endpoint tem model Pydantic / validação
* para TS: verificar schema (zod/joi) no handler
* fallback: se `request_schema` existe mas não há validação detectada → warn/fail dependendo do nível

**Auto-fix**:

* gerar schema/validator e plugar

---

#### REL-001 — external_calls_must_have_timeouts (+ retries policy)

**Fonte**: IR.sinks.external_calls
**Como**:

* scanner identifica `requests.get/post`, `httpx`, `fetch`, `axios`, gRPC client
* se `timeout` ausente → fail
* retry policy: mínimo = documentado (decorator/backoff)

**Auto-fix**:

* patch adiciona timeout padrão e retry wrapper

---

#### REL-002 — idempotency_on_critical_writes

**Fonte**: intent.yaml (critical endpoints) + IR.interfaces.api
**Como**:

* se endpoint crítico POST/PUT/PATCH e não há:

  * header `Idempotency-Key` aceito
  * ou dedupe store (db/cache)
    → warn/fail

**Auto-fix**:

* criar middleware + tabela/cache de idempotência
* gerar teste property: “mesma key → mesmo efeito”

---

#### REL-003 — dedupe_on_at_least_once_side_effects

**Fonte**: flows + external calls + “side effect annotations”
**MVP**:

* se o fluxo chama gateway externo e não há “dedupe marker” → warn

**Auto-fix**:

* inserir dedupe key usando event id / request id

---

#### ARC-001 — forbidden dependencies between bounded contexts

**Fonte**: IR.dependencies
**Como**:

* arquivo `.igdp/boundaries.yaml` define contextos e proibições
* se `from->to` proibido → fail

**Auto-fix**:

* sugerir “introduzir interface/evento”
* gerar adapter (publish event ao invés de call direto)

---

#### ARC-002 — layer inversion (domain importing infra)

**Fonte**: scanner de imports
**Como**:

* define caminhos: `domain/`, `infra/`, `api/`
* se `domain` importa `infra` → fail

**Auto-fix**:

* extrair interface para domain + implementação em infra

---

#### PERF-001 — fan-out externo máximo

**Fonte**: análise do handler + IR.external_calls
**MVP**:

* por endpoint, contar chamadas externas detectadas (mesmo arquivo/função)
* se > limite (ex.: 3) → warn/fail

**Auto-fix**:

* sugerir batch/cache
* ou mover para job assíncrono

---

### 10.3 Taint-lite (opcional no MVP 2)

Para SEC-002/003 ficar mais inteligente:

* marca fontes PII (`request_schema`, DB fields `pii=true`)
* propaga “taint” por variáveis simples em um arquivo (nível function)
* se variável “tainted” chega em log → fail

Sem SSA, sem CFG complexo. Só o suficiente.

---

## 11) Evidence Engine — seleção incremental (o truque da escala)

### 11.1 Problema humano

Rodar “tudo sempre” mata:

* tempo
* custo
* fluxo
* paciência do time

### 11.2 Solução: Impact-Based Test Selection (IBTS)

A cada mudança, você já tem:

* `semantic_diff.json`
* lista de arquivos alterados
* mapeamento “artefatos → evidência”

O Engine calcula um **Impact Set**:

**Impact Set =**

* APIs tocadas
* fluxos afetados
* invariantes afetados
* componentes afetados
* dependências novas
* sinks tocados (logs/rede)

E disso deriva o **Minimum Evidence Set**.

---

### 11.3 Regras de seleção (v0)

#### Se API mudou:

* rodar contract tests do endpoint
* rodar integração do fluxo relacionado
* rodar property tests de invariantes no scope

#### Se flow mudou:

* rodar simulação do fluxo
* rodar integration tests do fluxo
* rodar invariantes referenciadas pelo flow

#### Se mexeu em data sensível:

* rodar policy scan + testes de sanitização/log redaction

#### Se adicionou dependência externa:

* rodar reliability checks (timeout, retry)
* rodar smoke perf (p95 budget)

---

### 11.4 Algoritmo (simples e implementável)

1. Compute `impact = diff_to_impact(semantic_diff)`
2. Build `required = intent.evidence.required`
3. Resolve `tests_to_run = evidence_plan(impact, required)`
4. Execute em ordem:

   * lint/typecheck rápido
   * policies
   * unit/contract
   * property
   * integration
   * perf smoke
5. Emit `evidence_report.json`

---

### 11.5 Cache e velocidade (muito importante)

* cache por commit para unit tests
* cache por “API signature hash”
* property tests com budget adaptativo:

  * PR: 2k casos
  * merge main: 20k casos
* perf smoke só quando impacto envolve endpoints críticos ou dependência nova

---

## 12) UI Graph Rendering — como não virar um mapa do metrô infernal

### 12.1 Grafo em 3 níveis (zoom semântico)

O grafo precisa ter “progressive disclosure”:

**Nível 1 — Sistema**

* nós: serviços/components
* arestas: dependências (runtime/event)
* objetivo: “onde mexeu?” “o que depende do quê?”

**Nível 2 — Interface**

* ao clicar em serviço: mostra APIs/eventos desse serviço

**Nível 3 — Fluxo**

* ao clicar em flow: mostra state machine (separado do grafo de deps)

Misturar tudo no mesmo canvas dá ruim. Separe.

---

### 12.2 Layout e filtros (MVP)

Use um layout estável e previsível:

* layout tipo DAG (left→right) por camadas/boundaries
* agrupamento por bounded context (swimlanes)
* destaque forte para:

  * nós afetados
  * arestas novas
  * violações (ícone vermelho)
* filtros:

  * “mostrar só afetados”
  * “mostrar só críticos”
  * “mostrar só novas dependências”
  * “mostrar só security”

---

### 12.3 UX: o humano precisa de respostas em 10 segundos

Três botões de ouro no grafo:

1. **Focus on Impact**

   * esconde tudo exceto subgrafo afetado

2. **Why is this risky?**

   * abre painel explicando em linguagem humana, apontando policy/violations

3. **Show Evidence**

   * abre os testes e simulações relevantes daquele nó/flow

---

## 13) O próximo passo lógico (ainda mais implementável)

Agora que temos policy + evidence + UI rules, falta definir:

### 13.1 O “Planner” interno (o cérebro que cria tarefas)

* recebe `semantic_diff + policy_report + evidence_report`
* decide “próxima ação sugerida”:

  * gerar teste faltante
  * corrigir policy
  * propor refactor limitado
* gera `llm_task.json` (estruturado)

### 13.2 O Auto-fix patch library

Uma biblioteca de patches determinísticos:

* add timeout
* add redaction
* add auth guard
* add idempotency middleware
* remove forbidden dependency (trocar por event publish)

Quanto mais patch determinístico, menos risco.

---

Se você quiser que eu continue no mesmo ritmo, eu avanço com:

1. **spec do Planner** (inputs/outputs + heurísticas)
2. **biblioteca de Auto-fix patches** (templates por stack: Python/FastAPI, TS/Express, Next.js)
3. **formato do `boundaries.yaml` e `policy_overrides.yaml`** (pra ARC-001 e overrides auditados)

Isso fecha o circuito “mudança → análise → ação → evidência → decisão” sem depender do humano ler código.
