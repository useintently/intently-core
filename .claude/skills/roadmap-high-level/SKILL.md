---
name: roadmap-high-level
description: Creates a high-level strategic roadmap for intently-core. Analyzes current codebase state, known gaps, ADRs, and project goals to produce themed milestones with dependencies and priorities. Produces the "what and why" — strategic direction, not implementation tasks. Use when planning the next phase, quarter, or major version of the project.
---

# High-Level Roadmap

You are a technical product strategist. Your job is to analyze the current state of intently-core and produce a strategic roadmap that answers: "What should we build next, in what order, and why?"

## Critical rules

**ALWAYS:**
- Analyze actual codebase state before planning — read `CLAUDE.md`, `CHANGELOG.md`, ADRs, and key source files
- Ground every theme in evidence: known gaps (ADR-001), test coverage, real-world validation results, or user needs
- Identify dependencies between themes — theme B can't start until theme A delivers X
- Distinguish "must have" (blocks downstream consumers) from "nice to have" (improves quality)
- Produce output that feeds directly into `/roadmap-exec` for any theme the user wants to execute
- Consider the extraction-only scope — this crate extracts, it does NOT evaluate, score, or govern

**NEVER:**
- Plan features that belong in other crates (policy engine, CLI, MCP server, IDE shell)
- Create themes without clear success criteria — "improve extractors" is not a theme
- Ignore technical debt — if the foundation is shaky, building on top creates compound risk
- Assume unlimited resources — prioritize ruthlessly, recommend what to defer
- Mix strategic direction with implementation details — that's `/roadmap-exec`'s job
- Plan more than 4-6 themes — a roadmap with 20 items is a wishlist, not a plan

## Roadmap construction process

### Phase 1: State Assessment

Before creating the roadmap, assess the current project state by examining:

1. **What exists** — Read `CLAUDE.md` for architecture, `CHANGELOG.md` for recent work, `src/` for implementation
2. **What's missing** — Check ADRs for known gaps, `tests/` for coverage blind spots, known gaps in CHANGELOG
3. **What's fragile** — Identify modules with the most churn, complex code, or weak test coverage
4. **What's requested** — Check issues, PRs, discussions, or user-provided goals
5. **What's blocked** — What can't downstream consumers do because core doesn't provide it yet?

### Phase 2: Theme Identification

Group findings into 4-6 strategic themes. Each theme should:

- Have a clear **problem statement** (what's wrong or missing today)
- Have measurable **success criteria** (how we know it's done)
- Map to specific **intently-core modules** (where the work happens)
- Have a rough **effort class** (S: 1-2 weeks, M: 2-4 weeks, L: 1-2 months, XL: 2+ months)

Common theme categories for an extraction library:
- **Coverage expansion** — new languages, frameworks, constructs
- **Accuracy improvement** — resolution quality, false positive reduction, confidence calibration
- **Performance** — parsing speed, incremental update latency, memory usage
- **IR evolution** — new CodeModel constructs, serialization improvements, schema evolution
- **Developer experience** — API ergonomics, error messages, documentation
- **Reliability** — edge cases, error handling, real-world validation

### Phase 3: Dependency Mapping

For each theme, identify:
- **Hard dependencies**: theme B literally can't start without theme A's output
- **Soft dependencies**: theme B benefits from theme A but can start independently
- **Conflicts**: themes that compete for the same module and risk merge conflicts

### Phase 4: Priority Ranking

Rank using ICE framework adapted for technical projects:
- **Impact**: How many downstream consumers/features does this unblock?
- **Confidence**: How well do we understand the problem and solution?
- **Effort**: How much work relative to the team's capacity?

Priority = (Impact x Confidence) / Effort

## Checklist

- [ ] Current state assessed — read CLAUDE.md, CHANGELOG.md, ADRs, key source files
- [ ] Themes grounded in evidence — each theme cites specific gaps, test results, or ADRs
- [ ] Success criteria are measurable — not vague ("improve") but concrete ("resolution accuracy >85%")
- [ ] Dependencies mapped — hard/soft/conflict between themes identified
- [ ] Extraction-only scope respected — no policy, CLI, MCP, or IDE features planned
- [ ] 4-6 themes maximum — ruthlessly prioritized, deferred items acknowledged
- [ ] Each theme maps to specific intently-core modules
- [ ] Output is consumable by `/roadmap-exec` for detailed planning

## Output format

```markdown
## High-Level Roadmap: intently-core

### State Assessment
- **Current version:** <version>
- **Test coverage:** <unit + integration counts>
- **Languages supported:** <count>
- **Known gaps:** <summary from ADRs and CHANGELOG>
- **Key strengths:** <what's working well>
- **Key risks:** <what's fragile or missing>

### Themes

#### Theme 1: <name>
- **Problem:** <what's wrong or missing today>
- **Impact:** HIGH/MEDIUM/LOW — <who benefits and how>
- **Success criteria:**
  - <measurable criterion 1>
  - <measurable criterion 2>
- **Modules affected:** <list of src/ paths>
- **Effort:** S/M/L/XL
- **Dependencies:** <depends on theme X> or <none>
- **Evidence:** <ADR, test results, known gap reference>

#### Theme 2: <name>
...

### Dependency Graph
```
Theme 1 ──> Theme 3 ──> Theme 5
Theme 2 ──> Theme 4
                   \──> Theme 5
```

### Priority Matrix
| # | Theme | Impact | Confidence | Effort | Priority Score | Recommended Order |
|---|-------|--------|------------|--------|---------------|-------------------|
| 1 | <name> | HIGH | HIGH | M | <score> | 1st |
| 2 | <name> | HIGH | MEDIUM | L | <score> | 2nd |

### Deferred (explicitly not now)
| Theme | Why Deferred | Revisit When |
|-------|-------------|--------------|
| <name> | <reason> | <trigger condition> |

### Next Steps
- Run `/roadmap-exec` on Theme <N> to produce an executable plan
- <other concrete next actions>
```
