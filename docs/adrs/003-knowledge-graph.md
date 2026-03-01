# ADR-003: Knowledge Graph — petgraph Foundation

## Status

Accepted

## Date

2026-02-28

## Context

Intently extracts rich semantic data (symbols, references, data models, imports, type hierarchies, module boundaries) but stores them as flat `Vec<T>` inside `Component`. Every graph query — callers, callees, type hierarchy, impact analysis — must linearly scan the entire reference set. The MCP server has a hand-rolled BFS (`traverse_call_graph` in `tools.rs`) operating on `Vec<&Reference>`.

This approach is:

1. **O(n) per query** — each BFS step scans all references to find neighbors
2. **Duplicated logic** — callers, callees, type hierarchy each re-implement traversal
3. **No structural analysis** — no cycle detection, connected components, or community detection
4. **No ARC-001** — the "no circular dependencies" policy is defined in CLAUDE.md but has no implementation

## Decision

Add a `KnowledgeGraph` type backed by `petgraph::DiGraph` as a **derived view** over the SystemTwin. The graph is built from existing extraction data — no new extraction needed.

### Why petgraph

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| **petgraph** | MIT, pure Rust, zero transitive deps, battle-tested (13M downloads), provides Tarjan's SCC, BFS, DFS out of the box | API can be verbose | **Chosen** |
| KuzuDB | Full graph DB with Cypher queries | External process, heavy dependency, overkill for in-memory use | Rejected |
| SQLite + recursive CTEs | Familiar SQL | Not a graph engine, poor traversal performance, external dep | Rejected |
| Custom adjacency list | No dependencies | Must implement SCC, BFS, DFS ourselves — violates "don't reinvent" | Rejected |

### Graph as Derived View

The `KnowledgeGraph` is NOT a separate storage layer. It is **computed from** the `SystemTwin` after every build. The twin remains the source of truth. The graph is a view optimized for traversal queries and structural analysis.

```
parse/extract → twin build → GRAPH BUILD → policy eval (ARC-001) → health
```

### Node Types

- `File` — source file (anchor for Defines edges)
- `Symbol` — function, class, method, trait, etc.
- `Interface` — HTTP endpoint
- `DataModel` — class/struct/record with fields
- `Module` — logical module from ModuleBoundary inference
- `External` — unresolved external dependency (npm package, stdlib, etc.)

### Edge Types

- `Calls` — function/method call (from `Reference::Call`)
- `Extends` — inheritance (from `Reference::Extends`)
- `Implements` — interface/trait implementation (from `Reference::Implements`)
- `Imports` — import statement (from `ImportInfo`)
- `UsesType` — type usage (from `Reference::TypeUsage`)
- `Defines` — file contains symbol/interface/data model
- `Contains` — module contains file
- `Exposes` — module exports symbol
- `DependsOn` — module-level dependency

### ARC-001: No Circular Dependencies

The first architecture policy, implemented using `petgraph::algo::tarjan_scc()`. Strongly connected components with size > 1 indicate cycles. This gives us cycle detection "for free" from petgraph — no reimplementation needed.

## Consequences

### Positive

- Graph queries become O(1) adjacency lookups instead of O(n) scans
- Cycle detection via Tarjan's SCC enables ARC-001 policy
- Impact analysis (blast radius) becomes a single BFS call
- JSON export enables future visualization (D3, Sigma.js)
- Replaces 56 lines of hand-rolled BFS in MCP tools.rs

### Negative

- Additional memory for the graph (petgraph DiGraph + node index HashMap)
- Graph rebuild cost on every twin change (~ms for typical projects)
- New dependency (petgraph), though it's MIT, pure Rust, zero transitive deps

### Deferred (Future Phases)

- Phase 2: Louvain community detection for architectural clustering
- Phase 3: Semantic search via embeddings (candle + usearch)
- Phase 4: Visual graph export for Tauri desktop (D3/Sigma.js)

## References

- [petgraph documentation](https://docs.rs/petgraph/)
- [Tarjan's SCC algorithm](https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm)
- Prowl project: graph-as-derived-view pattern, Louvain community detection
