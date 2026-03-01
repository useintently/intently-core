# ADR-002: Source Anchoring — AST-Grounded Semantic Analysis

**Status:** Accepted
**Date:** 2026-02-28
**Context:** Intently Core extraction pipeline

---

## Context

Intently extracts semantic data (routes, dependencies, sinks, symbols, data models) from source code via tree-sitter CSTs. Currently, extracted artifacts store only `file: PathBuf` + `line: usize`. All other tree-sitter position data (end_line, byte offsets, node kind) is discarded after extraction.

This is lossy. Once data enters the System Twin, there is no precise link back to the CST node that produced it. This blocks:

1. **AST Rewriting** — a policy violation should map to the exact AST node for deterministic code fixes.
2. **Code context retrieval** — LLMs consuming MCP tools see "POST /checkout at line 42" but cannot see the actual code.
3. **Stable navigation** — line numbers drift on edits; byte ranges combined with incremental parsing do not.

## Decision

Add `SourceAnchor` to every extracted artifact — a struct capturing full tree-sitter node position data. Add `get_code_context` MCP tool for anchor-to-source resolution.

### `SourceAnchor` type

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceAnchor {
    pub file: PathBuf,
    pub line: usize,        // 1-based start line
    pub end_line: usize,    // 1-based end line
    pub start_byte: usize,  // byte offset in source file
    pub end_byte: usize,    // byte offset in source file
    pub node_kind: String,  // tree-sitter CST node type
}
```

### Anchored types (Phase 1)

| Type | Previously | After |
|------|-----------|-------|
| `Interface` | `file`, `line` | `#[serde(flatten)] anchor: SourceAnchor` |
| `Dependency` | `file`, `line` | `#[serde(flatten)] anchor: SourceAnchor` |
| `Sink` | `file`, `line` | `#[serde(flatten)] anchor: SourceAnchor` |
| `Symbol` | `file`, `line`, `end_line` | `#[serde(flatten)] anchor: SourceAnchor` |
| `DataModel` | `file`, `line`, `end_line` | `#[serde(flatten)] anchor: SourceAnchor` |

### Deferred types (Phase 2)

- `Reference` — needs `source_anchor` + `target_anchor` (two anchors per instance, more complex migration).
- `ImportInfo` — no `file` field; inherits context from `FileExtraction`.
- `FieldInfo` — no `file` field; inherits context from `DataModel`.

## Rationale

### Why `#[serde(flatten)]`?

Using `#[serde(flatten)]` on the `anchor` field serializes `file`, `line`, `end_line`, `start_byte`, `end_byte`, and `node_kind` as top-level JSON fields. Existing MCP consumers that read `file` and `line` continue working without changes. New fields appear alongside.

### Why no columns?

YAGNI. Byte offsets are strictly more precise than column numbers and are what tree-sitter natively provides. Column numbers can be derived from byte offsets + source text if needed later.

### Why `node_kind`?

The tree-sitter CST node type (e.g., `call_expression`, `decorator`) provides semantic context about *what syntactic construct* produced the extraction. This is useful for AST rewriting (knowing the node kind informs the rewrite strategy) and for debugging extraction logic.

### Term origin

Source Anchoring follows the [Kythe](https://kythe.io/) (Google) model where every semantic fact is anchored to a precise source span via byte offsets.

## Consequences

- **Access pattern change:** `x.file` becomes `x.anchor.file`, `x.line` becomes `x.anchor.line` across all consumers.
- **JSON output grows:** Each anchored type gains 3 new fields (`end_line`, `start_byte`, `end_byte`, `node_kind`). For typical projects this is negligible.
- **Test construction:** Manual type construction in tests uses `SourceAnchor::from_line()` / `from_line_range()` convenience constructors that default byte offsets to 0 and node_kind to empty string.
- **Future capability:** Anchored types enable deterministic AST rewriting, code context retrieval for LLMs, and stable cross-edit navigation.
