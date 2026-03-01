## 1) Formato do IR: `system_twin.json` (o “gêmeo digital”)

Objetivo: representar o sistema em um grafo tipado, para diff semântico, visualização e gating.

### 1.1 Estrutura (v0.1)

```json
{
  "meta": {
    "version": "0.1",
    "generated_at": "2026-02-26T12:00:00Z",
    "repo": "org/repo",
    "commit": "abc123",
    "branch": "feature/payments"
  },
  "intents": {
    "intent_file_hash": "sha256:..."
  },
  "components": [
    {
      "id": "svc.payments",
      "type": "service",
      "language": "python",
      "runtime": "fastapi",
      "owners": ["team-payments"],
      "tags": ["critical"]
    }
  ],
  "interfaces": {
    "api": [
      {
        "id": "api.POST./v1/checkout",
        "component": "svc.payments",
        "method": "POST",
        "path": "/v1/checkout",
        "auth": ["user"],
        "request_schema": "CheckoutRequest@v2",
        "response_schema": "CheckoutResponse@v1"
      }
    ],
    "events": [
      {
        "id": "evt.payment.succeeded",
        "component": "svc.payments",
        "schema": "PaymentSucceeded@v1",
        "direction": "publish"
      }
    ]
  },
  "flows": [
    {
      "id": "flow.checkout",
      "component": "svc.payments",
      "states": ["cart_created","pending_payment","paid","fulfilled","cancelled"],
      "transitions": [
        {"from": "cart_created", "to": "pending_payment", "event": "checkout.started"},
        {"from": "pending_payment", "to": "paid", "event": "payment.succeeded"},
        {"from": "pending_payment", "to": "cancelled", "event": "checkout.cancelled"}
      ]
    }
  ],
  "data": {
    "entities": [
      {
        "id": "ent.Payment",
        "component": "svc.payments",
        "fields": [
          {"name": "id", "type": "uuid"},
          {"name": "user_id", "type": "uuid"},
          {"name": "card_last4", "type": "string", "pii": true, "storage": "forbidden"}
        ]
      }
    ]
  },
  "dependencies": [
    {
      "from": "svc.payments",
      "to": "svc.marketing",
      "type": "runtime_call",
      "confidence": 0.9
    }
  ],
  "sinks": {
    "logs": [
      {
        "component": "svc.payments",
        "location": {"file": "src/payments/logger.ts", "line": 88},
        "keys_detected": ["card_last4"]
      }
    ],
    "external_calls": [
      {
        "component": "svc.payments",
        "target": "https://api.stripe.com",
        "timeout_ms": 0
      }
    ]
  }
}
```

### 1.2 Como gerar IR no MVP (pragmático)

* APIs: OpenAPI gerado pelo framework (FastAPI/TS) ou extraído de rotas
* Dependencies: análise de imports + chamadas HTTP/grpc + env config
* Sinks: scanner simples (logs, stdout, requests sem timeout)
* Flows: vem do `intent.yaml` no MVP (não tenta inferir do código ainda)

---

## 2) Formato do diff: `semantic_diff.json`

Objetivo: “o que mudou de verdade” sem linhas de código.

```json
{
  "meta": { "from_commit": "abc123", "to_commit": "def456" },
  "changes": {
    "interfaces": {
      "api": {
        "added": [],
        "removed": [],
        "modified": [
          {
            "id": "api.POST./v1/checkout",
            "fields_changed": ["request_schema"],
            "before": { "request_schema": "CheckoutRequest@v1" },
            "after": { "request_schema": "CheckoutRequest@v2" }
          }
        ]
      }
    },
    "flows": {
      "modified": [
        {
          "id": "flow.checkout",
          "transitions_added": [
            {"from":"pending_payment","to":"cancelled","event":"checkout.cancelled"}
          ]
        }
      ]
    },
    "dependencies": {
      "added": [
        {"from":"svc.payments","to":"svc.marketing","type":"runtime_call"}
      ]
    },
    "data": {
      "sensitive_touched": [
        {"entity":"ent.Payment","field":"card_last4","reason":"appears_in_logs"}
      ]
    }
  },
  "risk_summary": {
    "security": "high",
    "reliability": "medium",
    "architecture": "high",
    "performance": "low"
  }
}
```

---

## 3) Evidências: `evidence_report.json`

Objetivo: o humano assina em cima disso (gate).

```json
{
  "meta": { "commit": "def456", "run_id": "ci-9012" },
  "requirements": [
    {"id":"REQ-1","type":"contract_tests","target":"interfaces.api","status":"pass"},
    {"id":"REQ-2","type":"property_tests","target":"invariants","status":"missing", "missing":["INV-002"]},
    {"id":"REQ-3","type":"integration_tests","target":"critical_flows","status":"pass"}
  ],
  "results": {
    "tests": {"passed": 120, "failed": 0, "skipped": 3},
    "properties": [
      {"id":"INV-001","status":"pass","cases":10000},
      {"id":"INV-002","status":"missing"}
    ],
    "performance": {"p95_delta_ms": 12, "budget_ms": 30, "status":"pass"}
  },
  "artifacts": {
    "logs_url": "artifact://ci/logs",
    "junit_url": "artifact://ci/junit.xml"
  },
  "gate": {
    "status": "fail",
    "reasons": ["missing_property_test:INV-002"]
  }
}
```

---

## 4) Policies: `policy_report.json`

Objetivo: explicável e acionável (com “auto-fix hooks”).

```json
{
  "meta": { "commit": "def456" },
  "policies": [
    {"id":"SEC-001","rule":"all_endpoints_must_require_auth","status":"pass"},
    {
      "id":"SEC-002",
      "rule":"no_pii_in_logs",
      "status":"fail",
      "violations": [
        {
          "component":"svc.payments",
          "location":{"file":"src/payments/logger.ts","line":88},
          "details":{"keys":["card_last4"]},
          "fix_suggestions":[
            {"type":"apply_patch","action":"redact_field","field":"card_last4"},
            {"type":"apply_patch","action":"remove_field","field":"card_last4"}
          ]
        }
      ]
    }
  ],
  "gate": { "status":"fail", "reasons":["SEC-002"] }
}
```

---

# 5) As 10 policies iniciais (as que evitam desastre real)

Essas são “low effort / high impact”.

### Segurança

1. **SEC-001**: todo endpoint precisa de auth (exceto allowlist explícita)
2. **SEC-002**: PII não pode aparecer em logs
3. **SEC-003**: secrets/tokens não podem aparecer em logs nem em exceptions
4. **SEC-004**: validação de input obrigatória (schema) em endpoints públicos

### Confiabilidade

5. **REL-001**: chamadas externas precisam de timeout + retry policy definida
6. **REL-002**: idempotência obrigatória em endpoints de escrita críticos (ou chave)
7. **REL-003**: efeitos colaterais devem ser “exactly-once-ish” (pelo menos once com dedupe)

### Arquitetura

8. **ARC-001**: dependências proibidas entre bounded contexts (payments → marketing etc.)
9. **ARC-002**: camadas não podem inverter (domain não importa infra; handlers não importam DB direto)

### Performance / Custo

10. **PERF-001**: fan-out externo limitado por request (ex.: max 3 calls) ou exige cache/batch

> Importante: policy boa é a que **fala “onde” e “como”** você quebrou, e oferece auto-fix.

---

# 6) Templates de tarefas LLM (sem prompt livre)

Aqui está o “hack” para controlar LLM: ela não recebe “conversa”; recebe **tarefas estruturadas** com contratos. Isso reduz hallucination e aumenta repetibilidade.

## 6.1 Modelo de task: `llm_task.json`

```json
{
  "task_id": "TASK-GEN-PROPTEST-INV-002",
  "type": "generate_property_test",
  "inputs": {
    "invariant_id": "INV-002",
    "invariant_text": "Refund nunca pode exceder total pago",
    "scope": ["refund"],
    "stack": {"language":"python","test_framework":"pytest","property":"hypothesis"},
    "files_context": ["src/payments/refund.py", "tests/test_refund.py"],
    "constraints": {
      "no_network": true,
      "max_files_changed": 3,
      "must_add_tests": true
    }
  },
  "expected_outputs": {
    "patch": true,
    "explanations": ["what_changed", "why_passes_gate"],
    "evidence": ["how_to_run_tests"]
  }
}
```

## 6.2 8 tipos de tasks (MVP)

1. `generate_property_test`
2. `generate_contract_tests_from_openapi`
3. `fix_policy_violation`
4. `refactor_with_constraints` (ex.: “separar módulos sem mudar API”)
5. `add_timeout_and_retry` (REL-001)
6. `add_idempotency_key_support` (REL-002)
7. `explain_semantic_impact` (para o humano)
8. `produce_migration_plan` (DB schema change)

---

# 7) LLM Sandbox: regras de execução (para não virar terror)

No MVP, sandbox é simples e rígido:

* sem acesso a rede por padrão
* sem acesso a secrets reais
* workspace isolado por task
* permissões por “capabilities” (ler/editar arquivos, rodar testes, etc.)
* toda alteração precisa ser um patch aplicável (git apply)

**Regra de ouro**: LLM não “edita livre”. Ela “propõe patch”.

---

# 8) Integração com Git/CI: checklist implementável

### No repositório

* `intent.yaml` na raiz
* `/.igdp/` opcional para config (budgets, allowlists)

### No CI (GitHub Actions por exemplo)

Jobs mínimos:

1. `intent_validate` → valida YAML
2. `build_system_twin` → gera `system_twin.json`
3. `semantic_diff` → compara com base branch, gera `semantic_diff.json`
4. `policy_check` → gera `policy_report.json`
5. `evidence_check` → gera `evidence_report.json`
6. `comment_pr` → posta resumo

E marca `policy_check` + `evidence_check` como required.

---

# 9) Adoção incremental (roteiro que funciona no mundo real)

### Semana 1: só CI + PR comment

* time já ganha valor sem mudar IDE
* revisa por impacto e gate

### Semana 2–3: plugin VS Code

* cockpit dentro do editor
* abrir relatórios do CI localmente
* botões: “gerar evidência faltante”, “corrigir policy”

### Semana 4+: IDE cockpit própria

* vira a central de governança para tech lead/staff
* editor fica secundário

---

