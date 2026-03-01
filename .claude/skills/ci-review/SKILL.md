# CI Review

CI/CD pipeline review for the Intently IDE project.

## Trigger

Activate when PRs or changes touch:
- `.github/workflows/` (GitHub Actions)
- `justfile` or `Makefile`
- Docker files (`Dockerfile`, `docker-compose.yml`)
- CI configuration (`.cargo/config.toml` CI-related sections)

Keywords: "CI review", "review CI", "pipeline review", "review workflow", "review justfile"

## What This Skill Does

1. **Quality Gates** — Verify all gates are present and ordered
   - `cargo fmt --check` (formatting)
   - `cargo clippy -- -D warnings` (linting)
   - `cargo test` (unit + integration tests)
   - Coverage report with minimum threshold
   - `cargo audit` (security vulnerabilities)
   - Frontend: `npm run lint`, `npm run typecheck`, `npm run test`

2. **Caching** — Check build cache effectiveness
   - Cargo registry and target directories are cached
   - Node modules are cached by lockfile hash
   - Cache keys include lockfile hash and rust toolchain version
   - Cache is invalidated correctly on dependency changes

3. **Determinism** — Ensure pipeline is reproducible
   - Toolchain versions are pinned (rust-toolchain.toml, .node-version)
   - Dependency versions locked (Cargo.lock, package-lock.json committed)
   - No floating tags in Docker base images (use digest or exact version)
   - Tests do not depend on external services without mocking

4. **Secrets Safety** — Verify no secrets in logs
   - Secrets use GitHub Actions secrets mechanism
   - No `echo $SECRET` or debug output of sensitive values
   - Artifacts do not contain credentials or tokens
   - PR workflows from forks cannot access secrets

5. **Build Reproducibility** — Check clean-build correctness
   - All justfile/Makefile targets work from clean checkout
   - `cargo build --release` produces consistent output
   - `cargo tauri build` completes without manual steps
   - Proto generation is checked in or deterministically generated in CI

6. **Pipeline Efficiency** — Review execution time
   - Independent steps run in parallel (matrix or parallel jobs)
   - Expensive steps (full test suite, builds) only run when relevant files change
   - Fail-fast: cheap checks (fmt, lint) run before expensive ones (test, build)
   - Timeout configured on all jobs

## What to Check

- [ ] All quality gates present (fmt, clippy, test, coverage, audit)
- [ ] Caching is effective and correctly invalidated
- [ ] Toolchain and dependency versions are pinned
- [ ] No secrets in logs or artifacts
- [ ] All targets work from clean checkout
- [ ] Pipeline is parallelized and fail-fast
- [ ] Timeouts are set on all jobs

## Output Format

```
## CI Review: <file_path>

### Quality Gates
- [PASS/FAIL] <detail>

### Caching
- [PASS/FAIL] <detail>

### Determinism
- [PASS/FAIL] <detail>

### Secrets Safety
- [PASS/FAIL] <detail>

### Pipeline Efficiency
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
