# DevOps & CI Rules

Rules for CI/CD pipeline, workspace management, Docker, and release processes.

## Monorepo Structure
- Cargo workspace defined in root `Cargo.toml`
- Workspace members: `crates/Intently_core`, `crates/Intently_cli`, `apps/desktop/src-tauri`
- Shared dependencies pinned in workspace `[dependencies]` section
- Frontend (`apps/desktop`) uses npm/pnpm with its own `package.json`
- Schemas live in `schemas/` at the repository root

## Task Runner — `just` (Justfile)
- `just check`: format + lint + typecheck (Rust + frontend)
- `just test`: all tests (cargo test + vitest)
- `just test-unit`: unit tests only (fast, for development)
- `just build`: build all crates and frontend
- `just ci`: full pipeline (check + test + coverage + security)
- `just proto`: regenerate any protobuf/schema bindings
- `just serve`: start desktop app in development mode
- All targets MUST work from a clean checkout after dependency installation

## CI Pipeline (GitHub Actions)
- Trigger: push to `main`, pull requests to `main`
- Steps (in order):
  1. `cargo fmt --all --check` — formatting
  2. `cargo clippy --workspace -- -D warnings` — linting
  3. `cargo test --workspace` — all Rust tests
  4. `cargo tarpaulin --workspace --out xml` — coverage >= 80%
  5. `cd apps/desktop && npm ci && npx vitest run` — frontend tests
  6. `cd apps/desktop && npx vitest run --coverage` — frontend coverage >= 80%
  7. `cargo tauri build` — verify Tauri build succeeds (release only)
- All steps MUST pass before merge
- Use `cargo` caching for faster CI runs (`actions/cache` with `target/` and registry)

## Docker
- Multi-stage builds for CI runner images
- Builder stage: Rust toolchain + Node.js for full build
- Runtime stage: minimal image for CLI distribution
- NEVER install dev dependencies in production images
- Pin base image versions (e.g., `rust:1.78-slim`, not `rust:latest`)

## Releases
- Version source of truth: `Cargo.toml` workspace version
- Versions synchronized: all crates share the workspace version
- Frontend version in `apps/desktop/package.json` matches workspace
- CHANGELOG.md updated before release — move `[Unreleased]` to versioned section
- Tag format: `vX.Y.Z`
- Semantic Versioning strictly followed
- Schema changes that break backward compatibility require major version bump

## Dependency Management
- Rust: `cargo update` for patch updates, manual review for minor/major
- Frontend: `npm audit` / `pnpm audit` for vulnerability scanning
- Audit new dependencies before adding: license, maintenance, transitive deps
- `cargo deny` for license and vulnerability checking in CI

## Observability
- `tracing` + `tracing-subscriber` for structured logging in all Rust crates
- Log format: JSON in production, human-readable in development
- Key context fields: `session_id`, `operation`, `crate`, `duration_ms`
- Performance-critical paths: instrument with `tracing::instrument` for timing
