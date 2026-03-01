# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Restructured repository: repo IS the crate (no more `crates/` workspace directory)
- Renamed `AnalysisResult` to `ExtractionResult` — reflects extraction-only scope
- Removed `policy_ms` from `PipelineTiming` — policy evaluation is not core's responsibility
- Added `sources()` and `extractions()` public accessors on `IntentlyEngine` for downstream consumers

### Removed
- Policy engine (`src/policy/`) — moved to separate `intently-policy` crate (to be created)
- Health score computation (`src/health/`) — moved with policy engine
- SEC-003 secret scanning cache and regex patterns from `IntentlyEngine`
- CLI binary (`intently_cli`) — moved to separate repository
- MCP server (`intently_mcp`) — moved to separate repository
- Cross-language policy verification integration tests (belong in policy crate)
- `regex` dependency (was only used by policy engine)

### Added
- Realtime code analysis engine with <250ms incremental analysis target
- Multi-language parser supporting 16 languages: TypeScript, TSX, JavaScript, JSX, Python, Java, C#, Go, Rust, PHP, Ruby, Kotlin, Swift, C, C++, Scala
- Language family grouping (JavaScriptLike, JvmLike, CLike, etc.) for extractor dispatch
- TypeScript/JavaScript extractor: Express routes, auth middleware, HTTP calls, log sinks, PII detection, imports
- Python extractor: FastAPI/Flask/Django route detection, auth decorator detection (`@login_required`, `@jwt_required`), HTTP calls (requests, httpx)
- Java/Kotlin extractor: Spring Boot annotation routes (`@GetMapping`, `@PostMapping`, `@RequestMapping`), auth annotations (`@PreAuthorize`, `@Secured`), HTTP calls (RestTemplate, WebClient)
- C# extractor: ASP.NET Core attribute routes (`[HttpGet]`, `[HttpPost]`), auth attributes (`[Authorize]`), HTTP calls (HttpClient)
- Go extractor: Gin/Echo/net-http route detection, middleware auth detection, HTTP calls (`http.Get`, `http.Post`, `client.Do`)
- PHP extractor: Laravel `Route::get()`/`Route::post()` routes, `->middleware('auth')` detection, HTTP calls (`Http::get`, Guzzle)
- Ruby extractor: Rails route DSL (`get '/path'`, `resources :name`), `before_action` auth detection, HTTP calls (HTTParty, Faraday, RestClient)
- Shared extractor utilities module (`common.rs`): node text extraction, argument collection, HTTP method parsing, log sink detection
- `AuthKind` enum extended with `Decorator`, `Annotation`, `Attribute` variants for cross-framework auth representation
- Generic fallback extractor for remaining languages (Rust, Swift, C, C++, Scala): log sink detection with PII scanning via text-based heuristics
- Shared cross-language patterns module: PII indicators, auth middleware names, log objects/methods
- System Twin builder with dynamic language detection (most common language across extractions)
- Semantic diff engine computing behavioral deltas between Twin states
- Policy engine with 4 MVP policies: SEC-001 (auth on endpoints), SEC-002 (no PII in logs), SEC-003 (no secrets in source), REL-001 (timeout on external calls)
- Health score computation with per-category and weighted overall scores
- IntentlyEngine stateful orchestrator with full and incremental analysis, caching, and batch processing
- File watcher (notify crate) with multi-language file filtering and ignored directory exclusion
- JSON-RPC 2.0 server over stdin/stdout for IDE integration
- CLI binary with debounced event loop (50ms window) using crossbeam channels
- C# `ILogger<T>` log methods (`LogInformation`, `LogWarning`, `LogError`, `LogDebug`, `LogTrace`, `LogCritical`) added to log sink detection patterns
- 15 multi-file e-commerce fixture projects (56 files total) covering all 16 supported languages for integration testing
- 23 integration tests exercising full extraction pipeline across Express, FastAPI, Flask, Django, Spring Boot, Kotlin Spring, ASP.NET Core, Gin, Echo, net/http, Laravel, Rails, and generic fallback languages
- Cross-language policy verification tests (SEC-001, SEC-002, REL-001) validating policy engine against real fixture projects
- 213 total tests (183 unit + 23 integration + 7 CLI) covering all modules including 7 dedicated backend framework extractors
- Incremental tree-sitter parsing with `InputEdit` computation via `similar` crate byte-level diffing — reduces re-parse time from O(file_size) to O(edit_size)
- Tree cache (`HashMap<PathBuf, tree_sitter::Tree>`) in `IntentlyEngine` for storing parsed CSTs across changes
- Per-file SEC-003 secret scanning cache — only re-scans changed files instead of entire codebase on every change
- Compiled secret detection patterns stored once at engine creation (5 regex patterns for AWS keys, Stripe keys, GitHub tokens, generic secrets, private keys)
- `TwinBuilder` incremental twin builder tracking per-file contributions — O(1) `set_file`/`remove_file` instead of O(n) full rebuild
- `Symbol` and `SymbolKind` types for code-level IR (classes, functions, methods, interfaces, traits, enums, structs, modules)
- Tree-sitter query-based symbol extraction for 9 languages (TypeScript, JavaScript, Python, Java, C#, Go, Rust, PHP, Ruby)
- `PipelineTiming` struct providing per-stage timing breakdown (parse/extract, twin build, policy evaluation, total)
- `Visibility` enum (`Public`, `Private`, `Protected`, `Internal`) for symbol access modifiers
- `signature`, `visibility`, `parent` fields on `Symbol` for enriched code-level IR — LLMs read full signatures natively
- Doc comment extraction for all 9 symbol-supported languages: JSDoc (`/** */`), Python docstrings, Rust `///`, Go `//`, Ruby `#`, Java JavaDoc, PHP `/** */`, C# `///`
- Go symbol query now captures `struct` and `interface` type declarations
- `imports` field on `Component` and `total_imports` on `TwinStats` — imports now flow from `FileExtraction` through `TwinBuilder` to the System Twin
- `analyze_single_file()` method on `IntentlyEngine` for on-demand single-file analysis without mutating caches
- `FileExtraction` re-exported from `intently_core` public API
- MCP server crate (`intently_mcp`) implementing Model Context Protocol over JSON-RPC 2.0 stdin/stdout
- 6 MCP tools: `get_system_overview`, `query_endpoints`, `query_symbols`, `get_policy_report`, `get_dependencies`, `get_file_analysis`
- 7 MCP knowledge graph tools: `get_module_map`, `get_data_models`, `get_callers`, `get_callees`, `get_type_hierarchy`, `get_references`, `get_module_detail`
- 4 MCP resources: `intently://system-twin`, `intently://policy-report`, `intently://knowledge-graph` (full reference graph JSON), `intently://module-map` (module boundaries JSON)
- Transitive call graph traversal in `get_callers`/`get_callees` with BFS up to depth 5, cycle-safe via visited set
- `max_results` parameter with `truncated`/`total_count` metadata on all new query tools to prevent context window overflow
- `get_system_overview` now includes `total_references`, `total_data_models`, `total_modules` stats and `module_summary` array
- `get_module_detail` for progressive disclosure: deep dive into one module's symbols, data models, and references
- Call graph extraction (`call_graph.rs`): recursive CST walk detecting call sites per language, with enclosing function resolution
- Type hierarchy extraction (`type_hierarchy.rs`): extends/implements detection for TS, Python, Java, C#, Go, Rust
- Data model extraction (`data_models.rs`): class/struct/interface declarations with field-level detail per language
- Cross-file import resolution (`import_resolver.rs`): 3-tier strategy (relative path, named symbol lookup, external package)
- Module boundary inference (`module_inference.rs`): directory-based grouping with public symbol collection and inter-module dependency computation
- `Reference`, `ReferenceKind`, `DataModel`, `DataModelKind`, `FieldInfo`, `ModuleBoundary` types for knowledge graph IR
- `references`, `data_models`, `module_boundaries` fields on `Component` and `FileExtraction`
- `total_references`, `total_data_models`, `total_modules` fields on `TwinStats`
- `ast-grep-core` integration for YAML-based structural code search patterns
- 479 total test runs (7 CLI + 335 core unit + 23 integration + 57 MCP unit + 57 MCP integration)
- Real-world validation harness: 20 `#[ignore]` integration tests cloning 22 GitHub projects across all 16 supported languages, validating extraction on real codebases with automatic cleanup via `TempDir`
- ADR-001: Extractor gaps documentation from real-world MCP validation against 8 GitHub repos — 6 gaps identified with priority matrix (ADR-001)
- NestJS decorator-based routing: `@Controller`, `@Get`/`@Post`/`@Put`/`@Delete`/`@Patch`/`@Options`/`@Head`/`@All` with path composition and `@UseGuards` auth detection (ADR-001 GAP-01)
- ASP.NET Minimal API extraction: `app.MapGet()`/`MapPost()`/`MapPut()`/`MapPatch()`/`MapDelete()` route detection with `.RequireAuthorization()` chain auth (ADR-001 GAP-03)
- C# class-level `[Route("prefix")]` composition: controller route prefix applied to method routes, `[controller]` token replacement, class-level `[Authorize]` inheritance with `[AllowAnonymous]` override (ADR-001 GAP-02)
- Laravel `Route::resource()` expansion (7 RESTful routes), `Route::apiResource()` expansion (5 routes), `Route::any()` with `HttpMethod::All` (ADR-001 GAP-05)
- C# `record_declaration` support in symbol extraction query (ADR-001 GAP-04)
- Symbol extraction diagnostics: parse error warning when tree-sitter CST has errors, empty symbols warning for non-trivial files (ADR-001 GAP-04)
- `HealthScores.confidence` field (0.0–1.0): extraction completeness heuristic based on ratio of files yielding semantic data (ADR-001 GAP-06)
- `HealthInput` struct for passing extraction stats to health computation
- Health confidence exposed in MCP `get_system_overview` tool response
- `SourceAnchor` type with full tree-sitter position data (`file`, `line`, `end_line`, `start_byte`, `end_byte`, `node_kind`) on all extracted artifacts — enables precise AST-grounded navigation and future code rewriting (ADR-002)
- `anchor_from_node()` helper in `extractors/common.rs` for creating `SourceAnchor` from tree-sitter CST nodes
- `SourceAnchor::from_line()` and `SourceAnchor::from_line_range()` constructors for test and manual anchor creation
- `get_code_context` MCP tool (14th tool) for source code retrieval by file/line range with semantic annotations — returns code snippet plus anchored items (interfaces, dependencies, sinks, symbols) found in the range
- `get_source()` and `get_extraction()` public methods on `IntentlyEngine` for MCP tool source code access
- `KnowledgeGraph` type backed by petgraph for structural code analysis — 6 node types (File, Symbol, Interface, DataModel, Module, External) and 9 edge types (Calls, Extends, Implements, Imports, UsesType, Defines, Contains, Exposes, DependsOn) (ADR-003)
- Impact analysis via multi-edge BFS computing blast radius for any symbol — traverses calls, inheritance, and type usage edges
- Circular dependency detection via Tarjan's SCC algorithm (`petgraph::algo::tarjan_scc`) at both module and symbol levels
- ARC-001 policy implementation: "No circular dependencies" — first architecture policy, evaluated when knowledge graph is available
- 3 new MCP tools: `get_impact_analysis` (blast radius), `get_cycles` (circular dependency detection), `get_graph_stats` (graph-wide statistics)
- `intently://knowledge-graph-export` MCP resource for visualization-ready JSON export with nodes, edges, and metadata (D3, Sigma.js, Cytoscape)
- `GraphStats` included in `get_system_overview` response when knowledge graph is available

### Changed
- Interface, Dependency, Sink, Symbol, DataModel now carry `anchor: SourceAnchor` instead of separate `file`/`line` fields — JSON backward-compatible via `#[serde(flatten)]` (ADR-002)
- `get_callers`, `get_callees`, `get_type_hierarchy` MCP tools now use graph-backed traversal via petgraph BFS (was hand-rolled BFS on flat `Vec<Reference>`) — O(1) adjacency lookups instead of O(n) linear scans
- `intently://knowledge-graph` MCP resource now returns full graph JSON export when knowledge graph is available (was raw reference list)
- MCP server now exposes 17 tools (was 14) and 5 resources (was 4) with addition of `get_impact_analysis`, `get_cycles`, `get_graph_stats`, and `intently://knowledge-graph-export`
- Policy engine evaluates ARC-001 when knowledge graph is available — `evaluate_policies()` now accepts optional `&KnowledgeGraph` parameter
- `AnalysisResult` now includes `graph_stats: Option<GraphStats>` for lightweight graph metrics
- MCP server now exposes 14 tools (was 13) with addition of `get_code_context`
- `compute_health()` now accepts `&HealthInput` parameter for extraction confidence computation (ADR-001 GAP-06)
- `HealthScores` struct extended with `confidence: f64` field (ADR-001 GAP-06)
- `parse_source()` now accepts optional `old_tree` parameter for incremental tree-sitter parsing
- `evaluate_policies` split into full (`evaluate_policies`) and incremental (`evaluate_policies_with_cache`) variants
- `build_result()` uses `TwinBuilder.build()` instead of collecting all cached extractions
- Engine tracks `secret_scan_cache`, `tree_cache`, and `twin_builder` for incremental updates
- `AnalysisResult` now includes `timing: PipelineTiming` field for pipeline observability
- `Symbol` struct extended with `signature: Option<String>`, `visibility: Option<Visibility>`, `parent: Option<String>` fields
- `Component` struct now includes `imports: Vec<ImportInfo>` (previously dropped during twin build)
- `TwinStats` struct now includes `total_imports: usize` field
- Workspace expanded to 3 crates: `intently_core`, `intently_cli`, `intently_mcp`

### Fixed
- NestJS projects now produce endpoints instead of 0 — full decorator-based routing extraction (ADR-001 GAP-01)
- ASP.NET Minimal API endpoints no longer have empty paths — `MapGet`/`MapPost` pattern extraction (ADR-001 GAP-03)
- C# controller routes now include class-level `[Route]` prefix — prevents bare method paths without `/api/...` prefix (ADR-001 GAP-02)
- Laravel `Route::resource()` and `Route::apiResource()` now expand to 7/5 routes instead of being silently skipped (ADR-001 GAP-05)
- C# `record` types now detected in symbol extraction — previously returned 0 symbols for modern C# files (ADR-001 GAP-04)
- Health scores no longer show 100% when extraction fails completely — `confidence` field reveals extraction quality (ADR-001 GAP-06)

### Known Gaps
- Route group prefix resolution missing for Go Gin `r.Group()`, PHP Laravel `Route::prefix()->group()`, C# `MapGroup()` (ADR-001 GAP-02 partial)
