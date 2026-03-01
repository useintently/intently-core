# Quality Standards

Non-negotiable quality rules for all Intently IDE contributions.

## Testing — If There's No Test, It Doesn't Work

- ALL business logic MUST have unit tests. No exceptions
- Test behavior, not implementation — tests that break on every refactor are worthless
- One test tests ONE thing. If the name has "and", split it
- Tests MUST be deterministic. Flaky test = bug with max priority
- Tests MUST be independent — no shared state, no execution order dependency
- Bug fix flow: write failing test FIRST, then fix the bug (regression test)

### Test Pyramid
```
      /  E2E  \        <- Few: critical end-to-end flows only
     /----------\
    / Integration \    <- Moderate: Tauri commands, cross-crate, IPC
   /----------------\
  /    Unit Tests    \  <- Many: isolated logic, fast, deterministic
```

### Rust Tests
- Unit tests: `#[cfg(test)] mod tests` in each module
- Integration tests: `tests/` directory at crate root
- Run during development: `cargo test -p Intently_core --lib`
- Property-based: `proptest` for parsers, transformers, diffing
- Benchmarks: `criterion` in `benches/` for hot paths
- Coverage: `cargo-tarpaulin` with target >= 80% per crate

### Frontend Tests
- Vitest + React Testing Library for component behavior
- Coverage target: 80% for `src/` (excluding generated bindings)
- Mock Tauri IPC in tests — never depend on running Tauri backend
- Test names describe behavior: `it("shows error when intent validation fails")`

### What to Test vs NOT Test
- DO test: IR construction, semantic diff, policy evaluation, evidence collection, intent parsing, planner logic
- DON'T test: serde derives, generated proto/schema code, third-party crate internals, trivial getters

## Error Handling — Fail Fast, Fail Loud, Fail Clear

- NEVER swallow errors. `let _ = result;` discarding Result is forbidden
- Validate inputs at system boundaries (CLI args, IPC commands, file parsing)
- Use typed domain errors per crate with `thiserror`
- Differentiate recoverable (retry, fallback) from irrecoverable (fail immediately)
- NEVER use `panic!` in library code — reserve for truly impossible states
- NEVER return magic values (`-1`, `None`, empty string) to indicate errors
- Tracing spans MUST have enough context to reproduce without a debugger

## Naming
- Choose the most specific, descriptive name. Long and clear > short and ambiguous
- `intent_policy_id` not `id`, `twin_component` not `component`, `diff_entry` not `entry`
- Rust: `snake_case` functions, `PascalCase` types, `UPPER_CASE` constants
- TypeScript: `camelCase` functions, `PascalCase` components/types

## Linting & Formatting
- Rust: `cargo clippy -- -D warnings` with pedantic lints, zero warnings
- Rust: `cargo fmt --check` with project `.rustfmt.toml`
- TypeScript: ESLint strict config, zero warnings
- TypeScript: Prettier for formatting

## Changelog
- ALL visible changes go in `CHANGELOG.md` under `[Unreleased]`
- Format: Keep a Changelog + Semantic Versioning
- Categories (this order): Added, Changed, Deprecated, Removed, Fixed, Security
- Every entry MUST reference ticket/issue/PR: `(#142)`
- Write for consumers, not developers
- NEVER edit released version entries
