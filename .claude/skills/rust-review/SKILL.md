---
name: reviewing-rust-code
description: Validates Rust code against intently-core conventions. Checks idiomatic patterns, thiserror error handling, clippy pedantic compliance, ownership correctness, trait design, module visibility, and public documentation. Use when reviewing .rs files, Cargo.toml changes, or when asked for a Rust-specific review.
---

# Rust Code Review

## Critical rules

**ALWAYS:**
- Use `thiserror` for error types in library code — every error enum lives in `error.rs`
- Use `pub(crate)` as the default visibility — only promote to `pub` what consumers need
- Use `?` operator for error propagation — never manual `match` on Result just to rewrap
- Justify every `#[allow(clippy::...)]` with an adjacent comment explaining why
- Add `///` doc comments with `# Examples` on all `pub` items

**NEVER:**
- Use `anyhow` in library code — it erases error types (reserved for CLI/binary crates)
- Use `unwrap()` or `expect()` outside of tests and provably infallible initialization
- Use catch-all `_` in match arms that hides new enum variants — match exhaustively
- Accept `String` parameters when `&str` suffices — avoid forcing callers to allocate
- Suppress clippy lints without a justification comment — no silent `#[allow]`

## Checklist

- [ ] Idiomatic Rust (iterators over indexing, `&str` params, exhaustive matching)
- [ ] Clippy pedantic passes without unjustified suppression
- [ ] Error types use `thiserror` with context-rich messages
- [ ] No unnecessary `.clone()` in hot paths, minimal lifetime annotations
- [ ] Traits are focused and cohesive (ISP)
- [ ] Module visibility is restrictive (`pub(crate)` default)
- [ ] All `pub` items have doc comments

## Output format

```
## Rust Review: <file_path>

### Findings
- [PASS/FAIL] <category>: <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
