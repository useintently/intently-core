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
| Core Engine, System Twin, Semantic Diff, KnowledgeGraph, Extractors, Architecture, Performance | Kael Okonkwo | kael-okonkwo | `src/`, `Cargo.toml`, `tests/` |
| Security Review, Dependency Audit, Input Validation, Unsafe Code | Tomás Herrera | tomas-herrera | `src/twin/extractors/`, `Cargo.toml` |

## Routing Logic

1. If the request mentions **Rust, core engine, IR, System Twin, semantic diff, KnowledgeGraph, extractors, performance, benchmarks, or architecture** -> Kael
2. If the request mentions **security, secrets, PII, dependency audit, input validation, unsafe code, supply chain** -> Tomás
3. If cross-cutting -> Kael as primary, Tomás for security review

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
