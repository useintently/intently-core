---
name: reviewing-code-model
description: Reviews CodeModel (intermediate representation) changes in intently-core. Validates IR completeness, representation correctness, serialization round-tripping, incremental update behavior, and language-agnostic extensibility. Use when changes touch src/model/types.rs, src/model/builder.rs, CodeModel data structures, or serialization logic.
---

# CodeModel Review

## Critical rules

**ALWAYS:**
- Attach a `SourceAnchor` (file, start/end line, start/end byte, node kind) to every extracted artifact
- Use `update_file()` for incremental updates — only the changed file is reprocessed
- Ensure serialization round-trips: `serialize → deserialize → serialize` must produce identical output
- Keep the core IR language-agnostic — language-specific logic goes behind `LanguageBehavior` trait
- Verify `CodeModel` output is deterministic: same input files must always produce the same model

**NEVER:**
- Trigger a full CodeModel rebuild on a single file change — always use incremental path
- Break node identity on incremental updates — unchanged nodes must retain stable IDs (no spurious diffs)
- Add language-specific fields to core IR types (`CodeModel`, `Component`, `Interface`) — extend via extractors
- Modify `src/model/extractors/mod.rs` dispatch logic when adding a new extractor — register only
- Skip `FileExtraction.content_hash` (SHA-256) — content fingerprinting is required for cache invalidation

## Key files

- `src/model/types.rs` — CodeModel, FileExtraction, Component, Interface, SourceAnchor
- `src/model/builder.rs` — CodeModelBuilder with incremental per-file updates
- `src/model/graph/` — KnowledgeGraph (petgraph), WeightedEdge, impact analysis
- `src/model/symbol_table.rs` — Two-level symbol table (per-file exact + global fuzzy)
- `src/model/extractors/` — Language-specific extractors behind `LanguageBehavior` trait

## Checklist

- [ ] CodeModel captures: components, interfaces (routes), dependencies, sinks, symbols, data models, imports, module boundaries
- [ ] IR nodes correctly map to source language constructs via SourceAnchor (file, line, byte positions)
- [ ] Serialization round-trips without data loss (serialize → deserialize → serialize = identical)
- [ ] File change triggers partial rebuild via `update_file()`, not full rebuild
- [ ] Unchanged nodes retain stable identity across incremental updates (no spurious diffs)
- [ ] Language-specific logic is behind `LanguageBehavior` trait — core IR is language-agnostic
- [ ] New extractors register in `src/model/extractors/mod.rs` dispatch — zero changes to engine

## Output format

```
## CodeModel Review: <file_path>

### Findings
- [PASS/FAIL] <category>: <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
