# Task Protocol Review

Review Task Protocol and LLM orchestration code in Intently.

## Trigger

Activate when PRs or changes touch:
- `crates/Intently_core/src/planner/` (any file)
- Sandbox isolation code
- LLM integration or orchestration logic
- `action_plan.json` schema definitions

Keywords: "task protocol review", "review planner", "LLM orchestration", "sandbox review", "task protocol"

## What This Skill Does

1. **Task Constraints** — Verify constraints are enforced on LLM actions
   - Every LLM action is scoped to a specific task with defined boundaries
   - File access is limited to task-relevant paths
   - No unbounded or open-ended LLM actions (every action has a termination condition)
   - Token/cost budgets are enforced per task

2. **Sandbox Isolation** — Validate sandbox boundaries
   - LLM-generated code runs in an isolated environment
   - No access to credentials, secrets, or environment variables
   - Filesystem access is restricted to workspace scope
   - Network access is denied or explicitly allowlisted

3. **Output Validation** — Ensure LLM outputs are validated before application
   - Generated code is parsed and validated before writing to disk
   - Generated configs are schema-validated
   - All outputs pass through the policy engine before acceptance
   - Malformed outputs are rejected with clear error messages

4. **Action Plan Compliance** — Validate `action_plan.json` schema
   - Plan includes: steps, expected outcomes, rollback strategy
   - Each step has a precondition and postcondition
   - Plan is reviewable by the user before execution

5. **No Unconstrained Actions** — Verify safety boundaries
   - No `rm -rf`, no force pushes, no destructive operations without confirmation
   - All shell commands are allowlisted, not blocklisted
   - Escalation path exists for actions outside sandbox scope

## What to Check

- [ ] Every LLM action is scoped to a task with boundaries
- [ ] Sandbox restricts filesystem, network, and credential access
- [ ] LLM outputs are validated before application
- [ ] `action_plan.json` conforms to schema
- [ ] No destructive operations without explicit user confirmation
- [ ] Shell commands use allowlist approach
- [ ] Error handling for LLM failures (timeout, malformed output, refusal)
- [ ] Unit tests cover constraint enforcement and sandbox violations

## Output Format

```
## Task Protocol Review: <file_path>

### Task Constraints
- [PASS/FAIL] <detail>

### Sandbox Isolation
- [PASS/FAIL] <detail>

### Output Validation
- [PASS/FAIL] <detail>

### Action Plan Schema
- [PASS/FAIL] <detail>

### Safety Boundaries
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
