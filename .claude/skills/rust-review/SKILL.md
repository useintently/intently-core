# Rust Review

Rust-specific code review for Intently core engine crates.

## Trigger

Activate when PRs or changes touch `.rs` files, `Cargo.toml`, or Rust-specific tooling.

Keywords: "rust review", "review rust", "clippy", "rust code"

## What This Skill Does

1. **Idiomatic Rust** — Verify code follows Rust conventions
   - Use iterators over manual indexing where appropriate
   - Prefer `&str` over `String` for function parameters when ownership is not needed
   - Use `impl Trait` for return types when appropriate
   - Pattern matching is exhaustive (no catch-all `_` hiding new variants)

2. **Clippy Pedantic** — Ensure clippy pedantic compliance
   - No clippy warnings (`#[allow(clippy::...)]` requires justification comment)
   - Pedantic lints enabled at crate level
   - Known false positives documented with `#[allow]` + comment

3. **Error Handling** — Validate error strategy
   - Library crates use `thiserror` for typed domain errors
   - Application/CLI code may use `anyhow` for convenience
   - No `unwrap()` or `expect()` in library code (only in tests and main)
   - Error types implement `Display` with context-rich messages
   - Error propagation uses `?` operator, not manual match

4. **Ownership and Lifetimes** — Check correctness
   - No unnecessary `.clone()` (especially in hot paths)
   - Lifetime annotations are correct and minimal
   - Borrowed data does not outlive its owner
   - Smart pointers (`Arc`, `Rc`, `Box`) justified and not overused

5. **Trait Design** — Review trait definitions
   - Traits have focused, cohesive method sets (ISP)
   - Default implementations are sensible and documented
   - Trait bounds are minimal (`T: Clone + Send` only when needed)
   - Object safety considered for `dyn Trait` usage

6. **Module Organization** — Check structure
   - `mod.rs` or named modules with clear hierarchy
   - `pub` visibility is minimal (only what consumers need)
   - Re-exports in `lib.rs` form a clean public API
   - Internal modules use `pub(crate)` or `pub(super)`

7. **Documentation** — Verify public items are documented
   - All `pub` functions, types, and traits have `///` doc comments
   - Doc comments include examples for non-trivial APIs
   - `#![warn(missing_docs)]` at crate level

## What to Check

- [ ] Code is idiomatic Rust (iterators, pattern matching, ownership)
- [ ] Clippy pedantic passes without unjustified suppression
- [ ] Error handling uses thiserror, no unwrap in library code
- [ ] No unnecessary clones, correct lifetime usage
- [ ] Traits are focused and minimal
- [ ] Module visibility is restrictive (`pub(crate)` by default)
- [ ] Public items have doc comments

## Output Format

```
## Rust Review: <file_path>

### Idiomatic Rust
- [PASS/FAIL] <detail>

### Error Handling
- [PASS/FAIL] <detail>

### Ownership/Lifetimes
- [PASS/FAIL] <detail>

### Trait Design
- [PASS/FAIL] <detail>

### Documentation
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
