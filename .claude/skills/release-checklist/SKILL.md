---
name: releasing-versions
description: Coordinates and validates intently-core releases. Walks through quality gates (fmt, clippy, test, coverage, audit), changelog update, version bump, build verification, tagging, and post-release steps. Use when preparing, validating, or executing a release.
---

# Release Checklist

## Critical rules

**ALWAYS:**
- Run ALL quality gates (`fmt`, `clippy`, `test`, `tarpaulin`, `audit`) — no exceptions, no shortcuts
- Move every `[Unreleased]` entry to the versioned section — nothing left behind
- Reference a ticket/PR in every changelog entry: `(#142)` — untracked changes don't exist
- Follow semver strictly: breaking public API or CodeModel type changes = MAJOR bump
- Create an annotated git tag (`git tag -a`) — lightweight tags are not allowed for releases

**NEVER:**
- Release with any failing test, clippy warning, or known vulnerability — ABORT and fix first
- Edit entries of already-released versions in CHANGELOG.md — create a new entry instead
- Skip the `cargo build --release` verification — debug builds hide optimization-related bugs
- Tag on `main` branch without all checks passing — tag only after full gate validation
- Use `cargo publish` without verifying the tag exists and matches `Cargo.toml` version

Copy this checklist and track progress:

```
Release Progress:
- [ ] Step 1: Quality gates
- [ ] Step 2: Changelog update
- [ ] Step 3: Version bump
- [ ] Step 4: Final verification
- [ ] Step 5: Tag and push
- [ ] Step 6: Post-release
```

## Step 1: Quality gates (must all pass)

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo tarpaulin --out html   # coverage >= 80%
cargo audit
```

If any gate fails: **ABORT**. Fix the issue and restart from Step 1.

## Step 2: Changelog update

- Move `[Unreleased]` entries to `[X.Y.Z] - YYYY-MM-DD` section
- Every entry references a ticket/PR: `(#142)`
- Categories in order: Added, Changed, Deprecated, Removed, Fixed, Security
- Add new empty `[Unreleased]` section at top

## Step 3: Version bump

Update version in `Cargo.toml` following semver:
- **MAJOR**: breaking changes (public API, CodeModel types)
- **MINOR**: new features, backward-compatible
- **PATCH**: bug fixes only

## Step 4: Final verification

```bash
cargo build --release
cargo doc --no-deps
```

Verify no `[Unreleased]` entries remain.

## Step 5: Tag and push

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

## Step 6: Post-release

- Create GitHub Release with changelog excerpt
- Verify crate publishes to crates.io (if applicable)

## Output format

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

### Decision: PROCEED / ABORT
```
