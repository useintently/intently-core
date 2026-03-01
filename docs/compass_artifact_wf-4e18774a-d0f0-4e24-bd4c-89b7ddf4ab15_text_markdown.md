# Intently platform: six architecture decisions that shape everything

Intently's governance engine faces six foundational design choices—from multi-language parsing to LLM sandbox atomicity—that will determine whether the platform scales gracefully or accumulates irreversible technical debt. This report synthesizes real-world patterns from **Semgrep, CodeQL, ast-grep, LangGraph, Temporal, OpenHands, Datadog, Renovate**, and dozens of other production systems to provide concrete implementation blueprints for each decision. The overarching recommendation: **start with the simplest pattern that preserves the right abstractions**, and let each component evolve independently along a well-defined migration path.

---

## 1. Multi-language static analysis: tree-sitter CSTs beat unified IRs for governance

### How the major tools solve this

Three dominant architectures exist for multi-language static analysis, each with sharply different tradeoffs:

**Semgrep** parses each language through tree-sitter into a language-specific AST, then translates that into a **Generic AST (AST_generic)**—a unified type that all languages share. Pattern matching and taint tracking run against this single representation. The cost is enormous: Semgrep's codebase contains **6,000–10,000 lines of OCaml translator code per language** (e.g., `go_to_generic.ml`). Each rule specifies a `languages` list, but patterns must be syntactically valid for all listed languages—so you can combine `[javascript, typescript]` but never `[python, go]`. For unsupported languages, a `generic` mode does text-level matching with limited precision.

**CodeQL** takes a relational-database approach. Per-language *extractors* produce TRAP files conforming to language-specific database schemas. Queries are written in QL, a Datalog dialect. The critical innovation is **Models-as-Data**: sinks, sources, and flow summaries defined in YAML data files that any security query automatically picks up. A `log-injection` sink defined for JavaScript doesn't require writing QL—just a YAML entry mapping `console.log.Argument[0]` to the `log-injection` sink kind. The shared dataflow framework is implemented once and instantiated per language through semantic adapter modules.

**ast-grep** operates directly on tree-sitter's **Concrete Syntax Trees** with no intermediate representation. Rules are YAML with CSS-selector-like composition (`all`, `any`, `not`, `has`, `inside`, `follows`). Adding a language means adding a tree-sitter grammar dependency—often zero translator code. The tradeoff: rules are strictly language-specific, and there's no dataflow or taint analysis. But it's Rust-native, extremely fast, and supports **23+ built-in languages** plus custom grammars via dynamic `.so` loading.

### The architecture Intently should adopt

**True language-agnostic rules are a myth at the syntax level.** `logging.info(secret)` in Python, `log.Info(secret)` in Go, and `console.log(secret)` in TypeScript have fundamentally different AST structures. The practical solution is a **two-layer architecture**: a language-agnostic *rule intent* layer (shared ID, category, severity, description) paired with language-specific *pattern bindings*.

For sink scanners—detecting logger calls, HTTP clients, secret exposure—the recommended pattern is CodeQL's Models-as-Data approach adapted to YAML catalogs:

```yaml
# sinks/loggers.yml
category: log-call
sinks:
  - language: python
    patterns: ["logging.$METHOD($$$ARGS)", "logger.$METHOD($$$ARGS)"]
  - language: typescript
    patterns: ["console.$METHOD($$$ARGS)", "winston.$LOGGER.$METHOD($$$ARGS)"]
  - language: go
    patterns: ["log.$METHOD($$$ARGS)", "$LOGGER.$METHOD($$$ARGS)"]
```

This decouples sink knowledge from rule logic. Adding a new language to the sink scanner requires only data—no engine changes.

### MVP recommendation

Use **tree-sitter for parsing** (Rust crate, direct integration) with **ast-grep-core as a library** for pattern matching. Write rules in YAML per language. Build a Rust `LanguageConfig` registry that maps file extensions to tree-sitter grammars. Start with `tree-sitter-python` and `tree-sitter-typescript`. Adding Go and Java later means adding two crate dependencies and writing YAML rule files—**no engine changes required**. For Phase 3, build a lightweight `NormalizedCall` struct (receiver, method, arguments, location) to enable cross-language queries against a common model without the cost of a full generic AST.

### What to avoid

**Do not build a generic AST.** Semgrep's approach required years of investment and thousands of lines of translator code per language. For governance-focused policy enforcement (not deep vulnerability research), tree-sitter CST + YAML patterns deliver **90% of the value at 10% of the cost**. Also avoid the temptation to write "universal" pattern syntax that works across languages—it inevitably becomes a leaky abstraction that's worse than explicit per-language patterns.

### Migration path

Phase 1 (MVP): tree-sitter + ast-grep YAML rules for Python/TypeScript → Phase 2: add Go/Java grammars + sink catalog YAML → Phase 3: lightweight normalized call model for cross-language queries → Phase 4: intraprocedural dataflow via scope analysis, type resolution through LSP integration.

---

## 2. Deterministic patch failures need graduated intervention, not binary outcomes

### How production codemod tools handle failure

Every major transformation tool converges on the same fundamental pattern: **return null to skip, throw to abort, return modified source to apply**.

**jscodeshift** (Facebook) wraps recast for AST-to-AST transforms that preserve formatting. If a transform function returns `null` or `undefined`, the file stays unchanged. Parse failures log `ERR <filepath>` and continue. The runner reports `{ ok, nochange, error, skip }`—a built-in triage mechanism. There is no concept of "partial match" or "ambiguous match"; ambiguity handling is the transform author's responsibility via `.filter()` predicates. Facebook's original `codemod.py` pioneered **human-in-the-loop workflow**: each regex match shows a colored diff, and the user accepts, rejects, or edits interactively.

**libCST** (Instagram/Meta) creates lossless Concrete Syntax Trees for Python, preserving all whitespace, comments, and quotes. Its codemod framework provides an explicit `SkipFile("reason")` exception for clean bail-out when unsupported patterns are detected mid-operation. The two-pass approach—gather metadata with `CSTVisitor`, then mutate with `CSTTransformer`—reduces ambiguity. Rich metadata providers (`QualifiedNameProvider`, `ScopeProvider`, `TypeInferenceProvider` via Pyre) give transforms context to make safer decisions.

**Rector** (PHP) leverages PHPStan for type information, enabling type-aware transformations. When types can't be resolved statically, Rector generates **runtime guard code** (e.g., `is_array()` checks) rather than skipping entirely—a defensive middle ground between full transformation and skip.

**OpenRewrite** (Java ecosystem) operates on immutable **Lossless Semantic Trees** using `.withX()` methods that return new nodes. Change detection uses referential equality—if the returned tree is the same object, no change was made. Its `Preconditions.check()` mechanism lets recipes declare prerequisites that must be met before transformation is attempted, serving as both an optimization and a safety gate.

### Measuring patch confidence

No production tool ships a formal confidence score, but the research community has developed approaches. The **APCA** (Automated Patch Correctness Assessment) literature identifies static signals like pattern specificity, type resolution completeness, scope containment, and match uniqueness. A practical confidence model for Intently should score per-hunk (not per-file) across these dimensions:

| Signal | Weight | Description |
|--------|--------|-------------|
| Pattern specificity | High | Exact AST match vs. fuzzy/partial |
| Type resolution | High | Fully resolved vs. `any`/unknown |
| Scope containment | Medium | Local scope vs. exported API surface |
| Match uniqueness | Medium | Single unambiguous match vs. multiple candidates |
| Test coverage | High | Transformed code has tests vs. untested |

Map scores to tiers: **0.9–1.0** auto-apply with logging, **0.7–0.89** auto-apply requiring CI pass, **0.4–0.69** generate PR requiring review, **below 0.4** flag for manual intervention with no auto-change.

### The graduated intervention chain

The most mature implementation comes from **Codemod 2.0**, which demonstrated that vanilla GPT-4o achieves only **45% accuracy** on codemod generation, but with 3 refinement iterations using deterministic static/dynamic feedback, accuracy jumps to **75%**. Uber's **Piranha** pipeline for feature flag cleanup provides production-scale evidence: of 1,381 generated diffs, **65% landed without any changes** (high confidence), **85%+ compiled and passed tests**, and 88% were processed by developers.

The recommended escalation chain for Intently:

1. **Regex/string replacement** — fastest, highest confidence for simple patterns
2. **AST pattern matching** (ast-grep YAML rules) — deterministic structural matching
3. **Imperative AST transform** (libCST/jscodeshift) — programmatic, handles complex logic
4. **LLM-assisted transform** with deterministic validation loop (compile + test + AST diff)
5. **Human review** — PR with annotated diff, confidence scores, and context

### What to avoid

**Never try to land one massive change.** Follow Google's Rosie pattern: shard large changes into per-team or per-directory PRs. Don't build transforms that silently modify code without a dry-run preview—every tool in this space provides `--dry-run` for good reason. Avoid scoring confidence at the file level; a file may have 3 high-confidence changes and 1 dangerous one. And resist the temptation to automate everything—even Uber's Piranha only handles Boolean and Update flag APIs, explicitly scoping out parameter APIs where the engineering effort vastly exceeded the payoff.

### Migration path

MVP: ast-grep YAML rules with dry-run mode + `{ ok, nochange, error, skip }` reporting → add libCST for Python transforms with `SkipFile` pattern → implement per-hunk confidence scoring → add LLM fallback with validation loop → build Piranha-style rule graphs for cascading cleanup chains.

---

## 3. Sandbox atomicity starts with git branches and evolves to overlayfs

### What AI coding agents actually do

A surprising finding: **none of the major AI coding agents implement mid-task atomic rollback at the filesystem level**. They all use one of three strategies:

**SWE-Agent** (Princeton/Stanford) uses `SWE-ReX`, a sandboxed execution framework that starts a Docker container or remote VM. Within the container, the agent works on a git branch. If the task fails, the container is discarded—the original repo state is preserved as the base commit. The architecture is explicitly simple: "literally just switch out `subprocess.run` with `docker exec`."

**OpenHands** (formerly OpenDevin) evolved from V0 (SSH-accessed Docker containers with workspace mounts, plagued by divergent agent/sandbox state) to V1 (event-sourced state management with modular workspace support). Containerization ensures "agents are restricted to their own Docker-based environment, torn down post-session, ensuring filesystem integrity." But there is no mid-task atomic rollback—if a task fails, the workspace is discarded.

**Devin** (Cognition Labs) uses the **pull request as the atomicity boundary**. Changes exist only on a feature branch until approved. If a task fails, the branch is abandoned.

**E2B** provides the most sophisticated infrastructure: **Firecracker microVMs** (not Docker) with sub-200ms startup and <5 MiB memory overhead. E2B uses **OverlayFS inside microVMs** to share read-only root filesystems across instances, with per-sandbox upper layers capturing changes.

### The overlayfs pattern for atomic code changes

OverlayFS provides the strongest atomicity guarantee for Intently's use case. The architecture: a read-only `lowerdir` (original repo state) + a read-write `upperdir` (LLM's changes) + a `merged` directory presenting a unified view. The LLM process operates on the merged view. **The base repository is never modified until explicit commit.** Rollback is literally `rm -rf upperdir`—instant, zero residue. Multiple overlays can share the same lowerdir simultaneously, enabling parallel task execution on the same repo.

The **anoek/sandbox** tool (MIT license, Rust) implements exactly this pattern: "lightweight containerized copy-on-write views of your computer" with `sandbox sync`, `sandbox accept`, and `sandbox reject` commands. It's the closest existing implementation to what Intently needs.

For performance: copy-up is file-level (modifying 1 byte of a large file copies the entire file), but source code repos consist of many small files—the ideal workload for OverlayFS.

### MVP recommendation

**Week 1**: Use git branch per task as the atomicity boundary. Run LLM in a Docker container with the repo mounted. On success: squash-merge to main, create PR. On failure: delete the branch, discard the container. This is what Devin, OpenHands, and SWE-Agent all do.

**Weeks 2–3**: Add OverlayFS inside the Docker container. Base repo is lowerdir (bind-mounted read-only). LLM writes go to upperdir only. Validation runs against the merged view. Commit = extract upperdir changes into git commit. Rollback = delete upperdir.

### What to avoid

Don't attempt filesystem-level transactions with write-ahead logs—the complexity isn't justified when OverlayFS provides a cleaner abstraction. Don't trust LLM processes to clean up after themselves on failure; always assume the process may crash at any point and design the sandbox so that the only way changes reach the real repo is through an explicit commit step. Avoid running multiple LLM tasks on the same git worktree without isolation—use separate overlays or branches.

### Migration path

Git branch + Docker (MVP) → OverlayFS inside Docker with tmpfs-backed upper layer → warm container pool with pre-cloned repos → Kubernetes orchestration with `agent-sandbox` CRD pattern → Firecracker microVMs (E2B-style) if security requirements warrant hardware-level isolation.

---

## 4. LLM task orchestration needs typed state, not conversation history

### How agent frameworks pass context between steps

**LangGraph** treats **state as the central mechanism** for inter-node communication. State is a `TypedDict` or Pydantic `BaseModel` passed along graph edges. Nodes return only changed fields; LangGraph merges updates automatically. Reducer functions (e.g., `operator.add` for list concatenation) control how concurrent updates merge. Built-in checkpointing saves state at every node boundary, enabling fault tolerance, human-in-the-loop pauses, and time-travel debugging. Production backends include PostgresSaver and RedisSaver.

**CrewAI** automatically relays the output of one task as context to the next in sequential mode. For non-adjacent dependencies, tasks declare `context=[task1, task2]` explicitly. Outputs are encapsulated in `TaskOutput` supporting raw text, JSON, and Pydantic models—structured outputs enable type-safe inter-step communication.

**AutoGen** (Microsoft) treats workflows as dialogues—context flows through conversation history. This creates a fundamental problem: **context grows linearly with each step**. Community patterns suggest using a centralized typed state container rather than raw message history, achieving ~80% reduction in API tokens.

### The plan-execute-replan pattern

This is the recommended orchestration pattern for Intently. A **planner** LLM generates a multi-step governance plan (structured output). **Executors** run individual steps. A **replanner** inspects results after each step and either updates the remaining plan or returns a final answer. LangGraph implements this as a cyclic graph with conditional edges routing between executor and replanner nodes.

Advantages over pure ReAct: sub-tasks execute without consulting the large LLM each time (faster, cheaper); the executor can use smaller models; forcing explicit planning improves reasoning quality; and separation of planner (no tool access) from executor (scoped tool access) provides prompt injection resistance. UC Berkeley's **LLMCompiler** variant streams the planner output as a DAG where tasks execute in parallel once dependencies are met.

### Context window management is critical

The research identifies five concrete risks: **context poisoning** (early hallucinations propagate), **context distraction** (overwhelming context degrades attention), **context confusion** (superfluous context misdirects responses), and linear cost/latency scaling. JetBrains Research (NeurIPS 2025) found that **observation masking**—replacing older tool outputs with "...omitted" while preserving action and reasoning history—is highly effective, since observations (test logs, file reads) consume the most tokens.

The recommended strategy for Intently combines CI/CD pipeline wisdom with LLM-specific techniques:

- **Small data** (scores, decisions, summaries) → graph state parameters (Argo Workflows model)
- **Large data** (full code analysis, detailed reports) → external store with reference IDs (Argo artifacts model)
- **Each step** declares which prior step outputs it needs (CrewAI's `context=[...]` pattern)
- **Step boundaries** trigger summarization: compress full output into structured summary before passing forward
- **Temporal's durable execution** pattern inspires checkpoint-based recovery with idempotency keys

### MVP recommendation

Use LangGraph as the core orchestration engine with a typed `IntentlyGovernanceState` schema. Each governance step reads only declared dependencies from state, writes structured `StepResult` summaries (< 500 tokens each), and stores full outputs in an external database referenced by ID. Implement plan-execute-replan with conditional edges. Use PostgresSaver for production checkpointing. Track `context_budget_tokens` in state to trigger compression when approaching limits.

### What to avoid

Don't pass raw conversation history between steps—use typed state fields with explicit schemas. Don't let context accumulate unboundedly; implement hard token budgets with automatic summarization. Avoid AutoGen's conversation-centric approach for deterministic governance pipelines—it's designed for exploratory dialogue, not structured workflows. Don't build custom orchestration from scratch when LangGraph provides checkpointing, conditional routing, and typed state out of the box.

### Migration path

LangGraph with InMemorySaver (MVP) → PostgresSaver for persistence → add step-boundary summarization → implement observation masking for verbose tool outputs → add LLMCompiler-style parallel execution for independent governance checks → Temporal for enterprise durability requirements.

---

## 5. Progressive disclosure follows the "no dead ends" principle

### Patterns from production observability tools

**Datadog** built DRUIDS, a **600+ component React design system**, around the principle that "additional context should always be close at hand." Every point in the UI provides paths to drill deeper—there is never a dead end. Their traffic-light color system (green/yellow/red) requires zero explanation and serves as the simplest information layer before drill-down. Cmd+K universal search enables jumping between any view instantly.

**New Relic** implements three distinct view modes that represent the gold standard for zoom levels. The **Entity Explorer** (list view) shows all monitored entities organized by category. The **Navigator** displays a high-density honeycomb visualization with traffic-light colors—an intermediate zoom level showing health patterns at a glance. The **Lookout** shows entities with *recent performance changes*, sized and colored by deviation rate. Critically, New Relic's service maps support **configurable depth (1–3 hops)**, and depths 2–3 **only show degraded entities** to prevent visual clutter. This is progressive disclosure at the architectural level—deeper views are filtered to show only what matters.

**Linear** follows "simple first, then powerful"—opinionated defaults with one good way of doing things rather than infinite configurability. Speed is treated as a UX feature: instant response enables fluid exploration, which is itself a form of progressive disclosure. Keyboard-first interaction coexists with visual UI, implicitly serving two expertise levels.

**Grafana** formalizes drill-down through its `SceneAppDrilldownView` API: nested pages where users progressively explore from overview to underlying data, with URL-parameterized breadcrumbs. Collapsible dashboard rows suppress queries for hidden panels—a critical performance optimization. Template variables filter data dynamically across all panels.

### Designing for mixed audiences

Staff engineers and mid-level developers have fundamentally different needs. Mid-level developers want **"what do I need to fix?"**—actionable guidance with lower information density. Staff engineers want **"what patterns am I seeing?"**—systemic understanding with raw data access. The solution isn't separate interfaces but **layered information architecture**:

- **Layer 1 (Executive)**: KPIs, traffic-light status, trend arrows—scannable in <5 seconds. Both audiences start here.
- **Layer 2 (Analytical)**: Charts showing trends, comparisons, distributions. Mid-level developers often stop here.
- **Layer 3 (Investigation)**: Raw data, query builders, full context. Staff engineers live here during deep analysis.

Role-based *default views* set appropriate starting points without restricting access. JetBrains' New UI explicitly documents its progressive disclosure strategy: "reduce visual complexity, provide easy access to essential features, and progressively disclose complex functionality as needed." Their viewing modes (Distraction-free, Zen, Compact, Presentation) demonstrate density-aware simplification.

### MVP recommendation for Intently's Tauri v2 app

Use **React + TypeScript + Vite** as the frontend stack (largest ecosystem, proven by Linear's own architecture). Implement four zoom levels: System Overview (honeycomb/heatmap of all repos), Project Level (governance scores and finding summaries), Component Level (specific findings with filtering), and Detail Level (individual findings with code context and remediation). Build with **Radix UI primitives** for accessibility, **Recharts or Apache ECharts** for visualization, and **Zustand** for lightweight state management. Leverage Tauri's Rust backend for all governance computation, streaming results via IPC events.

Key patterns to implement: Cmd+K quick navigation, collapsible sections that suppress data loading when collapsed, breadcrumb navigation preserving drill-down context, template variables for filtering by team/project/severity, and dismissible "Why this matters" tooltips for contextual learning.

### What to avoid

Don't offer 10 ways to view the same data—follow Linear's opinionated approach. Don't show tooltips and onboarding hotspots everywhere simultaneously (causes frustration and churn). Don't build separate "beginner" and "expert" interfaces—use a single interface with layered depth. Avoid heavyweight charting libraries that degrade Tauri's webview performance; test rendering performance with realistic data volumes early.

### Migration path

MVP: three zoom levels with collapsible detail panels → add role-based default views → implement Cmd+K navigation → add viewing modes (Standard, Summary, Focus) → build interactive query capability for staff engineers → add CodeScene-style hierarchical hotspot visualization for codebase health.

---

## 6. Bootstrapping intent.yaml follows the "propose, review, refine" pattern

### How existing tools bootstrap configuration

**Terraform** evolved from per-resource CLI import (which only updated state, never generating config) to import blocks (plannable, bulk-capable) and experimental `-generate-config-out` (generates HCL as a template). The generated config includes **all possible attributes** including defaults—requiring significant manual pruning. It cannot detect relationships between resources, doesn't know which attributes are safely skippable, and produced config risks causing drift if it doesn't match live state exactly.

**Pulumi** generates idiomatic code in the user's chosen language (TypeScript, Python, Go, C#) from provider state. Imported resources are **marked as protected by default** to prevent accidental deletion—a critical safety pattern. Since v3.26, generated code omits default values for cleaner output.

**Renovate** provides the strongest analog for Intently's use case. When first enabled, it creates an **onboarding PR** proposing a `renovate.json` with auto-detected defaults. The PR description explains what was detected and why. Users review and merge. A `config:recommended` preset works for most cases, and users incrementally add `packageRules` for specific needs. The preset system supports organization-wide inheritance via `extends` arrays. A Dependency Dashboard (GitHub issue) provides ongoing interactive management.

### Detection strategies

Framework detection follows the **signals-based approach** pioneered by Netlify's `framework-info`: JSON descriptors specify detection signals (package.json dependencies, config files), confidence weights per signal, default build commands, and expected directory structure. **GitHub Linguist** handles language detection via file extensions, shebangs, and content analysis. Package manifest files (`package.json`, `pyproject.toml`, `go.mod`, `Cargo.toml`) provide dependency information, with lock files being more reliable for exact versions. API endpoint detection is framework-specific: Express routes, FastAPI decorators, Spring `@RequestMapping`, Django URL patterns each require different extraction logic.

Schema inference varies dramatically by framework. **FastAPI generates OpenAPI specs fully automatically** from type hints and decorators—zero configuration. **springdoc-openapi** is mostly automatic via reflection. **swagger-jsdoc** requires explicit JSDoc annotations. The lesson: the right inference level depends on the framework, and framework detection should drive which analysis strategies Intently applies.

### The risks of auto-generated intent

Five common failure modes threaten auto-generated configuration: **false positives** (detecting a framework that's a dev dependency but not actually used), **missing business context** (auto-detection can't capture *why* something is configured a certain way), **over-verbosity** (Terraform's all-attributes problem creates review fatigue), **stale detection** (dependencies installed but never configured), and **drift** between detection-time snapshot and evolving code. Every production tool mitigates these through preview-before-apply mechanisms, protection defaults, and explicit override capabilities.

### MVP recommendation

Implement a four-phase system: **Scan** (language detection + manifest parsing + framework detection + route extraction), **Generate** (`intent.yaml` with confidence annotations per field), **Propose** (Renovate-style onboarding PR explaining what was detected and why), and **Refine** (user reviews, edits, commits; subsequent scans preserve manual changes).

Each auto-detected item should carry a confidence tag: **High** (multiple corroborating signals, e.g., both `next.config.js` and `next` dependency), **Medium** (single signal), **Low** (heuristic inference). Items below a configurable threshold should be commented out by default. Support `# Intently:ignore` annotations to suppress specific detections and `# Intently:manual` to protect hand-authored entries from re-scan overwrite.

```yaml
# Auto-generated by Intently — review and commit
services:
  - name: api-service          # confidence: high
    framework: express         # detected from package.json
    language: typescript       # detected via file analysis
    entry_point: src/index.ts  # confidence: medium
    apis:
      - path: /api/users       # detected from route analysis
        methods: [GET, POST]
```

### What to avoid

Don't generate config silently—always go through a review mechanism (PR, CLI preview, or dashboard). Don't overwrite manual changes on re-scan; use a merge strategy that proposes additions and warns about removals without acting on them. Don't attempt to detect business intent (team ownership, SLA requirements, deployment targets) automatically—these require human declaration. Avoid the Dependabot anti-pattern of requiring fully manual config authoring; the bootstrapping step is critical for adoption.

### Migration path

MVP: file-based language/framework detection + manifest parsing → generate `intent.yaml` via CLI with confidence annotations → Renovate-style onboarding PR → add framework-specific API route extraction → implement layered presets (org-wide → team → repo) → add re-scan diffing with merge strategy → integrate with Intently's governance engine to validate intent against actual code state.

---

## Conclusion

Six architectural choices, one unifying principle: **preserve the right abstraction boundaries so each component can evolve independently**. The tree-sitter CST approach for multi-language analysis avoids the generic AST trap while keeping the door open for semantic enrichment. Graduated intervention chains for patching acknowledge that deterministic transforms handle the bulk (Uber proved 65% auto-land rates) while LLMs fill the gaps. Git branches provide sufficient atomicity for an MVP sandbox—OverlayFS adds true copy-on-write isolation when needed. Typed state schemas for LLM orchestration prevent the context bloat that plagues conversation-centric approaches. Progressive disclosure follows the "no dead ends" principle rather than building separate interfaces for different expertise levels. And intent bootstrapping succeeds through the "propose, review, refine" cycle rather than either fully manual authoring or silent auto-generation.

The critical non-obvious insight across all six domains: **the tools that scale best are the ones that treat configuration as data, not code**. Semgrep's YAML rules, CodeQL's Models-as-Data, Renovate's preset system, Argo's artifact parameters, and ast-grep's declarative rule format all demonstrate that data-driven architectures are cheaper to extend, easier to validate, and simpler to version-control than programmatic alternatives. Intently should internalize this principle at every layer.