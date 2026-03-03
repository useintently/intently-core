---
name: research-symbol-resolution
description: Researches cross-file analysis, import resolution, and type inference techniques relevant to intently-core. Tracks scope graph theory, demand-driven name resolution, module resolution algorithms, and confidence scoring calibration. Use when investigating resolution accuracy improvements, new language module systems, type inference without compilation, or cross-language reference resolution.
---

# Symbol Resolution Research

## Critical rules

**ALWAYS:**
- Benchmark resolution accuracy against ground truth (IDE resolution or compiler output)
- Evaluate techniques per-language — Python's import system is fundamentally different from Go's
- Measure both precision (correct resolutions) and recall (found resolutions) — both matter
- Check that proposed techniques work WITHOUT full compilation or type checking
- Calibrate confidence scores against actual resolution correctness rates

**NEVER:**
- Recommend full type inference systems — intently-core does static extraction, not compilation
- Ignore the two-level architecture (per-file exact + global fuzzy) — it's a deliberate design choice
- Propose language-specific resolution logic in core modules — use `LanguageBehavior` trait
- Evaluate resolution quality only on happy-path imports — test re-exports, barrel files, wildcard imports
- Break the `ResolutionMethod` → `confidence` mapping without recalibrating all 8 variants

## Current state in intently-core

- **Two-level SymbolTable**: per-file exact lookup + global fuzzy lookup
- **6-level heuristic resolution chain**: import → same-file → global-unique → same-directory → ambiguous → unresolved
- **8 ResolutionMethod variants**: ImportBased (0.95), SameFile (1.0), GlobalUnique (0.80), GlobalSameDir (0.60), GlobalAmbiguous (0.40), External (0.60), Unresolved (0.0)
- **Python stdlib classification**: 130 CPython 3.12 modules via `is_stdlib_module()`
- **Relative import normalization**: Python dot-prefix → JS-style paths for unified resolution
- **Python import extraction**: all 4 forms (simple, dotted, from-import, wildcard)
- **Resolution method distribution**: tracked in `CodeModelStats` for diagnostics

### Key files
- `src/model/symbol_table.rs` — Two-level symbol table with heuristic resolution
- `src/model/import_resolver.rs` — Cross-file import resolution with confidence scoring
- `src/model/extractors/language_behavior.rs` — `LanguageBehavior` trait including `is_stdlib_module()`
- `src/model/module_inference.rs` — Module boundary detection

## Research sources

### Academic conferences
- **PLDI** (Programming Language Design and Implementation)
- **OOPSLA** (Object-Oriented Programming, Systems, Languages, and Applications)
- **ECOOP** (European Conf on Object-Oriented Programming)
- **POPL** (Principles of Programming Languages)
- **SLE** (Software Language Engineering)

### Foundational papers and theories
| Paper/Theory | Venue | Key Contribution |
|-------------|-------|-----------------|
| Scope Graphs (Neron, Tolmach et al.) | ESOP 2015 | Declarative name resolution framework |
| Statix (van Antwerpen et al.) | SLE 2018 | Constraint-based scope graph resolution |
| Demand-driven analysis (Reps et al.) | Various | Resolve only what's queried, not everything |
| Module systems survey (Cardelli) | POPL 1997 | Formal treatment of module systems |

### Open-source projects to monitor
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| rust-analyzer/rust-analyzer | Name resolution algorithm, incremental resolution | IDE-grade resolution patterns |
| microsoft/pyright | Python module resolution, type narrowing | Python-specific resolution rules |
| sorbet/sorbet | Ruby type checking, constant resolution | Ruby name resolution (our weak spot) |
| facebook/flow | JS/TS type inference, module resolution | JS module resolution algorithms |
| davidhalter/jedi | Python completion/resolution, scope analysis | Lightweight Python analysis |
| JetBrains/intellij-community | Multi-language resolution, reference providers | Industrial-strength resolution |
| source-academy/sourcetrail | Cross-reference indexing, graph-based navigation | Resolution visualization |
| spoofax/nabl2 | Scope graph implementation | Reference implementation of scope graphs |

### Language-specific module system docs
- Python: importlib documentation, PEP 328 (relative imports), PEP 302 (import hooks)
- TypeScript: module resolution (classic vs node), path mapping, barrel files
- Go: module system, go.mod replace directives, internal packages
- Java: module system (JPMS), classpath resolution, package naming conventions
- C#: namespace resolution, assembly references, global using directives

## What to evaluate

1. **Scope graph theory** — can Neron et al.'s framework replace our 6-level heuristic chain?
2. **Resolution accuracy benchmarking** — measure our precision/recall against IDE resolution on real repos
3. **Confidence calibration** — are our 8 confidence levels correctly calibrated against actual correctness?
4. **Demand-driven resolution** — resolve only queried symbols vs eager full-project resolution
5. **Barrel file/re-export handling** — techniques for following re-export chains (index.ts, __init__.py)
6. **Cross-language references** — how to resolve JS calling a Rust WASM module, or Python calling C extensions
7. **Stdlib classification** — extend `is_stdlib_module()` beyond Python (Go stdlib, Java JDK, Node.js builtins)
8. **Type narrowing without types** — can control flow analysis improve resolution without full type inference?

## Checklist

- [ ] Searched academic venues (PLDI, OOPSLA, ECOOP, POPL) for recent name resolution papers
- [ ] Evaluated scope graph theory applicability to intently-core's heuristic chain
- [ ] Benchmarked current resolution accuracy against IDE/compiler ground truth
- [ ] Calibrated confidence scores against measured correctness rates
- [ ] Tested proposed techniques on barrel files, re-exports, and wildcard imports
- [ ] Verified per-language techniques use `LanguageBehavior` trait, not core module changes
- [ ] Checked demand-driven vs eager resolution trade-offs for incremental scenarios
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
