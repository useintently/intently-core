---
name: research-parsing
description: Researches parsing and AST techniques relevant to intently-core. Tracks tree-sitter evolution, incremental parsing algorithms, grammar quality, CST/AST alternatives, and error recovery strategies. Use when investigating new grammars, parsing performance, language detection improvements, or alternative parser architectures.
---

# Parsing & AST Research

## Critical rules

**ALWAYS:**
- Search academic venues (PLDI, OOPSLA, CC, SLE) AND open-source projects — neither alone gives the full picture
- Evaluate grammar quality by error recovery behavior, not just happy-path parsing
- Benchmark findings against intently-core's current tree-sitter setup before recommending changes
- Check grammar maintenance status (last release, open issues, CI) before recommending adoption
- Cite specific papers, commits, or releases — vague "it's better" claims are worthless

**NEVER:**
- Recommend replacing tree-sitter without concrete evidence of superior error recovery + incremental parsing
- Evaluate parsers by syntax coverage alone — intently-core needs CST positions for SourceAnchor
- Ignore grammar versioning — tree-sitter grammar breaking changes can silently corrupt extraction
- Recommend single-language parsers (swc, oxc, rustpython-parser) as replacements — intently-core needs 16-language coverage
- Present findings without checking applicability to intently-core's thread-local cached parser architecture

## Current state in intently-core

- **16 tree-sitter grammars** with thread-local cached parsers (`parse_source_cached()`)
- **Incremental parsing** via `similar` crate byte-level diffing → `InputEdit` computation
- **Language detection** heuristic in `parser/mod.rs` based on file extension + content sniffing
- **CST positions** feed `SourceAnchor` on all extracted artifacts (file, line, byte positions)
- **Grammar versions** (as of 2026-03): java 0.23.5, c-sharp 0.23.1, ruby 0.23.1, cpp 0.23.4, swift 0.7.1, php 0.24.2

### Key files
- `src/parser/mod.rs` — `SupportedLanguage` enum, `detect_language()`, `parse_source()`, `parse_source_cached()`

## Research sources

### Academic conferences
- **PLDI** (Programming Language Design and Implementation)
- **OOPSLA** (Object-Oriented Programming, Systems, Languages, and Applications)
- **CC** (Compiler Construction)
- **SLE** (Software Language Engineering)
- **ICSE** — software engineering perspective on parsing

### Open-source projects to monitor
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| tree-sitter/tree-sitter | Core releases, API changes, WASM support | Our parsing foundation |
| tree-sitter grammars (16) | Version bumps, breaking changes, new queries | Direct extraction impact |
| ast-grep/ast-grep | Pattern engine improvements, rule language | Already used for structural search |
| nickel-lang/topiary | Tree-sitter-based formatting — grammar quality signals | Grammar maturity indicator |
| biomejs/biome | Unified JS/TS/JSON parser, error recovery | Alternative for JS ecosystem |
| nickel-lang/tree-sitter-nickel | Grammar test methodology | Testing patterns for grammars |
| oxc-project/oxc | JS/TS parser with AST, linter integration | Performance benchmark |
| swc-project/swc | TS/JS AST, speed optimizations | Incremental parsing techniques |
| lezer-parser/lezer | CodeMirror's incremental parser (Marijn Haverbeke) | Alternative incremental approach |
| rust-analyzer/rust-analyzer | IDE-grade Rust parsing, error recovery | Name resolution patterns |

### Blogs and talks
- tree-sitter GitHub discussions and release notes
- Max Brunsfeld's talks on incremental parsing design
- ast-grep blog (Herrington Darkholme)
- Rust analyzer devlogs (matklad/Alex Kladov)
- Lezer design documents (Marijn Haverbeke)

## What to evaluate

1. **Grammar maturity per language** — error recovery quality, test coverage, maintenance cadence
2. **Incremental parsing correctness** — does `InputEdit` produce correct CSTs for all edit types?
3. **Parse speed benchmarks** — baseline parse time per language on representative files (1K, 10K, 100K lines)
4. **Error recovery strategies** — how do different parsers handle malformed input during editing?
5. **New grammar releases** — breaking changes, new node types, query compatibility
6. **GLR vs PEG vs packrat** — trade-offs for ambiguous grammars (C++, Scala)
7. **CST-to-AST lowering** — techniques for simplifying tree-sitter CSTs without losing position data
8. **Language detection improvements** — shebang parsing, content heuristics beyond file extension

## Checklist

- [ ] Searched academic venues (PLDI, OOPSLA, CC, SLE) for recent parsing papers
- [ ] Checked tree-sitter core and all 16 grammar repos for new releases
- [ ] Monitored ast-grep, Biome, oxc, swc, Lezer for relevant techniques
- [ ] Evaluated findings against current thread-local cached parser architecture
- [ ] Benchmarked any recommended changes against current parse times
- [ ] Checked grammar maintenance status (last release date, open issues, CI)
- [ ] Verified CST position data preservation for SourceAnchor compatibility
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
