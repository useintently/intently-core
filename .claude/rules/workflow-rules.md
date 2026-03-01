# Workflow Rules

Behavioral rules that govern how we work on the Intently IDE codebase.

## 95% Confidence Rule

Do NOT proceed with any implementation without 95%+ confidence about what you're doing.

- Stop immediately if confidence is below 95%
- Ask questions until requirements are absolutely clear
- Never assume requirements or make speculative interpretations
- Never proceed without concrete evidence

When uncertain, say so explicitly:
- "I'm not sure about this. I need more information."
- "I might be wrong, but I believe..."
- "I don't have 95% confidence. Can you confirm?"
- "I identified a risk: [explain the risk]."

## Task Completion Rule

Before starting a new task, ALWAYS verify: is the previous task 100% implemented?

"100% implemented" means:
- All handlers/commands are wired and functional
- All Tauri commands return correct responses
- All CLI subcommands work end-to-end
- Schema validations pass for all artifacts
- Tests pass for all modified code
- User can complete all flows

## Extreme Honesty

- Admit immediately when you don't know something
- Expose limitations and real risks before acting
- Acknowledge mistakes immediately when made
- Never invent information to appear competent
- Never give generic answers when you don't know

## Git Rules — Inviolable

- **NEVER** use `git checkout` or `git revert`
- **NEVER** work directly on `main` branch
- Commit messages: concise, focused on "why" not "what"
- Always create NEW commits — never amend unless explicitly asked
- Branch naming: `feat/`, `fix/`, `refactor/`, `docs/` prefixes

## Schema-First Development

- When adding or modifying an artifact, update the JSON Schema FIRST
- Validate that existing tests still pass against the updated schema
- Then update the Rust types to match the schema
- Then update the business logic
- Schema changes require an ADR if they break backward compatibility

## Crate Boundary Discipline

- `Intently_core` MUST NOT depend on `Intently_cli` or `apps/desktop`
- `Intently_cli` depends on `Intently_core` only via public API
- `apps/desktop` (Tauri) depends on `Intently_core` only via public API
- Cross-crate changes require explicit justification
- Shared types live in `Intently_core` and are re-exported

## Code Review Checklist

Before submitting or approving code, validate against principles:

| # | Check | Principle |
|---|-------|-----------|
| 1 | Am I 95%+ confident about this? | Confidence |
| 2 | Is the previous task 100% complete? | Completeness |
| 3 | Does a Rust crate already solve this? | Don't Reinvent |
| 4 | Do I need this now or am I anticipating? | YAGNI |
| 5 | Is there a simpler solution? | KISS |
| 6 | Am I duplicating business logic? | DRY |
| 7 | Does this module have more than one reason to change? | SRP |
| 8 | Does business logic have unit tests? | Testing |
| 9 | Are errors handled explicitly with typed errors? | Error Handling |
| 10 | Are all artifacts schema-validated? | Schema-First |
| 11 | Are crate boundaries respected? | Crate Discipline |
