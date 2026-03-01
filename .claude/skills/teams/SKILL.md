# Team Routing

Route requests, issues, and tasks to the correct persona.

## Trigger

Activate when the user needs to identify who should handle a request or task.

Keywords: "who handles", "which team", "route to", "assign to", "responsible for"

## What This Skill Does

1. **Analyze Request** — Understand the domain of the request
   - Identify key terms and affected subsystems
   - Map to one or more persona domains
   - Determine primary vs secondary ownership

2. **Route to Persona** — Match request to the routing table below
3. **Flag Cross-Cutting** — Identify if multiple personas need coordination

## Routing Table

| Domain | Owner | Agent File | Key Files |
|--------|-------|------------|-----------|
| Core Engine (Rust), System Twin, Semantic Diff, Architecture | Kael Okonkwo | kael-okonkwo | `crates/Intently_core/`, `Cargo.toml`, `benches/` |
| VSCode Extension, System Cockpit, Intention Mode, Product DX | Priya Chakrabarti | priya-chakrabarti | `apps/desktop/src/`, `vscode/` |
| LLM Orchestrator, Planner Engine, Skill System, AI Strategy | Jun Tanaka | jun-tanaka | `crates/Intently_core/src/planner/`, `crates/Intently_core/src/orchestrator/` |
| UI Implementation, Design System, Data Visualization | Dara Abramović | dara-abramovic | `apps/desktop/src/cockpit/`, `apps/desktop/src/components/` |
| Policy Engine, Evidence Engine, Sandbox, Security, Governance | Tomás Herrera | tomas-herrera | `crates/Intently_core/src/policy/`, `crates/Intently_core/src/evidence/` |
| CLI, Bootstrapper, Triggers, CI, Docs, Ecosystem | Maren Lindqvist | maren-lindqvist | `crates/Intently_cli/`, `.github/`, `docs/`, `schemas/` |

## Routing Logic

1. If the request mentions **Rust, core engine, IR, System Twin, semantic diff, or architecture** -> Kael
2. If the request mentions **VSCode extension, product, Cockpit, Intention Mode, or DX** -> Priya
3. If the request mentions **LLM, planner, sandbox execution, skills, or AI strategy** -> Jun
4. If the request mentions **UI, design system, visualization, Tailwind, components** -> Dara
5. If the request mentions **policy, evidence, security, compliance, governance** -> Tomás
6. If the request mentions **CLI, triggers, bootstrapper, CI, docs, onboarding, schemas** -> Maren
7. If cross-cutting -> primary owner + involve secondary personas

## Cross-Domain Patterns

- **Schema changes** — Kael (types) + Maren (ergonomics) + consumers
- **New Tauri commands** — Kael (Rust) + Priya (extension) + Dara (UI)
- **Policy additions** — Tomás (rules) + Kael (engine)
- **LLM task changes** — Jun (orchestrator) + Tomás (safety)
- **New features** — Priya (product) + Dara (UI) + Kael (core)
- **Onboarding flows** — Maren (bootstrapper) + Priya (value) + Tomás (security)

## Output Format

```
## Routing Decision

### Request: <summarized request>

### Primary Owner: <persona_name>
- Reason: <why this persona owns it>

### Secondary Personas (if applicable):
- <persona_name>: <reason for involvement>

### Suggested Action:
<what should happen next>
```
