# Code Review

General code review for Rust and TypeScript in the Intently IDE project.

## Trigger

Activate for any PR or code change that does not match a more specific review skill.

Keywords: "code review", "review code", "review PR", "review changes", "/review"

## What This Skill Does

1. **Correctness** — Verify the code does what it claims
   - Logic matches the stated intent (PR description, ticket, commit message)
   - Edge cases are handled (empty inputs, overflows, None/null values)
   - Off-by-one errors, boundary conditions checked

2. **Clarity** — Ensure code is readable and maintainable
   - Functions are small and focused (SRP)
   - Names are specific and descriptive (no `data`, `info`, `temp`, `handle`)
   - Complex logic has explanatory comments on the "why", not the "what"
   - No clever one-liners that sacrifice readability (KISS)

3. **Error Handling** — Validate error management
   - Rust: uses `Result<T, E>` with `thiserror` for domain errors
   - TypeScript: uses typed error handling, not bare `catch(e)`
   - No panics in library code (Rust: `unwrap()`, `expect()` only in tests/CLI entry)
   - Errors propagate with context, not swallowed silently

4. **Naming** — Check naming conventions
   - Rust: `snake_case` functions, `PascalCase` types, `UPPER_CASE` constants
   - TypeScript: `camelCase` functions/variables, `PascalCase` types/components
   - File names: `snake_case.rs`, `kebab-case.ts`/`PascalCase.tsx`

5. **Test Coverage** — Verify tests exist for new/changed logic
   - New business logic has unit tests
   - Bug fixes include regression tests
   - Tests follow AAA pattern with descriptive names

6. **Unsafe Blocks** — Rust-specific: justify every `unsafe`
   - Safety invariant documented in a `// SAFETY:` comment
   - Minimal scope (smallest possible unsafe block)
   - Alternative safe approach considered and rejected with reason

7. **Principles Check** — Apply KISS, YAGNI, DRY, SOLID
   - No premature abstractions (YAGNI)
   - No duplicated business logic (DRY)
   - No unnecessary complexity (KISS)

## What to Check

- [ ] Logic matches stated intent
- [ ] Edge cases handled
- [ ] Error handling is explicit and typed
- [ ] No panics in library code
- [ ] Names are specific and follow conventions
- [ ] Tests cover new/changed behavior
- [ ] Unsafe blocks justified with SAFETY comments
- [ ] KISS/YAGNI/DRY principles respected

## Output Format

```
## Code Review: <file_path>

### Summary
<1-2 sentence overview>

### Issues Found
- [CRITICAL/MAJOR/MINOR] <description> (line X)

### Suggestions
- <optional improvement suggestions>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
