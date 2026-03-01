# Architecture Review

Review architecture decisions, module boundaries, and dependency structure in intently-core.

## Trigger

Activate when PRs or changes:
- Add new modules
- Modify `Cargo.toml` dependencies
- Introduce new abstractions, traits, or interfaces
- Include or reference ADRs (Architecture Decision Records)
- Change the public API surface

Keywords: "architecture review", "review architecture", "ADR", "module boundary", "dependency review"

## What This Skill Does

1. **Module Boundaries** — Verify separation of concerns
   - `engine.rs` orchestrates; `twin/` builds IR; `parser/` parses; `search/` searches
   - Each module has a focused responsibility (SRP)
   - Public API surface (`pub` items in `lib.rs`) is minimal and intentional
   - No leaking of internal types through the public API

2. **Dependency Direction** — Validate dependency flow
   - No circular dependencies between modules
   - Core types in `twin/types.rs` are the shared vocabulary
   - Extractors depend on common utilities, not on each other
   - Check with `cargo tree` for external dependency analysis

3. **Abstraction Justification** — Ensure new abstractions earn their place
   - New trait/interface has 2+ concrete use cases (not speculative)
   - Abstraction simplifies the code, not complicates it (KISS)
   - If only one implementor exists, question whether the trait is needed (YAGNI)

4. **ADR Documentation** — Verify decisions are recorded
   - Significant architectural decisions have an ADR in `docs/adrs/`
   - ADR includes: context, decision, consequences, alternatives rejected

## What to Check

- [ ] Module boundaries respect separation of concerns
- [ ] No circular dependencies between modules
- [ ] New abstractions have 2+ concrete use cases
- [ ] Significant decisions documented as ADRs
- [ ] Public API surface is minimal
- [ ] Extraction-only scope is respected (no policy/governance logic)

## Output Format

```
## Architecture Review: <scope>

### Module Boundaries
- [PASS/FAIL] <detail>

### Dependency Direction
- [PASS/FAIL] <detail>

### Abstraction Justification
- [PASS/FAIL] <detail>

### ADR Coverage
- [PASS/FAIL] <detail>

### Scope Discipline
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
