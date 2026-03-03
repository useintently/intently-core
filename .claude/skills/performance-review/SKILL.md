---
name: reviewing-performance
description: Reviews performance-sensitive code in intently-core. Checks algorithmic complexity, incremental computation, memory allocation patterns, async correctness, and concurrency. Use when changes touch hot paths (CodeModel building, diff computation, graph analysis), data structures, async code, or benchmarks.
---

# Performance Review

## Critical rules

**ALWAYS:**
- Run `criterion` benchmarks before AND after performance-related changes — no regressions without ADR
- Use `Vec::with_capacity` when the collection size is known or estimable
- Use `tokio::spawn_blocking` for CPU-intensive work in async contexts
- Verify incremental computation: single file change must NOT trigger full CodeModel rebuild
- Profile before optimizing — measure, don't guess

**NEVER:**
- Introduce O(n²) or worse in any hot path (builder, diff, graph, symbol_table, engine)
- Allocate inside tight loops — reuse buffers, pre-allocate, collect into pre-sized Vecs
- Use `std::fs` or `std::thread::sleep` in async code — use tokio equivalents
- Create unbounded caches or queues — all growth must have a size limit
- Clone `String`, `Vec`, or `HashMap` in hot paths without measured justification — borrow first

## intently-core hot paths

- CodeModel building (`src/model/builder.rs`) — incremental per-file updates
- Semantic diff (`src/model/diff.rs`) — behavioral delta computation
- KnowledgeGraph (`src/model/graph/`) — impact analysis (BFS), cycle detection (Tarjan SCC)
- Extraction pipeline (`src/engine.rs`) — chunked parallel extraction (CHUNK_SIZE=500)
- Symbol resolution (`src/model/symbol_table.rs`) — two-level lookup (per-file exact + global fuzzy)

## Checklist

- [ ] No O(n²) in hot paths listed above
- [ ] Incremental computation: only changed files reprocessed, unchanged nodes retain identity
- [ ] `Vec::with_capacity` for known-size collections, no allocation in tight loops
- [ ] No blocking operations in async context (use `tokio::spawn_blocking` for CPU work)
- [ ] Synchronization is correct and minimal (lock ordering, bounded queues/caches)
- [ ] New hot-path code has `criterion` benchmarks compared against baseline

## Output format

```
## Performance Review: <file_path>

### Findings
- [PASS/FAIL] <category>: <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
