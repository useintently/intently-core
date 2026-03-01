# Rust Conventions

Non-negotiable Rust conventions for all Intently IDE contributions.

## Language & Edition
- Rust 2021 edition required
- Minimum supported Rust version (MSRV) defined in `Cargo.toml` workspace
- All code MUST pass `cargo clippy -- -D warnings` with pedantic lints enabled
- All code MUST be formatted with `rustfmt` using project `.rustfmt.toml`
- Max line length: 120 characters

## Error Handling
- Use `Result<T, E>` everywhere — never return sentinel values
- `thiserror` for domain-specific error types in library crates
- `anyhow` ONLY at application boundaries (CLI, command handlers)
- NEVER `unwrap()` or `expect()` in library code — always propagate with `?`
- `unwrap()` is acceptable ONLY in tests and one-time initialization proven infallible
- Define error enums per crate in a dedicated `error.rs` module
- Error messages must include context: what failed, with what input, what was expected

## Serialization
- `serde` with `serde_json` for all JSON serialization/deserialization
- Derive `Serialize, Deserialize` on all data transfer types
- Use `#[serde(rename_all = "snake_case")]` for consistent field naming
- Use `#[serde(deny_unknown_fields)]` on strict schema types (artifacts)

## Async Runtime
- `tokio` as the async runtime (multi-threaded by default)
- Use `tokio::spawn` for concurrent tasks, `tokio::select!` for racing
- NEVER use `std::thread::sleep` in async code — use `tokio::time::sleep`
- Prefer `tokio::sync` primitives (Mutex, RwLock, mpsc) over `std::sync` in async contexts

## Logging & Tracing
- `tracing` crate for all structured logging — never `println!` or `eprintln!`
- Use spans for request/operation scoping: `#[instrument]` on public functions
- Include context fields: `session_id`, `crate_name`, `operation`
- Log levels: TRACE (verbose debug), DEBUG (development), INFO (operations), WARN (recoverable), ERROR (failures)

## Module Organization
- `lib.rs` re-exports the public API of each crate
- Use `mod.rs` for directory modules
- Default visibility: `pub(crate)` — expose only what is needed
- Group: types, traits, implementations, tests within each module
- One type per file for complex types; related small types can share a file

## Type System
- Prefer newtypes over primitive aliases: `struct SessionId(Uuid)` not `type SessionId = Uuid`
- Use `enum` for state machines with exhaustive matching
- Derive `Debug, Clone` on all public types; add `PartialEq, Eq, Hash` where meaningful
- Use `#[non_exhaustive]` on public enums to allow future extension

## Testing
- Unit tests in `#[cfg(test)] mod tests` at the bottom of each module
- Integration tests in `tests/` directory at crate root
- `proptest` for property-based testing of parsers and transformations
- `criterion` for performance benchmarks in `benches/`
- Test names describe behavior: `fn rejects_intent_with_missing_policy_section()`
- Use `assert_eq!`, `assert_matches!` with descriptive messages

## Documentation
- Document ALL `pub` items with `///` doc comments
- Include `# Examples` section for non-trivial public functions
- Use `//!` module-level docs to explain purpose and usage
- Run `cargo doc --no-deps` to verify documentation builds cleanly

## Dependencies
- Pin major versions in `Cargo.toml`: `serde = "1"`, not `serde = "*"`
- Prefer well-maintained crates: serde, tokio, tracing, thiserror, anyhow, clap, uuid
- Audit new dependencies for maintenance status, license, and transitive deps
- Use workspace dependencies in root `Cargo.toml` to ensure version consistency
