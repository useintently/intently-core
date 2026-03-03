---
name: reviewing-architecture
description: Reviews architecture decisions and module boundaries in intently-core. Validates separation of concerns, dependency direction, abstraction justification, ADR documentation, and extraction-only scope discipline. Use when changes add new modules, modify dependencies, introduce traits, change the public API, or reference ADRs.
---

# Architecture Review

## Critical rules

**ALWAYS:**
- Respect the extraction-only scope — this crate extracts, it does NOT evaluate, score, or govern
- Document significant architectural decisions as ADRs in `docs/adrs/` with context, decision, consequences
- Require 2+ concrete use cases before introducing a new trait or abstraction (YAGNI)
- Keep `lib.rs` public API surface minimal — every `pub` item is a maintenance commitment
- Verify dependency direction: extractors → common, never extractor → extractor

**NEVER:**
- Add policy evaluation, health scores, governance logic, or CLI commands to this crate
- Create circular dependencies between modules — verify with `cargo tree` if in doubt
- Leak internal types (`pub(crate)` types) through the public API in `lib.rs`
- Add a trait with a single implementor — wait for the second concrete use case
- Modify `engine.rs` to contain business logic — it orchestrates only, delegates to `model/`

## intently-core module boundaries

```
engine.rs       — orchestrates extraction pipeline
model/          — CodeModel IR (types, builder, diff, graph, extractors, symbol_table)
parser/         — tree-sitter parsing, language detection
search/         — ast-grep structural pattern matching
git/            — git metadata extraction (feature-gated)
workspace/      — monorepo workspace detection
```

Each module has ONE reason to change. `engine.rs` orchestrates; `model/` builds the IR; `parser/` parses; `search/` searches. **This crate is extraction-only** — no policy evaluation, health scores, or governance logic.

## Checklist

- [ ] Module boundaries respect separation of concerns (see map above)
- [ ] No circular dependencies between modules (`cargo tree` to verify)
- [ ] New abstractions (traits) have 2+ concrete use cases — not speculative (YAGNI)
- [ ] Significant decisions documented as ADRs in `docs/adrs/`
- [ ] Public API surface in `lib.rs` is minimal and intentional
- [ ] Extraction-only scope is respected (no policy/governance logic)

## Output format

```
## Architecture Review: <scope>

### Findings
- [PASS/FAIL] <category>: <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
