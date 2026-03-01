## 1) Wireframes por telas (com interações)

### Tela 0 — Welcome / Project Hub

Objetivo: o humano escolhe “onde governar” sem abrir arquivo.

```
┌──────────────────────────────────────────────────────────────┐
│  IGDP — Project Hub                                          │
│  [New Project]  [Open Repo]  [Connect GitHub]  [Import Intent]│
└──────────────────────────────────────────────────────────────┘

Recent Projects:
- Loved CRM (main)   Status: ✅ All gates green
- Macaw Runtime      Status: ⚠ 2 policy warnings
- Hotploy            Status: ❌ Evidence failing

Right Panel: “What changed since last session?”
- PR #182: payments refactor (semantic impact: 2 APIs)
- PR #183: auth scopes update (risk: HIGH)
```

Interações:

* “Open Repo” → detecta se tem `intent.yaml`. Se não tiver, “Bootstrap intent”.
* “Connect GitHub” → pega PRs e checks pra compor a timeline.

---

### Tela 1 — System Cockpit (Home Governança)

Objetivo: o usuário bate o olho e sabe **se está seguro**.

```
┌───────────────────────────────────────────────────────────────┐
│ System Cockpit | branch: feature/payments | PR: #183          │
│ [Run Evidence] [Ask LLM] [Record Decision] [Open Code]        │
└───────────────────────────────────────────────────────────────┘

┌───────────────┬───────────────────────────────┬───────────────┐
│ System Twin    │ Semantic Impact              │ Risk Radar     │
│ (Grafo)        │ - APIs: 1 changed            │ - Security: ↑  │
│                │ - Flows: 2 affected          │ - Perf: ok     │
│                │ - Data: PII touched          │ - Arch: ok     │
└───────────────┴───────────────────────────────┴───────────────┘

┌───────────────────────────────────────────────────────────────┐
│ Evidence Gate                                                 │
│ Contracts: ✅ 18/18   Policies: ❌ 31/32 (SEC-002)             │
│ Tests: ✅ 120/120     Property: ⚠ 4/5 (INV-002 missing)       │
│ Performance: ✅ p95 +12ms (budget +30ms)                      │
└───────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│ “What do you want to do next?”                                │
│ [Fix policy violation] [Generate missing evidence] [Approve]  │
└───────────────────────────────────────────────────────────────┘
```

Interações importantes:

* Clique em “SEC-002” → abre a Policy View (explica *onde* e *como* violou).
* “Generate missing evidence” → dispara LLM Task: gerar property test para INV-002.
* “Approve” só habilita se gate estiver green (ou exige override auditado).

---

### Tela 2 — Semantic Diff View (revisão sem ler código)

Objetivo: o humano entende **mudança real** sem ficar catando arquivo.

```
Semantic Diff (IR v481 → v482)

APIs
- POST /v1/checkout
  - input_schema: CheckoutRequest@v1 → @v2 (field added: coupon_code)
  - auth: unchanged

Flows
- checkout
  - added transition: “pending_payment” → “cancelled”
  - affected invariants: INV-001, INV-002

Data
- Payment.card_last4: touched
  - policy: storage forbidden
  - log scan: “card_last4” found in logger call

Dependencies
- payments → marketing_service (NEW)  [violates ARC-001? no, payments must not depend]
```

Interações:

* cada item tem “Show Evidence” e “Why this matters”
* “Open code at source” existe, mas é opcional (último recurso)

---

### Tela 3 — Flow Studio (state machine executável)

Objetivo: ver e simular o fluxo crítico.

```
Flow: checkout

[cart_created] → [pending_payment] → [paid] → [fulfilled]
                    |                 |
                    └→ [cancelled]    └→ [refund_requested] → [refunded]

Buttons:
[Simulate] [Generate Tests] [Add Invariant] [Ask LLM to implement transition]
```

Interações:

* **Simulate** roda um “walk” com inputs fake e mostra estados, eventos, persistência.
* **Generate Tests** cria testes de integração e property tests vinculados ao fluxo.

---

### Tela 4 — Policy View (governança real)

Objetivo: policies explicáveis, acionáveis e bloqueantes.

```
Policy SEC-002: no_pii_in_logs  ❌ FAIL

Violations:
1) src/payments/logger.ts: line 88
   - detected key: "card_last4"
   - sink: logger.info(...)
   - suggestion: redact() or remove

Auto-fix options:
[Apply redact patch] [Remove log field] [Whitelist w/ justification]
```

Interações:

* “Apply redact patch” → gera patch determinístico + roda evidence incremental.
* “Whitelist” exige justificativa + expiração (ex.: 7 dias) + vira item de dívida.

---

### Tela 5 — Evidence View (o coração)

Objetivo: transformar “testes” em “evidência legível”.

```
Evidence Required (from intent.yaml):
- contract_tests:interfaces.api   ✅ PASS
- property_tests:invariants       ⚠ MISSING: INV-002
- integration_tests:critical_flows✅ PASS

Evidence Details:
Property INV-001 ✅
- Checked 10k cases
- Found 0 duplicates

Property INV-002 ⚠
- Not implemented
- Suggested generator: random payments + refunds
[Generate property test] [Mark as not applicable (requires justification)]
```

Interações:

* “Generate property test” chama LLM com template rígido (sem liberdade demais).
* “Mark not applicable” abre uma decisão auditada e exige compensação.

---

### Tela 6 — Decision Log (a memória do sistema)

Objetivo: auditar e evitar “decisões invisíveis”.

```
Decision: DEC-104
Title: Allow temporary whitelist for SEC-002
Reason: debugging incident #552
Scope: payments-service
Expires: 2026-03-05
Compensations:
- Add redaction middleware (TASK-881)
- Add regression test for log scan (TASK-882)
Status: Active
```

Interações:

* “Expire now” + “Create follow-up tasks”
* integra com Issues

---

## 2) DSL mínima (v0.2) — pequenas melhorias para virar produto

Vou manter minimalista, mas adicionar 2 coisas essenciais:

1. **flow states** (para simular)
2. **decision requirements** (para overrides)

```yaml
flows:
  - name: "checkout"
    states: ["cart_created","pending_payment","paid","fulfilled","cancelled"]
    transitions:
      - {from: "cart_created", to: "pending_payment", event: "checkout.started"}
      - {from: "pending_payment", to: "paid", event: "payment.succeeded"}
      - {from: "pending_payment", to: "cancelled", event: "checkout.cancelled"}

overrides:
  require_decision_log: true
  max_active_overrides: 3
  override_expiration_days: 7
```

Isso dá base para:

* simulação
* gating
* auditoria

---

## 3) Backlog MVP natural (sem enrolação)

### Épico A — System Twin (IR + diff semântico)

* Extrair IR do repo (APIs, deps, fluxos quando declarados)
* Persistir snapshots por commit
* Renderizar grafo + semantic diff

### Épico B — Intent DSL + Validador

* Parser YAML
* Schema validation
* Lint de intenção (ex.: flow referenced mas não existe)

### Épico C — Evidence Engine incremental

* Runner incremental (detecta área afetada)
* Contract tests (API schema)
* Property tests templates
* Integration tests por fluxo

### Épico D — Policy Engine + scanners

* Regras básicas (auth required, timeouts, no PII logs, no forbidden deps)
* Scanners: logs, deps, secrets

### Épico E — LLM Orchestrator + sandbox

* “Tasks” estruturadas (gerar teste, aplicar patch, refatorar)
* Execução em sandbox
* Logging completo (audit)

### Épico F — Git/CI Integration

* GitHub Action que publica artefatos (IR, diff, evidence report, policy report)
* PR comment automático com resumo semântico
* Required checks

### Épico G — Plugin VS Code (adoção incremental)

* Painel “System Cockpit”
* Botões Run Evidence / View Diff / Fix Policy
* Abrir telas web embutidas (webview)

---

## 4) Detalhe crucial: como o humano “manda na LLM” sem ser técnico

A IDE precisa ter **comandos humanos** (botões) e não depender de prompt perfeito:

* Fix violation
* Generate missing evidence
* Explain impact in plain language
* Propose safer alternative
* Create decision record

Isso reduz o “custo cognitivo” do humano.

---

## 5) Pergunta que define o MVP (e eu respondo pra você)

**Começa como plugin**.
Porque o valor nasce em:

* semantic diff
* gates
* evidência

Não em editor de texto.

A IDE completa vem depois, como cockpit.

---

S