---
name: research-semantic-diff
description: Researches structural diff algorithms and behavioral change detection relevant to intently-core. Tracks AST-level diffing (GumTree, Difftastic), rename/move detection, tree edit distance algorithms, and semantic-aware merging. Use when investigating diff accuracy improvements, false positive reduction, rename detection, or merge conflict resolution techniques.
---

# Semantic Diff Research

## Critical rules

**ALWAYS:**
- Evaluate diff algorithms on behavioral change detection (not textual similarity)
- Measure false positive AND false negative rates — a diff that over-reports is as bad as one that under-reports
- Benchmark algorithm complexity against tree size — O(n^3) algorithms don't scale to 10K+ node CodeModels
- Compare against established baselines (GumTree, Difftastic) with reproducible test cases
- Distinguish cosmetic changes (formatting, comments, renames) from behavioral changes (logic, contracts)

**NEVER:**
- Evaluate diff algorithms on raw source text — intently-core diffs operate on the CodeModel IR
- Recommend tree edit distance algorithms without analyzing their complexity class
- Ignore rename/move detection — spurious "removed + added" is the most common false positive
- Propose diff changes that break the `SemanticDiff` → `ExtractionResult.diff` pipeline contract
- Conflate AST-level diff with semantic diff — syntactically different code can be behaviorally identical

## Current state in intently-core

- **SemanticDiff** operates on the CodeModel IR (not raw text or AST)
- **Detects**: added/removed/modified components, changed contracts, new/removed dependencies
- **Cosmetic-only changes** produce empty diffs (formatting, comments don't trigger)
- **Rename/move detection** avoids spurious remove+add for relocated symbols
- **Pipeline integration**: diff is optional in `ExtractionResult` (present when previous model exists)

### Key files
- `src/model/diff.rs` — SemanticDiff implementation, diff computation between CodeModel states

## Research sources

### Academic conferences
- **ICSE** (Int'l Conf on Software Engineering)
- **FSE** (Foundations of Software Engineering)
- **ASE** (Automated Software Engineering)
- **ICSME** (Int'l Conf on Software Maintenance and Evolution)
- **SANER** (Software Analysis, Evolution, and Reengineering)

### Foundational papers
| Paper | Venue | Key Contribution |
|-------|-------|-----------------|
| GumTree (Falleri et al.) | ASE 2014 | AST-level diff with move detection, top-down/bottom-up matching |
| ChangeDistiller (Fluri et al.) | TSE 2007 | Fine-grained source code change extraction |
| MTDIFF (Dotzler & Philippsen) | ASE 2016 | Move-optimized tree differencing |
| Zhang-Shasha | TCS 1989 | Foundational tree edit distance algorithm, O(n^2) |
| RTED (Pawlik & Augsten) | VLDB 2011 | Robust tree edit distance, optimal worst-case |
| APTED (Pawlik & Augsten) | IS 2016 | All-path tree edit distance, improved RTED |

### Open-source projects to monitor
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| GumTreeDiff/gumtree | Matching algorithms, multi-language support | Gold standard for AST diff |
| Wilfred/difftastic | Structural diff display, tree-sitter integration | User-facing structural diff |
| ast-grep/ast-grep | Pattern-based diff, structural matching | Pattern-aware change detection |
| SemanticDiff (commercial) | IDE integration, diff visualization | Commercial state of the art |
| Lisandra-dev/mergiraf | Merge tool using tree-sitter | Semantic-aware merging |
| tree-diff (various) | Tree edit distance implementations | Algorithm benchmarks |

## What to evaluate

1. **GumTree comparison** — false positive/negative rates vs intently-core's current approach
2. **Tree edit distance scaling** — Zhang-Shasha/RTED/APTED complexity vs CodeModel tree size
3. **Rename detection accuracy** — how GumTree's top-down/bottom-up matching compares to current heuristics
4. **Cross-file move detection** — techniques for detecting symbol relocation across files
5. **Diff minimality** — are we producing minimal change sets or inflated ones?
6. **Behavioral equivalence** — techniques for detecting semantically identical but syntactically different code
7. **Merge conflict resolution** — can semantic diff inform automated merge strategies?
8. **Incremental diff** — computing diff deltas from previous diff + new change (not full recomputation)

## Checklist

- [ ] Searched academic venues (ICSE, FSE, ICSME, SANER) for recent diff papers
- [ ] Benchmarked GumTree and Difftastic on representative CodeModel changes
- [ ] Evaluated tree edit distance algorithms (Zhang-Shasha, RTED, APTED) for complexity vs accuracy
- [ ] Measured false positive and false negative rates with concrete test cases
- [ ] Tested rename/move detection accuracy against known refactoring datasets
- [ ] Verified proposed changes preserve the `SemanticDiff` → `ExtractionResult.diff` contract
- [ ] Analyzed scaling behavior at 10K+ node CodeModels
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
