# Intently

### The Intent-Driven Development IDE

> Developers don't read AI-generated code.
> They declare intent, review impact, and govern with evidence.

---

## What is Intently?

Intently is an **analytical, proactive IDE for intent-driven development**. It works as a copilot alongside your existing code generation tools — Claude Code, Codex, Cursor, Windsurf, Copilot — running as a VSCode extension.

You keep using whatever tool you prefer to generate code. Intently is where you **see, review, automate, and govern** — without ever reading code.

```
You (declare intent) → LLM (generates code) → Intently (analyzes, validates, acts) → You (govern)
```

## The Problem

LLMs made code generation instant. But IDEs, Git, and CI still operate in the source-code paradigm — files, lines, textual diffs.

The result: developers spend more time understanding what the AI generated than it took the AI to generate it. Cognitive burnout grows. Reviews become superficial. Implicit changes slip through. Trust in AI output is low.

**The bottleneck moved from "writing code" to "comprehending, validating, and governing changes."** And there's no tooling for that.

## The Solution: Three Pillars

### 1. Intention Mode (Plan)

Declare what you want to change. See the impact before any code exists.

```
You: "Add cancellation support to checkout flow with proportional refund"

Intently shows:
  As Is: checkout [cart → pending → paid → fulfilled]
  To Be: checkout [... → paid → refund_requested → refunded]
  Delta: +1 API, +2 states, +1 invariant, 1 policy impacted

You approve → Intently generates implementation roadmap → LLMs execute
```

No code to read. You review **system impact**, not diffs.

### 2. Development Observability (DevObs)

SRE concepts applied to the development cycle. Intently monitors system health in real-time using indicators and objectives — like an SRE team monitors production.

| SRE Concept | Intently Equivalent | Example |
|---|---|---|
| SLI | **DLI** (Development Level Indicator) | Policy compliance %, evidence coverage % |
| SLO | **DLO** (Development Level Objective) | "Security policies always green before merge" |
| Alert | **Trigger** | When DLI drops below DLO, Intently acts |
| Dashboard | **System Cockpit** | Real-time system health |
| Incident | **Governance Debt** | Active overrides, missing evidence |

### 3. Governance Triggers

Proactive automations that connect observability to action. When an indicator changes, Intently acts — generates tests, applies patches, notifies, blocks.

**Built-in triggers:**
- Policy violation detected → auto-fix or generate task
- Missing evidence for invariant → generate property test
- PII detected in logs → apply redaction patch
- API breaking change → block merge + generate contract tests
- Override expired → reactivate policy + create correction task

**Custom triggers:**

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
```

## Core Concepts

**Intent (`intent.yaml`)** — The source of truth. What must be true about your system: services, APIs, flows, invariants, policies. Versioned and auditable. Evolves with the system.

**System Twin** — A machine-readable representation of your system as it is now. Components, dependencies, contracts, flows. The formalized mental model — the memory that AI doesn't have.

**Semantic Diff** — What changed in behavior and risk, not in lines. "1 API altered, 2 flows affected, PII touched" — not "487 lines added in 12 files".

**Policies** — Verifiable, actionable rules about the system. Security (SEC), reliability (REL), architecture (ARC), performance (PERF). Each policy: detect → locate → suggest fix → auto-correct.

**Evidence** — Executable tests and validations that prove: policies satisfied, invariants maintained, risk controlled. Mandatory, incremental, explicit.

**Governance Triggers** — Proactive automations. When indicator changes → Intently acts. Customizable, chainable, auditable.

## How It Works

```
1. Declare intent                    "Add refund support to checkout"
2. Preview impact                    As Is → To Be with delta
3. Approve roadmap                   Structured tasks, not vague prompts
4. LLMs execute                      Claude Code / Codex via VSCode
5. Intently observes                     System Twin updates, DLIs recalculate
6. Triggers fire                     Auto-fix, generate tests, notify
7. You govern                        Merge / adjust / override
```

## Getting Started

### Phase 0: Zero Config

Install the VSCode extension. Intently monitors your repo via git, generates the System Twin and semantic diff automatically. No `intent.yaml` required. Immediate value: "what did this change really do to my system?"

### Phase 1: Bootstrap Intent

```bash
Intently init
```

Auto-detects languages, frameworks, APIs, and dependencies. Generates `intent.yaml` with confidence tags. You review and commit — like Renovate's onboarding PR.

### Phase 2: Full Governance

Refine your intent. Configure DLOs. Create custom triggers. Use Intention Mode for every change. The code becomes a derived artifact. The intent is the source of truth.

## Architecture

```
┌─────────────────────────────────────────────┐
│  VSCode                                     │
│  ┌──────────┐    ┌───────────────────────┐  │
│  │ Editor   │    │ Intently Extension        │  │
│  │ (Claude  │    │  Intention Mode       │  │
│  │  Code /  │    │  System Cockpit       │  │
│  │  Cursor) │    │  Trigger Alerts       │  │
│  └──────────┘    └───────────────────────┘  │
└─────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│  Intently Core Engine (Rust)                    │
│  Intent | System Twin | Policies | Evidence │
│  Planner | Triggers | LLM Orchestrator      │
└─────────────────────────────────────────────┘
```

- **Core Engine:** Rust (CLI + Library). Deterministic, auditable. Same rules in dev and CI.
- **VSCode Extension:** System Cockpit, Intention Mode, Trigger notifications.
- **Languages (MVP):** Python (FastAPI) + TypeScript (Express/Node).
- **Integration:** Observer model (monitors git). Evolves to Orchestrator.

## Tech Stack

- **Core:** Rust (CLI + Library)
- **Parsing:** tree-sitter CSTs + ast-grep YAML catalogs
- **Extension:** VSCode Extension API (TypeScript)
- **Orchestration:** LangGraph with typed state
- **Sandbox:** Docker containers, OverlayFS (MVP+)
- **Formats:** YAML (intent, triggers, policies), JSON (System Twin, action plans)

## Design Principles

**Proactive, not reactive.** Intently acts before you ask. Triggers fire automatically. The goal is to reduce cognitive load, not add to it.

**System-level, not file-level.** You see services, APIs, flows, invariants — not files and lines. The right level of abstraction for governing AI-generated code.

**Intent as source of truth.** Code is derived. Intent is declared, versioned, and auditable. Review happens at the intent level, not the diff level.

**Configuration as data.** Policies are YAML. Triggers are YAML. Intent is YAML. Patterns are YAML. Data is easier to validate, version, and extend than code.

**Graduated intervention.** Not all-or-nothing. Confidence scores, deterministic patches before LLM tasks, human escalation as last resort. The system does what it can and asks for help with what it can't.

**Nothing implicit.** Every LLM action requires a registered skill. Every override requires justification and expiration. Every decision is logged.

## Contributing

Intently is in active development. See [CLAUDE.md](./CLAUDE.md) for development guidelines and architecture details.

## License

[TBD]
# intently
