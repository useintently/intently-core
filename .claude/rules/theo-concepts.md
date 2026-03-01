# Intently Core Concepts

Core domain concepts of the intently-core extraction library. This is the foundational vocabulary — every contributor MUST understand these concepts before writing code.

## Scope

intently-core is an **extraction-only** library. It analyzes source code and produces structured semantic representations. It does NOT evaluate policies, compute health scores, or make governance decisions. Those are responsibilities of downstream consumers.

## CodeModel (IR)
- Semantic intermediate representation of the entire codebase
- Contains: components, interfaces (routes), dependencies (HTTP calls), sinks (log calls), symbols, data models, imports, module boundaries
- Built by the Core Engine through static analysis of source code via tree-sitter
- NOT a 1:1 mapping of files — it captures behavioral semantics, not syntax
- The CodeModel is regenerated on every relevant code change
- Produced by `CodeModelBuilder` with incremental per-file updates

## FileExtraction
- Per-file extraction result containing all semantic data found in a single source file
- Contains: interfaces, dependencies, sinks, symbols, references, data models, module boundaries, imports
- Each artifact carries a `SourceAnchor` with precise tree-sitter position data
- FileExtractions are aggregated by `CodeModelBuilder` into the CodeModel

## Semantic Diff
- Computes the behavioral delta between two CodeModel states
- Captures: added/removed/modified components, changed contracts, new dependencies
- This is NOT a textual diff — it understands what changed semantically
- A diff with zero semantic impact may still have textual changes (formatting, comments)

## KnowledgeGraph
- petgraph-backed graph for structural code analysis
- 6 node types: File, Symbol, Interface, DataModel, Module, External
- 9 edge types: Calls, Extends, Implements, Imports, UsesType, Defines, Contains, Exposes, DependsOn
- Provides: impact analysis (BFS blast radius), circular dependency detection (Tarjan SCC), graph statistics

## ExtractionResult
- The main output type of `IntentlyEngine`
- Contains: `model` (CodeModel), `diff` (Option<SemanticDiff>), `timing` (PipelineTiming), `graph_stats` (Option<GraphStats>)
- No policy reports, no health scores — pure extraction data

## Extractors
- Language-specific modules that walk tree-sitter CSTs to find semantic artifacts
- Each extractor targets specific frameworks: Express/NestJS (TS), FastAPI/Flask/Django (Python), Spring Boot (Java/Kotlin), ASP.NET Core (C#), Gin/Echo (Go), Laravel (PHP), Rails (Ruby)
- A generic fallback extractor handles remaining languages via text-based heuristics
- Shared utilities in `extractors/common.rs` for cross-language patterns

## SourceAnchor
- Position data attached to every extracted artifact
- Contains: file path, start/end line, start/end byte, node kind
- Enables precise navigation and future code rewriting by downstream tools

## IntentlyEngine
- Stateful orchestrator that manages the full extraction pipeline
- Caches: source code, parsed trees (tree-sitter), per-file extractions
- Methods: `full_analysis()` (full scan), `on_file_changed()` (incremental), `analyze_single_file()` (on-demand)
- Public accessors: `sources()`, `extractions()` for downstream consumers

## Artifact Flow (Core Scope)
```
Source Files
    |
    v
[tree-sitter Parsing]
    |
    v
[Language Extractors]
    |
    v
FileExtraction (per file)
    |
    v
[CodeModelBuilder]
    |
    v
CodeModel  -->  SemanticDiff (if previous model exists)
    |
    v
[KnowledgeGraph Builder]
    |
    v
ExtractionResult { model, diff, timing, graph_stats }
```

## What is NOT in Core
- Policy evaluation (SEC-*, REL-*, ARC-*, PERF-*) → `intently-policy` crate
- Health score computation → `intently-policy` crate
- Intent parsing (`intent.yaml`) → separate crate
- Evidence engine (IBTS, test selection) → separate crate
- Planner (action plans) → separate crate
- LLM orchestrator → separate crate
- CLI binary → separate repo
- MCP server → separate repo
- Desktop IDE (Tauri) → separate repo
