---
name: reviewing-semantic-diff
description: Reviews semantic diff implementation in intently-core. Validates that diffs operate on the CodeModel (not raw text), checks IR comparison correctness, false positive/negative analysis, and performance. Use when changes touch src/model/diff.rs, SemanticDiff types, or CodeModel comparison logic.
---

# Semantic Diff Review

## Critical rules

**ALWAYS:**
- Operate on the CodeModel IR, never on raw source text — this is a semantic diff, not a textual diff
- Produce empty diffs for cosmetic-only changes (whitespace, comments, formatting, import reordering)
- Capture ALL behavioral changes: added/removed components, changed contracts, new/removed dependencies
- Detect renames and moves as such — not as remove + add (avoids false positives)
- Verify algorithm complexity is O(n log n) or better for typical cases

**NEVER:**
- Diff raw source text or line-based content — always diff the CodeModel IR structures
- Produce spurious diffs for changes that don't affect behavior (false positives)
- Miss a behavioral change that should have been captured (false negatives are the worst bug here)
- Allow quadratic blowup on large codebases — diff must scale linearly or linearithmically
- Modify `SemanticDiff` output types without updating all consumers in `engine.rs`

## Key context

- **File**: `src/model/diff.rs` — SemanticDiff computation
- **Core invariant**: Diffs operate on the CodeModel IR, not raw source text. Whitespace, formatting, and comment-only changes MUST produce empty diffs.
- **Output type**: `SemanticDiff` with added/removed/modified components, changed contracts, new dependencies

## Checklist

- [ ] Diff operates on CodeModel IR, not on raw text
- [ ] All IR element types are compared (components, interfaces, dependencies, sinks, symbols)
- [ ] Cosmetic changes (whitespace, comments, formatting) produce empty diffs
- [ ] Behavioral changes are always captured — no false negatives
- [ ] No false positives (spurious changes flagged as behavioral)
- [ ] Renames and moves are detected correctly (not reported as remove + add)
- [ ] Algorithm is at most O(n log n) for typical cases — no quadratic blowup

## Output format

```
## Semantic Diff Review: <file_path>

### Findings
- [PASS/FAIL] <category>: <detail>

### False Positive/Negative Risk: LOW / MEDIUM / HIGH

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
