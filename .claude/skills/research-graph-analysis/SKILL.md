---
name: research-graph-analysis
description: Researches graph-based program analysis techniques relevant to intently-core. Tracks knowledge graphs for code (Code Property Graph, Joern), call graph algorithms, community detection, impact analysis, and graph scalability. Use when investigating graph query improvements, module boundary detection, impact propagation algorithms, or graph storage alternatives.
---

# Graph Analysis Research

## Critical rules

**ALWAYS:**
- Benchmark graph algorithms against intently-core's actual graph sizes (measure node/edge counts first)
- Evaluate both correctness and scalability — an O(n^3) algorithm that's accurate is useless at 10K+ nodes
- Compare Code Property Graph (CPG) approach against current flat graph to quantify real benefits
- Test community detection algorithms on actual codebases before recommending for module inference
- Include incremental graph maintenance — full rebuild on every change is unacceptable

**NEVER:**
- Recommend graph databases (Neo4j, ArangoDB) without proving petgraph's in-memory model is insufficient
- Propose call graph algorithms (k-CFA, VTA) that require type information intently-core doesn't extract
- Ignore weighted edges — confidence scoring on references is fundamental to impact analysis accuracy
- Evaluate graph techniques on synthetic graphs — use real CodeModel graph outputs
- Conflate graph analysis (this crate's scope) with graph visualization (downstream concern)

## Current state in intently-core

- **KnowledgeGraph** backed by `petgraph::DiGraph<GraphNode, WeightedEdge>`
- **6 node types**: File, Symbol, Interface, DataModel, Module, External
- **9 edge types**: Calls, Extends, Implements, Imports, UsesType, Defines, Contains, Exposes, DependsOn
- **WeightedEdge** with confidence scoring (structural = 1.0, reference-derived = resolver confidence)
- **Impact analysis**: BFS with cumulative confidence, pruning below 0.1 threshold
- **Cycle detection**: Tarjan SCC via `petgraph::algo::tarjan_scc`
- **Composable pipeline**: `GraphAnalyzer` trait + `AnalysisPipeline` with 4 standard passes (degree centrality, entry points, process flows, cycles)

### Key files
- `src/model/graph/` — KnowledgeGraph types, construction, analysis (split into submodules)
- `src/model/graph_analysis.rs` — GraphAnalyzer trait, AnalysisPipeline, 4 standard analyzers

## Research sources

### Academic conferences
- **ICSE** (Int'l Conf on Software Engineering)
- **FSE** (Foundations of Software Engineering)
- **ISSTA** (Int'l Symposium on Software Testing and Analysis)
- **CGO** (Code Generation and Optimization)
- **CC** (Compiler Construction)
- **SOAP** (Static Analysis Symposium)

### Foundational papers
| Paper | Venue | Key Contribution |
|-------|-------|-----------------|
| Code Property Graph (Yamaguchi et al.) | S&P 2014 | Unified AST+CFG+PDG graph for vulnerability detection |
| Louvain method (Blondel et al.) | JSTAT 2008 | Community detection in large networks |
| CHA/RTA/VTA (Dean et al., Bacon & Sweeney) | Various | Call graph construction precision hierarchy |
| Tarjan's SCC | SIAM 1972 | Strongly connected components in O(V+E) |
| PageRank (Brin & Page) | WWW 1998 | Importance scoring applicable to code centrality |

### Open-source projects to monitor
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| petgraph/petgraph | Algorithm additions, performance improvements | Our graph library |
| joernio/joern | Code Property Graph schema, analysis passes | Security-focused code graph |
| github/codeql | Dataflow analysis, taint tracking | Industry-standard graph queries |
| facebookincubator/Glean | Graph storage at scale, incremental updates | Meta's approach to code graphs |
| neo4j/neo4j | Graph query patterns (Cypher), optimization techniques | Graph query language patterns |
| eclipse/openvsx | Dependency graph analysis | Package ecosystem graphs |

## What to evaluate

1. **CPG approach vs current flat graph** — quantify what Code Property Graph adds (CFG, PDG) vs complexity cost
2. **Graph query languages** — Cypher vs custom Rust traversals for developer ergonomics
3. **Scalability at 10K+ nodes** — profile petgraph performance at large monorepo scale
4. **Louvain/spectral clustering** — applicability to automatic module boundary detection
5. **Incremental graph maintenance** — algorithms for updating graph on single-file change without full rebuild
6. **Call graph precision** — CHA vs RTA vs points-to analysis trade-offs without type information
7. **Graph embeddings** — node2vec/GraphSAGE for code similarity and clone detection
8. **Weighted impact propagation** — alternatives to BFS with cumulative confidence (random walks, diffusion)

## Checklist

- [ ] Searched academic venues (ICSE, FSE, ISSTA, SOAP) for recent program graph papers
- [ ] Benchmarked petgraph at realistic scale (measure actual node/edge counts from test repos)
- [ ] Evaluated Code Property Graph approach with concrete complexity vs benefit analysis
- [ ] Tested community detection algorithms (Louvain, label propagation) on real CodeModel graphs
- [ ] Profiled incremental graph update performance (single-file change scenario)
- [ ] Compared call graph construction techniques available without type information
- [ ] Verified that recommendations preserve weighted edge confidence model
- [ ] Documented rejected alternatives with reasons

## Output format

```markdown
## Research Report: <topic>

### Research Question
<What specific question was investigated>

### Search Scope
- **Conferences/Journals:** <which ones searched>
- **Repositories:** <which OSS projects examined>
- **Articles/Blogs:** <which sources consulted>
- **Date range:** <timeframe of search>

### Findings

#### Papers
| Paper | Venue/Year | Relevance | Key Technique |
|-------|-----------|-----------|---------------|
| <title> | <venue year> | HIGH/MEDIUM/LOW | <what it proposes> |

#### Open Source Projects
| Project | Stars/Activity | License | Relevance | What to Learn |
|---------|---------------|---------|-----------|---------------|
| <name> | <metrics> | <license> | HIGH/MEDIUM/LOW | <specific technique or pattern> |

#### Techniques Discovered
- **<technique name>**: <description> — Applicability: HIGH/MEDIUM/LOW

### Impact on intently-core
| Module | Current Approach | Potential Improvement | Effort |
|--------|-----------------|----------------------|--------|
| <file path> | <what we do now> | <what we could do> | S/M/L |

### Recommendations

#### Adopt (high confidence, proven technique)
- <recommendation with justification>

#### Investigate Further (promising but needs spike)
- <recommendation with what spike would validate>

#### Avoid (evaluated and rejected)
- <what and why rejected>

### References
- [1] <full citation or URL>
```
