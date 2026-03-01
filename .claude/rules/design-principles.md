# Design Principles

Non-negotiable design principles for all Intently IDE contributions, adapted for Rust and the Intently domain.

## Don't Reinvent the Wheel
- Before implementing anything non-trivial, check if a mature Rust crate already solves it
- Preferred crates: serde, tokio, tracing, thiserror, anyhow, clap, uuid, jsonschema, similar
- Encapsulate third-party dependencies behind domain traits (DIP) for swappability
- NEVER implement your own: JSON parser, schema validator, diff algorithm, async runtime
- Evaluate crates by: maintenance activity, license (MIT/Apache 2.0), transitive deps, CI visibility

## KISS — Keep It Simple
- Prefer explicit code over "clever" code — Rust's type system already provides safety
- If a module needs a diagram to be understood, simplify it
- Use the simplest solution: a function before a trait, a trait before a macro
- No proc-macro magic unless the ergonomic gain is massive and well-documented
- Avoid deep generic nesting — if the type signature is unreadable, the design needs rethinking

## YAGNI — You Aren't Gonna Need It
- Implement only what the current iteration requires
- Don't create traits for a single implementor — add the trait when the second implementation arrives
- Don't add config parameters nobody asked for
- Don't build multi-backend support "just in case"
- Ask: "Do we have 2+ concrete cases that justify this abstraction?" If not, don't abstract

## DRY — Don't Repeat Yourself
- DRY applies to knowledge and business logic, NOT lines of code
- Rule of 3: extract only when the same knowledge appears a third time
- Centralize shared types in crate-level `types.rs` or `models.rs` modules
- Centralize constants in dedicated modules (e.g., `constants.rs`)
- NEVER force DRY when it creates fragile coupling between unrelated crates

## SOLID (Adapted for Rust)

### SRP — Single Responsibility
- Each crate has ONE domain: `Intently_core` (engine), `Intently_cli` (interface), `apps/desktop` (IDE)
- Each module has ONE reason to change, owned by ONE domain area
- Tauri commands only route — business logic goes to core engine crate
- If you need "and" to describe what a module does, split it

### OCP — Open/Closed
- Prefer composition over inheritance (Rust's natural model)
- New policy checkers: implement `PolicyChecker` trait, zero changes to engine
- New evidence runners: implement `EvidenceRunner` trait, register in config
- Extension points use trait objects (`Box<dyn Trait>`) or enums with `#[non_exhaustive]`

### LSP — Liskov Substitution
- All trait implementations must honor the trait's documented contract
- Never return `unimplemented!()` or `todo!()` in released trait implementations
- If a type cannot fulfill a trait fully, it should not implement that trait

### ISP — Interface Segregation
- Small, focused traits: `PolicyChecker`, `EvidenceRunner`, `DiffComputer`, `TwinBuilder`
- Consumers depend only on the traits they use — not on a monolithic engine interface
- Prefer multiple small traits over one large trait with optional methods

### DIP — Dependency Inversion
- Core engine defines traits; infrastructure and plugins implement them
- CLI and IDE depend on core abstractions, never on concrete implementations
- Config uses typed structs (serde) driven by files and environment
- Test doubles implement the same traits as production code

## Tension Resolution
When principles conflict, resolve in this priority order:
1. **KISS** — simplicity wins when complexity adds no value
2. **YAGNI** — don't build what's not needed yet
3. **DRY** — eliminate duplicated knowledge, not duplicated code
4. **SOLID** — apply where complexity is real, not speculative
5. **Don't Reinvent** — use crates for real problems, not speculative ones
