# Release Checklist

Coordinate and validate a release of the intently-core library.

## Trigger

Activate when preparing, validating, or executing a release.

Keywords: "release", "version", "changelog", "publish", "tag", "release checklist"

## What This Skill Does

Walk through the complete release process, verifying each gate before proceeding.

## Pre-Release Checklist

### 1. Quality Gates (must all pass)
- [ ] `cargo fmt --check` -- zero formatting issues
- [ ] `cargo clippy -- -D warnings` -- zero warnings
- [ ] `cargo test` -- all tests pass
- [ ] Coverage meets minimum threshold (>= 80%)
- [ ] `cargo audit` -- no known vulnerabilities

### 2. Changelog Update
- [ ] `CHANGELOG.md` has entries under `[Unreleased]`
- [ ] Move `[Unreleased]` entries to new version section `[X.Y.Z] - YYYY-MM-DD`
- [ ] Every entry references a ticket/issue/PR number
- [ ] Entries are written for consumers, not developers
- [ ] Categories are in correct order: Added, Changed, Deprecated, Removed, Fixed, Security
- [ ] New empty `[Unreleased]` section added at top

### 3. Version Bump
- [ ] Version in `Cargo.toml`
- [ ] Version follows Semantic Versioning:
  - MAJOR: breaking changes (public API, IR types)
  - MINOR: new features, backward-compatible
  - PATCH: bug fixes only

### 4. Final Verification
- [ ] `cargo build --release` succeeds
- [ ] `cargo doc --no-deps` builds cleanly
- [ ] No `[Unreleased]` entries remain (all moved to version)

## Release Execution

### 5. Tag and Push
- [ ] Create annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z"`
- [ ] Push tag: `git push origin vX.Y.Z`

### 6. Post-Release
- [ ] GitHub Release created with changelog excerpt
- [ ] Verify crate publishes to crates.io (if applicable)

## Abort Criteria

Stop the release immediately if:
- Any quality gate fails
- Changelog is incomplete or missing entries
- Build fails

## Output Format

```
## Release: vX.Y.Z

### Gate Status
| Gate | Status | Notes |
|------|--------|-------|
| cargo fmt | PASS/FAIL | |
| cargo clippy | PASS/FAIL | |
| cargo test | PASS/FAIL | |
| coverage | PASS/FAIL | X% |
| cargo audit | PASS/FAIL | |
| changelog | PASS/FAIL | |
| version bump | PASS/FAIL | |
| build | PASS/FAIL | |

### Decision: PROCEED / ABORT
Reason: <explanation>
```
