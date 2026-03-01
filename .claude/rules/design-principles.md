# Design Principles

Non-negotiable design principles for all intently-core contributions, adapted for Rust.

## Don't Reinvent the Wheel
- Before implementing anything non-trivial, check if a mature Rust crate already solves it
- Preferred crates: serde, tracing, thiserror, similar, petgraph, tree-sitter, ast-grep-core, rayon, walkdir
- Encapsulate third-party dependencies behind domain traits (DIP) for swappability
- NEVER implement your own: JSON parser, diff algorithm, graph library, parser generator
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
- Centralize shared types in `model/types.rs`
- Centralize cross-language patterns in `extractors/common.rs`
- NEVER force DRY when it creates fragile coupling between unrelated modules

## SOLID (Adapted for Rust)

### SRP — Single Responsibility
- Each module has ONE reason to change, owned by ONE domain area
- `engine.rs` orchestrates; `model/` builds IR; `parser/` parses; `search/` searches
- If you need "and" to describe what a module does, split it

### OCP — Open/Closed
- Prefer composition over inheritance (Rust's natural model)
- New language extractors: add a new file in `extractors/`, register in dispatch — zero changes to engine
- Extension points use enums with `#[non_exhaustive]`

### LSP — Liskov Substitution
- All trait implementations must honor the trait's documented contract
- Never return `unimplemented!()` or `todo!()` in released trait implementations
- If a type cannot fulfill a trait fully, it should not implement that trait

### ISP — Interface Segregation
- Small, focused traits: `DiffComputer`, `CodeModelBuilder`
- Consumers depend only on the traits they use — not on a monolithic engine interface
- Prefer multiple small traits over one large trait with optional methods

### DIP — Dependency Inversion
- Core engine defines traits; downstream crates implement them
- Config uses typed structs (serde) driven by files and environment
- Test doubles implement the same traits as production code

## Tension Resolution
When principles conflict, resolve in this priority order:
1. **KISS** — simplicity wins when complexity adds no value
2. **YAGNI** — don't build what's not needed yet
3. **DRY** — eliminate duplicated knowledge, not duplicated code
4. **SOLID** — apply where complexity is real, not speculative
5. **Don't Reinvent** — use crates for real problems, not speculative ones
