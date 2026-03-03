---
name: reviewing-code
description: General code review for intently-core Rust library. Validates correctness, clarity, error handling, naming, test coverage, unsafe blocks, and KISS/YAGNI/DRY principles. Use as the default review skill when no specialized review (rust, performance, security, architecture) is requested.
---

# Code Review

## Critical rules

**ALWAYS:**
- Read the code under review BEFORE writing any feedback — never review from memory or assumptions
- Test every new behavior with at least one unit test following AAA pattern (Arrange-Act-Assert)
- Propagate errors with context using `?` — callers need to know what failed and why
- Write regression tests BEFORE fixing bugs — prove the bug exists, then fix it
- Name things specifically — `route_parameter` not `param`, `file_extraction` not `result`

**NEVER:**
- Approve code with `unwrap()` in library paths — this is a crash in production
- Approve swallowed errors (`let _ = result;`, empty `catch`) — silent failures corrupt data
- Add premature abstractions (trait with 1 implementor, config nobody asked for) — YAGNI
- Duplicate business logic across modules — if changing one requires changing the other, extract
- Comment "what" the code does — only comment "why" when the reason isn't self-evident

## What to validate

1. **Correctness** — Logic matches stated intent, edge cases handled
2. **Clarity** — Functions are small (SRP), names are specific, comments explain "why" not "what"
3. **Error handling** — `Result<T, E>` with `thiserror`, no panics in library code, errors propagate with context
4. **Naming** — `snake_case` functions, `PascalCase` types, `UPPER_CASE` constants. Specific names (not `data`, `info`, `temp`)
5. **Tests** — New logic has unit tests, bug fixes have regression tests, AAA pattern
6. **Unsafe** — Every `unsafe` has a `// SAFETY:` comment, minimal scope, safe alternative considered
7. **Principles** — No premature abstractions (YAGNI), no duplicated business logic (DRY), no unnecessary complexity (KISS)

## Output format

```
## Code Review: <file_path>

### Summary
<1-2 sentence overview>

### Issues Found
- [CRITICAL/MAJOR/MINOR] <description> (line X)

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
