# Quality Standards

Non-negotiable quality rules for all intently-core contributions.

## Testing — If There's No Test, It Doesn't Work

- ALL business logic MUST have unit tests. No exceptions
- Test behavior, not implementation — tests that break on every refactor are worthless
- One test tests ONE thing. If the name has "and", split it
- Tests MUST be deterministic. Flaky test = bug with max priority
- Tests MUST be independent — no shared state, no execution order dependency
- Bug fix flow: write failing test FIRST, then fix the bug (regression test)

### Test Pyramid
```
    / Integration \    <- Moderate: cross-module, full extraction pipeline
   /----------------\
  /    Unit Tests    \  <- Many: isolated logic, fast, deterministic
```

### Rust Tests
- Unit tests: `#[cfg(test)] mod tests` in each module
- Integration tests: `tests/` directory at crate root
- Run during development: `cargo test --lib`
- Property-based: `proptest` for parsers, transformers, diffing
- Benchmarks: `criterion` in `benches/` for hot paths
- Coverage: `cargo-tarpaulin` with target >= 80%

### What to Test vs NOT Test
- DO test: extraction pipeline, twin building, semantic diff, symbol extraction, call graph, knowledge graph, language-specific extractors
- DON'T test: serde derives, third-party crate internals, trivial getters

## Error Handling — Fail Fast, Fail Loud, Fail Clear

- NEVER swallow errors. `let _ = result;` discarding Result is forbidden
- Validate inputs at system boundaries (public API methods, file parsing)
- Use typed domain errors with `thiserror`
- Differentiate recoverable (retry, fallback) from irrecoverable (fail immediately)
- NEVER use `panic!` in library code — reserve for truly impossible states
- NEVER return magic values (`-1`, `None`, empty string) to indicate errors
- Tracing spans MUST have enough context to reproduce without a debugger

## Naming
- Choose the most specific, descriptive name. Long and clear > short and ambiguous
- `twin_component` not `component`, `diff_entry` not `entry`, `extraction_result` not `result`
- Rust: `snake_case` functions, `PascalCase` types, `UPPER_CASE` constants

## Linting & Formatting
- Rust: `cargo clippy -- -D warnings` with pedantic lints, zero warnings
- Rust: `cargo fmt --check` with project `.rustfmt.toml`

## Changelog
- ALL visible changes go in `CHANGELOG.md` under `[Unreleased]`
- Format: Keep a Changelog + Semantic Versioning
- Categories (this order): Added, Changed, Deprecated, Removed, Fixed, Security
- Every entry MUST reference ticket/issue/PR: `(#142)`
- Write for consumers, not developers
- NEVER edit released version entries
