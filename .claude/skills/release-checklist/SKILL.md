# Release Checklist

Coordinate and validate a release of the Intently IDE project.

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
- [ ] Coverage meets minimum threshold (check CI config)
- [ ] `cargo audit` -- no known vulnerabilities
- [ ] Frontend: `npm run lint && npm run typecheck && npm run test`

### 2. Changelog Update
- [ ] `CHANGELOG.md` has entries under `[Unreleased]`
- [ ] Move `[Unreleased]` entries to new version section `[X.Y.Z] - YYYY-MM-DD`
- [ ] Every entry references a ticket/issue/PR number
- [ ] Entries are written for consumers, not developers
- [ ] Categories are in correct order: Added, Changed, Deprecated, Removed, Fixed, Security
- [ ] New empty `[Unreleased]` section added at top

### 3. Version Bump
- [ ] Version in root `Cargo.toml` (workspace version)
- [ ] Version in `apps/desktop/src-tauri/Cargo.toml` (if separate)
- [ ] Version in `apps/desktop/package.json`
- [ ] Version follows Semantic Versioning:
  - MAJOR: breaking changes (IR schema, intent.yaml schema, public API)
  - MINOR: new features, backward-compatible
  - PATCH: bug fixes only

### 4. Final Verification
- [ ] `cargo build --release` succeeds
- [ ] `cargo tauri build` produces installable artifact
- [ ] Smoke test: open IDE, load a project, verify core flows work
- [ ] No `[Unreleased]` entries remain (all moved to version)

## Release Execution

### 5. Tag and Push
- [ ] Create annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z"`
- [ ] Push tag: `git push origin vX.Y.Z`
- [ ] Verify CI pipeline triggered by tag push

### 6. Post-Release
- [ ] CI builds release artifacts (binary, installer)
- [ ] GitHub Release created with changelog excerpt
- [ ] Announce in relevant channels
- [ ] Verify published artifacts are downloadable and functional

## Abort Criteria

Stop the release immediately if:
- Any quality gate fails
- Changelog is incomplete or missing entries
- Build fails on any supported platform
- Smoke test reveals blocking issues

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
| frontend checks | PASS/FAIL | |
| changelog | PASS/FAIL | |
| version bump | PASS/FAIL | |
| build | PASS/FAIL | |

### Decision: PROCEED / ABORT
Reason: <explanation>
```
