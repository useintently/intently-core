# CLAUDE.md — intently-core

This document is the primary reference for AI agents and human contributors working on the `intently-core` crate. It describes the project scope, architecture, and rules.

---

## What This Crate Is

`intently-core` is the **extraction foundation** of the Intently platform. It answers one question: **"What does this codebase contain?"**

It parses source code across 16 languages, extracts semantic information (routes, dependencies, sinks, symbols, data models, call graphs, module boundaries), and builds a structured intermediate representation called the **System Twin**.

This crate is a **library**. It does NOT contain:
- Policy evaluation (→ separate `intently-policy` crate)
- Health scores (→ separate `intently-policy` crate)
- CLI binary (→ separate `intently-cli` repo)
- MCP server (→ separate `intently-mcp` repo)
- Desktop app / IDE shell (→ separate repo)
- Intent parsing, evidence, planner, triggers, orchestrator (→ future repos)

---

## Architecture

```
intently-core/
├── Cargo.toml              # Single crate, not a workspace
├── src/
│   ├── lib.rs              # Public API re-exports
│   ├── engine.rs           # IntentlyEngine — stateful extraction orchestrator
│   ├── error.rs            # Error types (thiserror)
│   ├── parser/             # Tree-sitter parsing, language detection, incremental edit
│   │   └── mod.rs
│   ├── twin/               # System Twin — the IR
│   │   ├── mod.rs
│   │   ├── types.rs        # SystemTwin, FileExtraction, Component, Interface, etc.
│   │   ├── builder.rs      # Incremental twin builder (O(1) per-file updates)
│   │   ├── diff.rs         # Semantic diff between twin states
│   │   ├── graph.rs        # KnowledgeGraph (petgraph) — impact analysis, cycles
│   │   ├── import_resolver.rs  # Cross-file import resolution
│   │   ├── module_inference.rs # Module boundary detection
│   │   ├── patterns.rs     # Shared cross-language patterns
│   │   └── extractors/     # Per-language semantic extractors
│   │       ├── mod.rs      # Extractor dispatch by language
│   │       ├── common.rs   # Shared utilities (node text, PII detection, anchoring)
│   │       ├── typescript.rs   # Express, NestJS
│   │       ├── python.rs       # FastAPI, Flask, Django
│   │       ├── java.rs         # Spring Boot (also used for Kotlin)
│   │       ├── csharp.rs       # ASP.NET Core, Minimal API
│   │       ├── go.rs           # Gin, Echo, net/http
│   │       ├── php.rs          # Laravel
│   │       ├── ruby.rs         # Rails
│   │       ├── generic.rs      # Fallback (Rust, C, C++, Swift, Scala)
│   │       ├── symbols.rs      # Tree-sitter query-based symbol extraction
│   │       ├── call_graph.rs   # Call site detection per language
│   │       ├── type_hierarchy.rs # extends/implements detection
│   │       └── data_models.rs  # Struct/class/interface field extraction
│   └── search/             # ast-grep structural code search
│       ├── mod.rs
│       └── pattern_engine.rs
├── tests/
│   ├── fixtures/           # 15 multi-file projects across 16 languages
│   ├── full_extraction.rs  # Integration tests: full pipeline per language
│   └── real_world_validation.rs  # Real GitHub repo validation (#[ignore])
├── docs/
│   ├── adrs/               # Architecture Decision Records
│   └── PRD.md              # Product Requirements Document
├── CHANGELOG.md
├── README.md
└── CLAUDE.md               # This file
```

---

## Supported Languages (16)

| Language | Extractor | Frameworks |
|----------|-----------|------------|
| TypeScript | `typescript.rs` | Express, NestJS |
| JavaScript | `typescript.rs` | Express |
| TSX | `typescript.rs` | React components |
| JSX | `typescript.rs` | React components |
| Python | `python.rs` | FastAPI, Flask, Django |
| Java | `java.rs` | Spring Boot |
| Kotlin | `java.rs` | Spring Boot |
| C# | `csharp.rs` | ASP.NET Core, Minimal API |
| Go | `go.rs` | Gin, Echo, net/http |
| PHP | `php.rs` | Laravel |
| Ruby | `ruby.rs` | Rails |
| Rust | `generic.rs` | (log sinks only) |
| C | `generic.rs` | (log sinks only) |
| C++ | `generic.rs` | (log sinks only) |
| Swift | `generic.rs` | (log sinks only) |
| Scala | `generic.rs` | (log sinks only) |

---

## Public API

```rust
// Main entry point
let mut engine = IntentlyEngine::new(project_root);
let result: ExtractionResult = engine.full_analysis()?;

// Incremental updates
let result = engine.on_file_changed(path)?;
let result = engine.on_file_deleted(path)?;
let result = engine.on_files_changed(&[path1, path2])?;

// Single file (no cache mutation)
let extraction: FileExtraction = engine.analyze_single_file(path)?;

// Accessors for downstream consumers
let sources: &HashMap<PathBuf, String> = engine.sources();
let extractions: &HashMap<PathBuf, FileExtraction> = engine.extractions();
let graph: Option<&KnowledgeGraph> = engine.graph();
```

### ExtractionResult

```rust
pub struct ExtractionResult {
    pub twin: SystemTwin,              // The IR
    pub diff: Option<SemanticDiff>,    // Behavioral delta from previous twin
    pub timing: PipelineTiming,        // parse_extract_ms, twin_build_ms, total_ms
    pub graph_stats: Option<GraphStats>, // Node/edge counts
    pub duration_ms: u64,
    pub files_analyzed: usize,
}
```

---

## Development Rules

### Rust
- **Edition:** 2021
- **Error handling:** `thiserror` for all errors. Return `Result`. Never `unwrap()` in library code.
- **Logging:** `tracing` crate. Never `println!` or `eprintln!`.
- **Serialization:** `serde` with `serde_json`.
- **Parsing:** `tree-sitter` for CST parsing, `ast-grep-core` for structural pattern matching.
- **Testing:** unit tests in same file (`#[cfg(test)] mod tests`), integration tests in `tests/`.

### Commit Convention

```
<type>(<scope>): <description>

Types: feat, fix, refactor, docs, test, chore
Scopes: parser, twin, extractors, graph, diff, search, engine
```

---

## How to Add a New Language

1. Add tree-sitter grammar dependency in `Cargo.toml`
2. Add language variant in `parser/mod.rs` (`SupportedLanguage` enum + `detect_language`)
3. Create extractor in `src/twin/extractors/<language>.rs` (or reuse existing)
4. Add symbol extraction query in `src/twin/extractors/symbols.rs`
5. Add call graph patterns in `src/twin/extractors/call_graph.rs`
6. Create fixture project in `tests/fixtures/<framework>_<type>/`
7. Add integration test in `tests/full_extraction.rs`
8. Update this document

## How to Add a New Extractor for an Existing Language

1. Identify the framework's routing/middleware patterns
2. Add detection logic in the language's extractor file
3. Add fixture files exercising the new patterns
4. Add integration test assertions
5. Update CHANGELOG.md

---

## Working on intently-core with AI Agents

1. **Read this file first.** It is the source of truth.
2. **Run `cargo test`** after every change.
3. **Never add `unwrap()` in library code.** Use `Result` with `thiserror`.
4. **This crate is extraction only.** No policy evaluation, no health scores, no governance logic.
5. **Update this file** when adding new modules, languages, or extractors.

---

## References

- [PRD v2](./docs/PRD.md) — Product Requirements Document
- [ADR-001](./docs/adrs/001-extractor-gaps-real-world-validation.md) — Extractor gaps from real-world validation
- [ADR-002](./docs/adrs/002-source-anchoring.md) — Source anchoring strategy
- [ADR-003](./docs/adrs/003-knowledge-graph.md) — Knowledge graph design
- [tree-sitter](https://tree-sitter.github.io/) — Incremental parsing
- [ast-grep](https://ast-grep.github.io/) — Structural pattern matching
