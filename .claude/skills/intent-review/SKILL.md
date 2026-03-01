# Intent Review

Review changes to `intent.yaml` and intent-related schemas in the Intently IDE project.

## Trigger

Activate when PRs or changes touch:
- `intent.yaml` files anywhere in the project
- Intent schema definitions in `crates/Intently_core/src/`
- Files matching `*intent*` in core crates

Keywords: "intent review", "intent.yaml", "review intent", "intent schema"

## What This Skill Does

1. **Schema Compliance** — Validate that `intent.yaml` conforms to the declared schema version
   - Required top-level fields are present (name, version, components, invariants, policies)
   - Field types match schema expectations
   - No unknown or deprecated fields

2. **Policy References** — Verify all `policy_id` references point to existing policy definitions
   - Cross-reference against `crates/Intently_core/src/policy/` definitions
   - Flag dangling references (policy ID used but not defined)
   - Flag unused policies (defined but never referenced)

3. **Invariant Definitions** — Ensure invariants are testable
   - Each invariant has a clear predicate that can be evaluated programmatically
   - Invariants reference valid components/flows in the System Twin
   - No vague or unmeasurable invariants (e.g., "system should be fast")

4. **Evidence Requirements** — Confirm evidence requirements are achievable
   - Evidence types referenced exist in the evidence engine
   - Required coverage levels are realistic for the component scope
   - No circular evidence dependencies

5. **Backward Compatibility** — Check for breaking changes
   - Removed fields flagged as breaking
   - Changed semantics documented in CHANGELOG.md
   - Version bumped appropriately if breaking

## What to Check

- [ ] Schema version declared and valid
- [ ] All policy IDs resolve to existing definitions
- [ ] All invariants have testable predicates
- [ ] Evidence requirements reference valid evidence types
- [ ] No breaking changes without version bump
- [ ] CHANGELOG.md updated if intent schema changed

## Output Format

```
## Intent Review: <file_path>

### Schema Compliance
- [PASS/FAIL] <detail>

### Policy References
- [PASS/FAIL] <detail>

### Invariant Definitions
- [PASS/FAIL] <detail>

### Evidence Requirements
- [PASS/FAIL] <detail>

### Breaking Changes
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
