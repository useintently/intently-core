---
name: research-code-model
description: Researches intermediate representation design and code modeling techniques relevant to intently-core. Tracks competing IRs (CodeQL, Kythe, SCIP, LSIF, Glean), representation completeness, serialization formats, and incremental model building strategies. Use when investigating IR design improvements, new construct representation, workspace modeling, or serialization alternatives.
---

# Code Model & IR Design Research

## Critical rules

**ALWAYS:**
- Compare competing IRs (CodeQL, Kythe, SCIP, LSIF, Glean, Semgrep) on the same constructs — apples-to-apples
- Evaluate IR completeness against real codebases, not specification documents
- Check serialization format trade-offs (size, speed, human-readability, schema evolution)
- Verify that any proposed IR changes preserve incremental update capability (O(1) per-file)
- Distinguish what intently-core SHOULD model vs what belongs in downstream consumers (policy, health)

**NEVER:**
- Propose adding constructs to the IR without 2+ concrete consumer use cases (YAGNI)
- Recommend format changes (protobuf, flatbuffers) without benchmarking against current serde_json
- Conflate code representation with code evaluation — the CodeModel is a semantic snapshot, not a policy engine
- Ignore backward compatibility of serialized CodeModel — downstream consumers depend on the JSON schema
- Recommend workspace detection changes without testing against all 5 supported formats

## Current state in intently-core

- **CodeModel IR** contains: Components, Interfaces (routes), Dependencies (HTTP calls), Sinks (log calls), Symbols, DataModels, References, SourceAnchors, ModuleBoundaries, FileTree
- **Incremental builder** with O(1) per-file updates via `CodeModelBuilder::update_file()`
- **Multi-component** support: one `Component` per workspace package
- **SHA-256 content fingerprinting** on `FileExtraction.content_hash`
- **FileRole classification**, token estimation, directory role inference
- **5 workspace detectors**: pnpm, npm/yarn, Cargo, Go, uv
- **Serialization**: serde with serde_json, `#[serde(flatten)]` for SourceAnchor backward compatibility

### Key files
- `src/model/types.rs` — CodeModel, FileExtraction, Component, Interface, SourceAnchor, all IR types
- `src/model/builder.rs` — CodeModelBuilder with incremental per-file updates
- `src/model/file_tree.rs` — FileTree, DirectoryNode, DirectoryRole, DirectoryStats
- `src/workspace/mod.rs` — WorkspaceLayout, WorkspaceKind, WorkspacePackage
- `src/workspace/detect.rs` — 5 manifest parsers

## Research sources

### Academic conferences
- **ICSE** (Int'l Conf on Software Engineering)
- **FSE** (Foundations of Software Engineering)
- **ASE** (Automated Software Engineering)
- **ISSTA** (Int'l Symposium on Software Testing and Analysis)
- **MSR** (Mining Software Repositories)

### Open-source projects to monitor
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| github/codeql | QL database schema, language extractors | Industry-standard code IR |
| kythe-io/kythe | Graph schema, cross-language references | Google's code knowledge graph |
| sourcegraph/scip | Index format, symbol naming conventions | Sourcegraph's code intelligence protocol |
| microsoft/lsif-node | LSIF graph format, hover/definition data | LSP-based code indexing |
| facebookincubator/Glean | Schema language, derived predicates | Meta's code indexing at scale |
| semgrep/semgrep | Generic AST, pattern language | Cross-language rule engine |
| joernio/joern | Code Property Graph (CPG) schema | Security-focused code IR |

### Standards and specifications
- **SARIF** (Static Analysis Results Interchange Format) — OASIS standard
- **LSP** (Language Server Protocol) — Microsoft specification
- **LSIF** (Language Server Index Format) — LSP-derived indexing
- **SCIP** (SCIP Code Intelligence Protocol) — Sourcegraph specification

## What to evaluate

1. **Constructs not yet captured** — generics/templates, closures/lambdas, async flows, macros, type aliases, conditional compilation
2. **Cross-project references** — how SCIP/Kythe handle monorepo-scale cross-package references
3. **Serialization formats** — protobuf vs JSON vs binary for large CodeModels (10K+ files)
4. **Schema evolution** — how competing IRs handle backward-compatible schema changes
5. **Incremental model building** — algorithms for O(1) updates when file content changes
6. **Workspace modeling** — how other tools represent monorepo package relationships and boundaries
7. **Semantic fingerprinting** — content-addressable IR nodes for deduplication and caching
8. **IR completeness metrics** — how to measure what percentage of a codebase's semantics are captured

## Checklist

- [ ] Searched academic venues (ICSE, FSE, ASE, MSR) for recent IR design papers
- [ ] Compared at least 3 competing IRs (CodeQL, SCIP, Kythe, Glean, Joern) on specific constructs
- [ ] Evaluated serialization format trade-offs with concrete benchmark data
- [ ] Checked that proposed changes preserve O(1) incremental updates
- [ ] Verified backward compatibility of any schema changes
- [ ] Tested workspace modeling ideas against all 5 supported formats (pnpm, npm, Cargo, Go, uv)
- [ ] Confirmed new constructs have 2+ concrete consumer use cases
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
