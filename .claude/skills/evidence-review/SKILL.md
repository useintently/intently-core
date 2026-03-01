# Evidence Review

Review evidence engine and test selection logic in Intently.

## Trigger

Activate when PRs or changes touch:
- `crates/Intently_core/src/evidence/` (any file)
- Test selection algorithms or heuristics
- `evidence_report.json` schema definitions

Keywords: "evidence review", "review evidence", "test selection", "evidence engine"

## What This Skill Does

1. **Incremental Evidence** — Verify evidence collection is incremental
   - Only affected components are re-evaluated on change
   - Cache invalidation is correct (no stale evidence)
   - Full re-evaluation is available but not the default path

2. **Impact-Based Selection** — Validate test selection strategy
   - Test selection uses the semantic diff to identify affected components
   - Transitive dependencies are considered (A depends on B, B changed = test A)
   - Selection is neither too broad (test everything) nor too narrow (miss regressions)

3. **Schema Compliance** — Validate `evidence_report.json` output
   - Report includes: tests selected, tests run, results, coverage delta
   - Each evidence entry links back to the invariant it satisfies
   - Report is machine-parseable and versioned

4. **Flaky Evidence Detection** — Check for non-deterministic evidence
   - Tests must be deterministic (same code = same result)
   - No time-dependent, order-dependent, or network-dependent tests in evidence
   - Flaky test detection mechanism exists and is enforced

5. **Completeness** — Verify evidence covers declared invariants
   - Every invariant in `intent.yaml` has at least one evidence source
   - Gaps between invariants and evidence are reported
   - Evidence types are appropriate for the invariant type

## What to Check

- [ ] Evidence collection is incremental, not full-rebuild
- [ ] Test selection is impact-based using semantic diff
- [ ] `evidence_report.json` conforms to schema
- [ ] No flaky or non-deterministic evidence sources
- [ ] Every invariant has mapped evidence
- [ ] Cache invalidation logic is correct
- [ ] Unit tests cover selection edge cases (no deps, circular deps, leaf changes)

## Output Format

```
## Evidence Review: <file_path>

### Incremental Collection
- [PASS/FAIL] <detail>

### Test Selection Strategy
- [PASS/FAIL] <detail>

### Schema Compliance
- [PASS/FAIL] <detail>

### Flaky Evidence Risk
- [LOW/MEDIUM/HIGH] <detail>

### Invariant Coverage
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
