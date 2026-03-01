# Performance Review

Performance-focused review for the Intently IDE project.

## Trigger

Activate when PRs or changes touch:
- Hot paths (IR parsing, diff computation, evidence evaluation)
- Data structure choices for large datasets
- Async/concurrent code
- Benchmark files or performance-related configuration

Keywords: "performance review", "review performance", "benchmark", "perf review", "optimization"

## What This Skill Does

1. **Algorithmic Complexity** — Check for expensive operations
   - No O(n^2) or worse in hot paths (IR traversal, diff, search)
   - Data structure choice matches access pattern (HashMap vs BTreeMap vs Vec)
   - Sorting, searching, and filtering use appropriate algorithms
   - Nested loops over large collections are flagged

2. **Incremental Computation** — Verify unnecessary recomputation is avoided
   - IR updates are incremental (only changed files reprocessed)
   - Semantic diff reuses unchanged subtrees
   - Evidence selection caches previous results
   - Memoization where pure functions are called repeatedly

3. **Benchmark Results** — Review criterion benchmarks
   - New hot-path code has criterion benchmarks
   - Benchmark results compared against baseline
   - No regression beyond acceptable threshold (documented per benchmark)
   - Benchmarks are deterministic and reproducible

4. **Memory Allocation** — Check allocation patterns
   - Pre-allocation for known-size collections (`Vec::with_capacity`)
   - No allocation in tight loops (reuse buffers)
   - Large data structures use appropriate smart pointers
   - No unbounded growth (queues, caches have size limits)

5. **Async Correctness** — Validate async/await usage
   - No blocking operations in async context (`std::fs`, `std::thread::sleep`)
   - CPU-intensive work is offloaded to blocking threads (`spawn_blocking`)
   - No unnecessary `.await` points that increase latency
   - Task cancellation is handled gracefully

6. **Concurrency** — Check parallel execution patterns
   - Shared state uses appropriate synchronization (`Mutex`, `RwLock`, atomics)
   - Lock granularity is appropriate (not too coarse, not too fine)
   - No deadlock potential (consistent lock ordering)
   - Rayon or tokio task parallelism for CPU-bound batch work

## What to Check

- [ ] No O(n^2) in hot paths
- [ ] Incremental computation for IR, diff, and evidence
- [ ] Criterion benchmarks for new hot-path code
- [ ] Pre-allocated collections, no allocation in tight loops
- [ ] No blocking in async context
- [ ] Synchronization is correct and minimal
- [ ] Caches and queues have bounded size

## Output Format

```
## Performance Review: <file_path>

### Algorithmic Complexity
- [PASS/FAIL] <detail>

### Incremental Computation
- [PASS/FAIL] <detail>

### Memory Allocation
- [PASS/FAIL] <detail>

### Async Correctness
- [PASS/FAIL] <detail>

### Concurrency
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
