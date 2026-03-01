# Workflow Rules

Behavioral rules that govern how we work on the intently-core codebase.

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
- All public API methods work correctly
- Tests pass for all modified code
- No regressions in existing tests

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
