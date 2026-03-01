# CodeModel Review

Review CodeModel (IR) implementation in Intently.

## Trigger

Activate when PRs or changes touch:
- `crates/Intently_core/src/ir/` (any file)
- IR data structures, serialization, or deserialization
- `system_twin.json` schema definitions

Keywords: "code model review", "IR review", "review IR", "review code model"

## What This Skill Does

1. **IR Completeness** — Verify the IR captures all necessary information
   - Components: functions, modules, types, traits, interfaces
   - Dependencies: imports, calls, trait implementations, type references
   - Contracts: input types, output types, error types, invariants
   - Flows: control flow (branching, loops), data flow (assignments, transforms)

2. **Representation Correctness** — Validate IR accurately models source code
   - IR nodes correctly map to source language constructs
   - Relationships between nodes are bidirectional where appropriate
   - Type information is preserved and queryable

3. **Schema Compliance** — Validate `system_twin.json` output
   - Serialized IR is self-describing (includes schema version)
   - All node types have unique, stable identifiers
   - Deserialization round-trips correctly (serialize -> deserialize -> serialize = identical)

4. **Incremental Updates** — Verify IR updates are incremental
   - File change triggers partial IR rebuild, not full rebuild
   - Affected subgraph is correctly identified and updated
   - Unaffected nodes retain their identity (no spurious diffs)

5. **Extensibility** — Check that new language support can be added
   - Language-specific parsing is behind a trait/interface
   - Core IR is language-agnostic
   - Adding a new language does not require modifying existing IR code

## What to Check

- [ ] IR captures components, dependencies, contracts, and flows
- [ ] IR nodes correctly represent source constructs
- [ ] `system_twin.json` round-trips without data loss
- [ ] Incremental update only rebuilds affected subgraph
- [ ] Unchanged nodes retain stable identifiers across updates
- [ ] Language-specific logic is behind a parser trait
- [ ] Unit tests cover: parsing, serialization, incremental update, edge cases

## Output Format

```
## CodeModel Review: <file_path>

### IR Completeness
- [PASS/FAIL] <detail>

### Representation Correctness
- [PASS/FAIL] <detail>

### Schema Compliance
- [PASS/FAIL] <detail>

### Incremental Updates
- [PASS/FAIL] <detail>

### Extensibility
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
