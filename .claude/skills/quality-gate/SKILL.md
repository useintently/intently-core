---
name: quality-gate
description: Runs lightweight quality verification after implementing a task. Executes cargo fmt, clippy, and test gates, then reports PASS/FAIL per gate. Use after completing implementation work, before marking a task as done.
---

# Quality Gate

Lightweight verification skill for mid-task and post-task quality checks. Use this instead of `/release-checklist` when you need a quick "does my work pass?" check.

## Critical rules

**ALWAYS:**
- Run ALL three gates in order: fmt → clippy → test
- Report the first failure clearly — don't continue past a failing gate (failures cascade)
- Include the specific error output for any FAIL so the developer can fix it immediately
- Check CHANGELOG.md has a new entry under `[Unreleased]` if the change is user-visible

**NEVER:**
- Skip a gate because the previous one passed — all 3 are mandatory
- Report PASS if any gate has warnings (clippy warnings = FAIL)
- Run only `cargo test --lib` when integration tests exist for the changed area — run full `cargo test`

## Gate sequence

### Gate 1: Formatting
```bash
cargo fmt --check
```
- PASS: no output, exit 0
- FAIL: shows files that need formatting → run `cargo fmt` to fix

### Gate 2: Linting
```bash
cargo clippy -- -D warnings
```
- PASS: no warnings, exit 0
- FAIL: shows clippy diagnostics → fix each warning

### Gate 3: Tests
```bash
cargo test
```
- PASS: all tests pass, exit 0
- FAIL: shows failing tests → fix the failures

### Gate 4: CHANGELOG (advisory)
Check if `CHANGELOG.md` has been modified (if the change is user-visible):
```bash
git diff --name-only | grep CHANGELOG.md
```
- If user-visible change and CHANGELOG not modified → WARN (not a hard failure, but should be addressed)

## Output format

```
## Quality Gate Results

| Gate | Status | Details |
|------|--------|---------|
| cargo fmt | PASS/FAIL | <error details if FAIL> |
| cargo clippy | PASS/FAIL | <warning count if FAIL> |
| cargo test | PASS/FAIL | <failing test names if FAIL> |
| CHANGELOG | PASS/WARN/N/A | <advisory if WARN> |

### Verdict: ALL PASS / BLOCKED (fix gate N first)
```
