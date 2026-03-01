# Semantic Diff Review

Review semantic diff implementation in Intently.

## Trigger

Activate when PRs or changes touch:
- `crates/Intently_core/src/diff/` (any file)
- IR comparison logic
- `semantic_diff.json` schema definitions

Keywords: "semantic diff review", "review diff", "semantic diff", "behavioral diff"

## What This Skill Does

1. **Behavioral vs Textual** — Verify diff is semantic, not line-based
   - Diff operates on the IR (CodeModel), not on raw source text
   - Whitespace, formatting, and comment changes produce empty diffs
   - Renamed symbols with same behavior produce rename-only diffs

2. **IR Comparison Correctness** — Validate IR diff algorithm
   - Components added/removed are detected correctly
   - Contract changes (inputs, outputs, types) are captured
   - Dependency graph changes (new edges, removed edges) are captured
   - Flow changes (control flow, data flow) are captured

3. **False Positive/Negative Analysis** — Check diff accuracy
   - False positives: changes flagged as behavioral when they are cosmetic
   - False negatives: behavioral changes missed by the diff
   - Edge cases: moves, renames, split/merge of components

4. **Schema Compliance** — Validate `semantic_diff.json` output
   - Diff includes: affected components, change type, before/after IR snapshots
   - Each change entry has a severity (breaking, compatible, cosmetic)
   - Report is machine-parseable and versioned

5. **Performance** — Verify diff is efficient
   - Diff is computed incrementally where possible
   - Large codebases do not cause quadratic blowup
   - Diff computation has a timeout/budget mechanism

## What to Check

- [ ] Diff operates on IR, not on raw text
- [ ] All IR element types are compared (components, deps, contracts, flows)
- [ ] Cosmetic changes produce empty or cosmetic-only diffs
- [ ] Behavioral changes are always captured (no false negatives)
- [ ] `semantic_diff.json` conforms to schema
- [ ] Diff algorithm is at most O(n log n) for typical cases
- [ ] Unit tests cover: add, remove, rename, modify, move, split, merge

## Output Format

```
## Semantic Diff Review: <file_path>

### Behavioral Correctness
- [PASS/FAIL] <detail>

### IR Comparison
- [PASS/FAIL] <detail>

### False Positive/Negative Risk
- [LOW/MEDIUM/HIGH] <detail>

### Schema Compliance
- [PASS/FAIL] <detail>

### Performance
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
