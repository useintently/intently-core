## 14) Planner Engine — o cérebro que transforma “estado” em “ação”

### 14.1 Objetivo

O Planner pega os relatórios (diff, policies, evidence) e responde:

* “O que está bloqueando o gate?”
* “Qual a menor ação que destrava?”
* “Isso precisa de LLM ou dá para patch determinístico?”

Ele produz um **Action Plan** com passos ordenados e justificativa.

---

### 14.2 Inputs do Planner

* `intent.yaml` (regras e evidências exigidas)
* `semantic_diff.json` (impacto)
* `policy_report.json` (violations)
* `evidence_report.json` (missing/failing evidence)
* `repo_context` (stack, caminhos, frameworks detectados)

---

### 14.3 Output do Planner: `action_plan.json`

```json
{
  "meta": { "commit": "def456", "generated_at": "2026-02-26T12:10:00Z" },
  "gate_status": "fail",
  "blockers": [
    {"type":"policy", "id":"SEC-002"},
    {"type":"evidence_missing", "id":"INV-002"}
  ],
  "plan": [
    {
      "step": 1,
      "kind": "autofix_patch",
      "patch_id": "PATCH-LOG-REDACT-PII",
      "target": {"file":"src/payments/logger.ts","line":88},
      "why": "Remove PII from logs to satisfy SEC-002",
      "expected_effect": ["policy:SEC-002 pass"]
    },
    {
      "step": 2,
      "kind": "llm_task",
      "task_type": "generate_property_test",
      "inputs": {"invariant_id":"INV-002"},
      "why": "Missing required evidence for invariant INV-002",
      "expected_effect": ["evidence:INV-002 present"]
    },
    {
      "step": 3,
      "kind": "run_evidence",
      "scope": "incremental",
      "why": "Verify gate after fixes",
      "expected_effect": ["gate pass"]
    }
  ],
  "human_attention": [
    {
      "severity": "high",
      "topic": "New dependency added: payments → marketing",
      "question": "Is this boundary allowed? If not, choose event-based alternative."
    }
  ]
}
```

---

### 14.4 Heurísticas do Planner (simples e fortes)

**Regra 1 — Gate blockers primeiro**

* se policy fail → corrigir antes de gerar features/testes

**Regra 2 — Determinístico > LLM**

* se há patch template seguro → aplicar patch
* LLM só quando:

  * precisa criar testes
  * precisa refactor complexo
  * precisa criar código novo não-template

**Regra 3 — Menor mudança possível**

* preferir patch em 1 arquivo
* limitar “blast radius”

**Regra 4 — Se envolve risco alto, exigir humano**

* novas dependências proibidas
* mudanças em auth scopes
* alteração em data sensível
  → entra em `human_attention`

---

## 15) Biblioteca de Auto-fix Patches (determinísticos)

O segredo para domar LLM é: **80% das correções são repetíveis**.
Então a IDE precisa de patches “cirúrgicos” que:

* não inventam arquitetura
* não mudam comportamento além do necessário
* têm diff pequeno
* rodam evidence incremental após aplicar

### 15.1 Formato do patch template: `patch_template.yaml`

```yaml
id: PATCH-LOG-REDACT-PII
description: "Redact sensitive fields from logs"
applicability:
  languages: ["typescript","python"]
  patterns:
    - "logger.(info|debug|warn|error)"
    - "print("
params:
  - name: field_name
    type: string
actions:
  - type: replace_in_line
    match: "{{field_name}}"
    replace: "redact({{field_name}})"
post_actions:
  - type: ensure_helper_exists
    helper: "redact"
    location_hint: "src/shared/redact.ts"
```

### 15.2 12 patches MVP (os que mais pagam dividendos)

**Security**

1. PATCH-LOG-REDACT-PII
2. PATCH-LOG-MASK-SECRETS
3. PATCH-ADD-AUTH-GUARD (FastAPI dependency / Express middleware)

**Reliability**
4) PATCH-ADD-TIMEOUT-HTTPX/REQUESTS/FETCH/AXIOS
5) PATCH-ADD-RETRY-WRAPPER (ex.: tenacity / p-retry)
6) PATCH-ADD-IDEMPOTENCY-MIDDLEWARE (header + store)

**Architecture**
7) PATCH-EXTRACT-INTERFACE (domain interface + infra impl)
8) PATCH-REPLACE-DIRECT-CALL-WITH-EVENT-PUBLISH (síncrono → evento)

**Quality**
9) PATCH-ADD-INPUT-VALIDATION (zod/pydantic)
10) PATCH-ADD-SAFE-LOGGING-MIDDLEWARE

**Performance**
11) PATCH-ADD-CACHE-AROUND-EXTERNAL-CALL (TTL curto)
12) PATCH-BATCH-EXTERNAL-CALLS (quando detectável)

> A regra: patch é determinístico; se não dá para garantir, vira LLM task.

---

## 16) Governança de fronteiras: `boundaries.yaml`

Serve para ARC-001 e também para o UI agrupar “swimlanes”.

```yaml
version: 1
contexts:
  - name: payments
    components: ["svc.payments", "db.payments"]
  - name: marketing
    components: ["svc.marketing", "db.marketing"]
  - name: crm
    components: ["svc.crm", "db.crm"]

rules:
  forbidden_dependencies:
    - from: payments
      to: marketing
      reason: "Payments must be isolated from marketing concerns"
      allowed_alternatives:
        - "publish_event: payment.*"
        - "query_via_read_model: crm.payments_view"
  allowed_dependencies:
    - from: crm
      to: payments
      reason: "CRM may call payments APIs"
```

---

## 17) Overrides com auditoria: `policy_overrides.yaml`

O override precisa ser raro, expirar, e gerar dívida automaticamente.

```yaml
overrides:
  - id: OVR-SEC-002-2026-02-26
    policy: SEC-002
    scope:
      component: svc.payments
      file: src/payments/logger.ts
    reason: "Incident debugging: need temporary visibility"
    approved_by: "paulo"
    created_at: "2026-02-26"
    expires_at: "2026-03-05"
    compensations:
      - "TASK: add redaction middleware"
      - "TASK: add regression test for pii log scan"
    status: active
```

### Regras rígidas do sistema:

* override sem expiração → inválido
* override expirado → gate volta a falhar
* override conta para `max_active_overrides` do intent

---

## 18) Fluxo de override (UI + CI)

### UI

* Clique na policy fail → “Request Override”
* Form obrigatório:

  * motivo
  * escopo
  * expiração
  * compensação
* Gera DECISION LOG automaticamente (DEC-xxx)

### CI

* `policy_check` lê overrides
* se override ativo e dentro do escopo:

  * policy vira `pass_with_override`
* PR mostra banner:

  * “⚠ This merge relies on override OVR-... expira em 7 dias”

---

## 19) Como isso vira “pair programming acima do código”

A LLM continua veloz (gera código/testes/patch), mas o humano só precisa:

* olhar semantic diff
* ver gate
* aceitar ou mandar corrigir
* registrar decisão quando necessário

O humano não vira gargalo porque:

* revisão é **em 10 itens**, não 2000 linhas
* correções comuns são 1-clique (patch)
* o resto vira tasks estruturadas

---

