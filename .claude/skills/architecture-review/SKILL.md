# Architecture Review

Review architecture decisions, crate boundaries, and dependency structure in Intently.

## Trigger

Activate when PRs or changes:
- Add new crates or modules
- Modify `Cargo.toml` dependency graphs
- Introduce new abstractions, traits, or interfaces
- Include or reference ADRs (Architecture Decision Records)
- Change the boundary between core engine and IDE

Keywords: "architecture review", "review architecture", "ADR", "crate boundary", "dependency review"

## What This Skill Does

1. **Crate Boundaries** — Verify separation of concerns across crates
   - `Intently_core` contains domain logic, no UI dependencies
   - `apps/desktop` depends on `Intently_core`, not the reverse
   - Each crate has a focused responsibility (SRP at crate level)
   - Public API surface (`pub` items) is minimal and intentional

2. **Dependency Direction** — Validate dependency flow
   - Dependencies flow inward: UI -> Core -> Domain
   - Domain types do not depend on infrastructure
   - No circular dependencies between crates
   - Check with `cargo tree` or workspace dependency analysis

3. **Abstraction Justification** — Ensure new abstractions earn their place
   - New trait/interface has 2+ concrete use cases (not speculative)
   - Abstraction simplifies the code, not complicates it (KISS)
   - If only one implementor exists, question whether the trait is needed (YAGNI)

4. **Schema Versioning** — Check data schemas are versioned
   - JSON schemas include a version field
   - Breaking schema changes bump major version
   - Migration path documented for schema upgrades

5. **ADR Documentation** — Verify decisions are recorded
   - Significant architectural decisions have an ADR
   - ADR includes: context, decision, consequences, alternatives rejected
   - ADRs are in a discoverable location

## What to Check

- [ ] Crate boundaries respect separation of concerns
- [ ] Dependency direction is inward (UI -> Core -> Domain)
- [ ] No circular dependencies
- [ ] New abstractions have 2+ concrete use cases
- [ ] Schemas include version fields
- [ ] Significant decisions documented as ADRs
- [ ] Public API surface is minimal

## Output Format

```
## Architecture Review: <scope>

### Crate Boundaries
- [PASS/FAIL] <detail>

### Dependency Direction
- [PASS/FAIL] <detail>

### Abstraction Justification
- [PASS/FAIL] <detail>

### Schema Versioning
- [PASS/FAIL] <detail>

### ADR Coverage
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
