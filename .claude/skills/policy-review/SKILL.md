# Policy Review

Review policy engine code and policy definitions in Intently.

## Trigger

Activate when PRs or changes touch:
- `crates/Intently_core/src/policy/` (any file)
- Policy definition files (`.policy.yaml`, `.policy.json`)
- `policy_report.json` schema definitions

Keywords: "policy review", "review policy", "policy engine", "policy rules"

## What This Skill Does

1. **Determinism** — Verify policy rules produce deterministic results
   - Same input always yields same output (no randomness, no time-dependency)
   - Rule evaluation order does not affect outcome
   - No hidden state between evaluations

2. **False Positive Analysis** — Check for overly broad rules
   - Rules should not flag correct code as violations
   - Identify rules with high false-positive potential
   - Suggest narrowing predicates where appropriate

3. **Override Auditability** — Ensure policy overrides leave a trail
   - Every override must record: who, when, why, approval
   - Override scope is minimal (single rule, single file, not global)
   - No mechanism to silently disable policies

4. **Schema Compliance** — Validate `policy_report.json` output
   - Report includes all evaluated rules with pass/fail status
   - Violations include location, rule ID, severity, and message
   - Report is machine-parseable and versioned

5. **Integration** — Verify policy engine integrates correctly
   - Policies are evaluated at the correct gate in the pipeline
   - Policy failures block progression (not just warnings)
   - Error handling uses typed errors, not panics

## What to Check

- [ ] All policy rules are deterministic
- [ ] No obvious false-positive traps in rule predicates
- [ ] Override mechanism is auditable and scoped
- [ ] `policy_report.json` conforms to schema
- [ ] Policy evaluation is integrated at the correct gate
- [ ] Error handling uses `Result<T, E>` with typed errors
- [ ] Unit tests cover both pass and fail scenarios per rule

## Output Format

```
## Policy Review: <file_path>

### Determinism
- [PASS/FAIL] <detail>

### False Positive Risk
- [LOW/MEDIUM/HIGH] <detail>

### Override Auditability
- [PASS/FAIL] <detail>

### Schema Compliance
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
