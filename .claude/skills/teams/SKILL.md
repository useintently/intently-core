---
name: routing-teams
description: Routes requests, issues, and tasks to the correct intently-core persona. Kael Okonkwo owns core engine, CodeModel, semantic diff, KnowledgeGraph, extractors, architecture, and performance. Tomás Herrera owns security review, dependency audit, input validation, and unsafe code. Use when identifying who should handle a request or task.
---

# Routing Teams

## Critical rules

**ALWAYS:**
- Analyze the full request domain before routing — a "refactor extractors" request touches both Kael (architecture) and Tomás (security patterns)
- Flag cross-cutting concerns explicitly — most non-trivial changes need Kael as primary + Tomás for security review
- Include a suggested next action in the routing decision — routing without action is incomplete

**NEVER:**
- Route without reading the request carefully — keywords alone can be misleading
- Skip security review on changes touching extractors, dependencies, or error handling — always involve Tomás
- Assign to personas that don't exist — the team is exactly Kael Okonkwo and Tomás Herrera

## Routing table

| Domain | Owner |
|--------|-------|
| Core Engine, CodeModel, Semantic Diff, KnowledgeGraph, Extractors, Architecture, Performance | **Kael Okonkwo** |
| Security, Dependency Audit, Input Validation, Unsafe Code, Supply Chain | **Tomás Herrera** |

## Routing logic

1. Mentions Rust, engine, CodeModel, diff, graph, extractors, performance, benchmarks, architecture → **Kael**
2. Mentions security, secrets, PII, dependency audit, input validation, unsafe, supply chain → **Tomás**
3. Cross-cutting → **Kael** as primary, **Tomás** for security review

## Output format

```
## Routing Decision

### Request: <summarized request>
### Primary Owner: <persona> — <reason>
### Secondary (if applicable): <persona> — <reason>
### Suggested Action: <what should happen next>
```
