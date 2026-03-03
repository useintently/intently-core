# Implementation Playbook

Step-by-step process for implementing changes in intently-core. All agents follow this playbook when writing code (not just reviewing).

## Standard Implementation Flow

```
Read task → Read code → Implement → Test → Verify → Document → Complete
```

### 1. Understand Before Writing

- Read the task description and acceptance criteria fully
- Read the existing code in the area you're modifying
- Identify which module boundaries are involved (engine, model, parser, search, git, workspace)
- If confidence is below 95%, stop and ask for clarification

### 2. Implement

- Follow `.claude/rules/rust-conventions.md` for all Rust code
- Follow `.claude/rules/design-principles.md` for design decisions
- Prefer editing existing files over creating new ones
- Make the minimal change that satisfies the task — no drive-by refactoring
- Use `pub(crate)` as default visibility — only `pub` what consumers need

### 3. Write Tests

#### Unit tests (in same file)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptive_behavior_name() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

#### Integration tests (in tests/ directory)
- Full pipeline tests go in `tests/full_extraction.rs`
- Use fixtures from `tests/fixtures/` — create new fixture directories for new patterns
- Test the public API surface (IntentlyEngine methods), not internal functions

#### Test naming
- Describe behavior, not method: `rejects_empty_file_path` not `test_validate_1`
- One test tests ONE thing — if the name has "and", split it
- Bug fixes: write the failing test FIRST, then fix the bug

### 4. Verify

Run ALL quality gates before marking any task complete:

```bash
cargo fmt --check          # Formatting correct
cargo clippy -- -D warnings  # No lint warnings
cargo test                 # All tests pass
```

If any gate fails, fix the issue and re-run ALL gates (a clippy fix might break a test).

### 5. Update Documentation

#### CHANGELOG.md
- Add entry under `[Unreleased]` for every user-visible change
- Categories in order: Added, Changed, Deprecated, Removed, Fixed, Security
- Every entry references a ticket/issue/PR: `(#142)` — or `(no-ticket)` for unreferenced work
- Write for the consumer: "Route parameter extraction for Express.js endpoints" — NOT "Added regex to typescript.rs parse_route function"
- Internal refactoring with no external impact: NO changelog entry

#### CLAUDE.md
- Update when adding: new languages, new modules, new public API methods, new workspace formats
- Update the architecture tree, supported languages table, or public API section as applicable

#### ADRs (docs/adrs/)
- Create an ADR when: adding a new module, choosing between alternatives, adding a significant dependency
- Format: `NNN-short-description.md` with Context, Decision, Consequences sections

### 6. Commit

Follow the commit convention:
```
<type>(<scope>): <description>

Types: feat, fix, refactor, docs, test, chore
Scopes: parser, model, extractors, graph, diff, search, engine, workspace, git
```

Examples:
- `feat(extractors): add Elixir Phoenix route extraction`
- `fix(model): prevent duplicate symbols in CodeModel builder`
- `test(graph): add property-based tests for cycle detection`
- `refactor(engine): extract chunked extraction into separate method`

## Common Patterns

### Adding a field to an existing type
1. Add field to struct in `src/model/types.rs` (with `#[serde(default)]` if backward-compatible)
2. Populate field in the relevant extractor(s)
3. Add field to `CodeModelBuilder` aggregation if it's model-level
4. Update `SemanticDiff` if the field affects behavioral comparison
5. Add unit tests for the new field extraction
6. Update integration tests to assert the field value

### Adding a new extractor utility
1. Add function in `src/model/extractors/common.rs`
2. Unit test in the same file's `#[cfg(test)] mod tests`
3. Use from language-specific extractors via `use super::common::*`

### Modifying the public API
1. Change in `src/lib.rs` re-exports or engine.rs public methods
2. Verify downstream impact — this may be a semver MAJOR change
3. Add `///` doc comment with `# Examples` section
4. Update CLAUDE.md public API section
5. Consider `#[non_exhaustive]` on new public enums

## Anti-Patterns (Never Do)

- Do NOT add `unwrap()` in library code — use `?` with proper error types
- Do NOT add `println!` or `eprintln!` — use `tracing` crate
- Do NOT skip tests — "I'll add tests later" means never
- Do NOT change behavior without a test proving the old behavior was wrong
- Do NOT add dependencies without justification in the PR description
- Do NOT create traits with a single implementor — wait for the second case
- Do NOT add config parameters nobody asked for
