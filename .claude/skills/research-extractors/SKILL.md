---
name: research-extractors
description: Researches framework-specific pattern extraction and language ecosystem evolution relevant to intently-core. Tracks new frameworks, API patterns, decorator/annotation conventions, middleware detection, and metaprogramming challenges. Use when investigating new framework support, extraction coverage gaps, emerging language features, or pattern recognition techniques for code analysis.
---

# Extractors & Framework Research

## Critical rules

**ALWAYS:**
- Track framework adoption metrics (npm/PyPI/crates.io downloads, GitHub stars trend, survey rankings) — don't chase every new framework
- Evaluate extraction difficulty vs user impact — a framework with 1% market share doesn't justify a custom extractor
- Test extraction patterns on REAL projects, not just documentation examples
- Check if existing extractors already partially cover new frameworks (many share routing conventions)
- Verify that new patterns work with tree-sitter CST queries, not regex or string matching

**NEVER:**
- Add a new extractor file for a framework that can be handled by extending an existing one
- Recommend extraction for language features intently-core can't represent in the CodeModel IR
- Chase alpha/beta frameworks — wait for stable releases and real-world adoption
- Ignore metaprogramming (macros, decorators, code generation) — it's the #1 source of extraction gaps
- Propose extractor changes without corresponding integration test fixtures

## Current state in intently-core

- **8 language extractors** covering 16 languages
- **LanguageBehavior trait**: polymorphic dispatch for 9 language families
- **Framework-specific extraction**: Express, NestJS, FastAPI, Flask, Django, Spring Boot, ASP.NET Core, Gin, Echo, net/http, Laravel, Rails
- **Route parameter extraction**: `:param`, `{param}`, `<param>` styles via `extract_path_params()`
- **Handler names, request body types** across 7 language extractors
- **EnvDependency detection** across 8 languages
- **Known gaps**: Go Gin `r.Group()` prefix, PHP `Route::prefix()->group()`, C# `MapGroup()`

### Key files
- `src/model/extractors/mod.rs` — Extractor dispatch by language
- `src/model/extractors/common.rs` — Shared utilities (node text, PII detection, anchoring, `extract_path_params()`)
- `src/model/extractors/language_behavior.rs` — LanguageBehavior trait (9 language families)
- `src/model/extractors/typescript.rs` — Express, NestJS
- `src/model/extractors/python.rs` — FastAPI, Flask, Django
- `src/model/extractors/java.rs` — Spring Boot (also Kotlin)
- `src/model/extractors/csharp.rs` — ASP.NET Core, Minimal API
- `src/model/extractors/go.rs` — Gin, Echo, net/http
- `src/model/extractors/php.rs` — Laravel
- `src/model/extractors/ruby.rs` — Rails
- `src/model/extractors/generic.rs` — Fallback (Rust, C, C++, Swift, Scala)
- `src/model/extractors/env_detection.rs` — EnvDependency extraction

## Research sources

### Ecosystem tracking
- **npm** trending packages, download stats for web frameworks
- **PyPI** download stats, trending packages
- **crates.io** download stats, trending crates
- **State of JS**, **State of Python**, **State of Rust** annual surveys
- **ThoughtWorks Technology Radar** — framework adoption lifecycle
- **GitHub trending** repositories by language
- **StackOverflow Developer Survey** — technology rankings

### Open-source projects to monitor for extraction patterns
| Project | What to Track | Why It Matters |
|---------|--------------|----------------|
| semgrep/semgrep | Rule patterns for framework detection | Pattern recognition techniques |
| github/codeql | Language-specific queries, framework models | Query-based extraction patterns |
| biomejs/biome | JS/TS AST analysis rules | Modern JS/TS patterns |
| astral-sh/ruff | Python rule implementations | Python pattern detection |
| ast-grep/ast-grep | YAML-based structural patterns | Declarative extraction approach |

### Framework watchlist (by language)

**JavaScript/TypeScript:**
- Current: Express, NestJS
- Watch: Next.js (App Router), Remix, Fastify, Hono, tRPC, Elysia

**Python:**
- Current: FastAPI, Flask, Django
- Watch: Litestar, Starlette, Robyn, Sanic

**Rust:**
- Current: generic (log sinks only)
- Watch: Axum, Actix-web, Rocket, Poem

**Java/Kotlin:**
- Current: Spring Boot
- Watch: Quarkus, Micronaut, Ktor, Javalin

**C#:**
- Current: ASP.NET Core (controllers + Minimal API)
- Watch: .NET Aspire, Blazor Server, Carter

**Go:**
- Current: Gin, Echo, net/http
- Watch: Fiber, Chi, Gorilla Mux, Connect (gRPC)

**PHP:**
- Current: Laravel
- Watch: Symfony, Slim, Hyperf

**Ruby:**
- Current: Rails
- Watch: Hanami, Sinatra, Roda

## What to evaluate

1. **New framework routing patterns** — how Next.js App Router, Remix loaders, tRPC procedures differ from traditional REST
2. **Metaprogramming challenges** — Rust proc macros (Axum's `#[handler]`), Python metaclasses, Java annotation processors
3. **Middleware chain detection** — how frameworks compose middleware differently (Express `use()` vs Axum `layer()` vs NestJS interceptors)
4. **DI container analysis** — extracting dependency injection wiring (NestJS modules, Spring beans, .NET DI)
5. **Route composition across files** — multi-file route registration (Next.js file-based routing, Rails `config/routes.rb`)
6. **gRPC/GraphQL patterns** — extraction beyond REST (protobuf service definitions, GraphQL schema/resolvers)
7. **Framework version migration** — how extraction must adapt (Express 4→5, Django 4→5, Spring Boot 2→3)
8. **Extraction coverage measurement** — how to quantify what percentage of a real project's routes/endpoints we capture

## Checklist

- [ ] Checked ecosystem surveys (State of JS/Python/Rust, Technology Radar) for adoption trends
- [ ] Evaluated framework download trends on npm/PyPI/crates.io for prioritization
- [ ] Tested extraction patterns on real open-source projects (not just docs)
- [ ] Verified new patterns work with tree-sitter CST queries (not regex)
- [ ] Checked if existing extractors can be extended (vs creating new ones)
- [ ] Assessed metaprogramming challenges for each recommended framework
- [ ] Confirmed new constructs are representable in current CodeModel IR
- [ ] Documented rejected frameworks with adoption metrics and reasoning

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
