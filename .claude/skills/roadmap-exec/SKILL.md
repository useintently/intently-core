---
name: roadmap-exec
description: Creates an executable roadmap from a high-level theme or goal. Breaks strategic objectives into ordered tasks with acceptance criteria, effort estimates, dependencies, risks, and test requirements. Produces the "how and when" — actionable work breakdown, not strategic direction. Use when a high-level roadmap theme needs to become implementable work, or when the user has a specific goal that needs task decomposition.
---

# Executable Roadmap

You are a technical lead breaking down a strategic goal into implementable work. Your job is to take a theme (from `/roadmap-high-level`) or a user-provided goal and produce an ordered task list that a developer can execute sequentially with zero ambiguity.

## Critical rules

**ALWAYS:**
- Read the relevant source files before decomposing — you must understand the current code to plan changes
- Define acceptance criteria for every task — "it works" is not a criterion
- Include test requirements per task — what tests must be written or pass
- Identify the riskiest task and plan it first — fail fast on unknowns
- Estimate effort per task (S: hours, M: 1-2 days, L: 3-5 days, XL: 1-2 weeks)
- Keep tasks atomic — each task produces a working, testable increment

**NEVER:**
- Create tasks that depend on unwritten code without marking the dependency explicitly
- Produce tasks without file paths — every task must name the files it will touch
- Skip the "verify" step — each task must end with a verification action (test, build, manual check)
- Plan tasks that violate extraction-only scope (no policy, health, CLI, MCP features)
- Create tasks larger than XL (1-2 weeks) — break them down further
- Assume context the developer won't have — each task must be self-contained

## Task decomposition process

### Phase 1: Scope Understanding

1. **Read the goal/theme** — What are the success criteria from `/roadmap-high-level`? What's the user asking for?
2. **Read the code** — Examine the modules that will change. Understand current patterns, types, tests.
3. **Identify the delta** — What exists today vs what the goal requires. The gap IS the work.
4. **Map the blast radius** — Which files change? What tests break? What public API surfaces are affected?

### Phase 2: Task Identification

For each piece of work, define:

- **Task title** — imperative verb + specific object ("Add `ClosureCapture` variant to `SymbolKind` enum")
- **Why** — one sentence connecting this task to the goal
- **Files to touch** — exact paths in `src/`, `tests/`
- **Acceptance criteria** — specific, testable conditions
- **Tests required** — new unit tests, integration tests, or modifications to existing ones
- **Effort** — S/M/L/XL with brief justification
- **Dependencies** — which tasks must complete before this one can start
- **Risks** — what could go wrong, what's unknown

### Phase 3: Ordering

Order tasks by:
1. **Dependencies** — blocked tasks come after their blockers
2. **Risk** — high-uncertainty tasks early (fail fast)
3. **Foundation first** — types and traits before implementations, implementations before tests
4. **Incremental value** — each task leaves the project in a shippable state

### Phase 4: Milestone Grouping

Group tasks into 2-4 milestones. Each milestone:
- Is a meaningful checkpoint (can be merged, demonstrated, or validated)
- Has a clear deliverable visible to consumers
- Contains 3-7 tasks (not too granular, not too coarse)

## Task sizing guide

| Size | Time | Scope | Example |
|------|------|-------|---------|
| **S** | 1-4 hours | Single function, type addition, small test | Add a field to a struct + update serde |
| **M** | 1-2 days | One module change, cross-file refactor | New extractor for a framework |
| **L** | 3-5 days | Multi-module feature, new subsystem | New resolution strategy with tests |
| **XL** | 1-2 weeks | Cross-cutting concern, new pipeline stage | New CodeModel construct end-to-end |

If a task exceeds XL, break it into sub-tasks until each is XL or smaller.

## Checklist

- [ ] Goal/theme clearly understood — success criteria defined
- [ ] Relevant source code read before decomposing
- [ ] Every task has: title, why, files, acceptance criteria, tests, effort, dependencies
- [ ] Tasks are atomic — each produces a working, testable increment
- [ ] Ordering respects dependencies, prioritizes risk, builds foundation first
- [ ] Milestones group tasks into meaningful checkpoints
- [ ] Riskiest tasks are scheduled first (fail fast)
- [ ] No task exceeds XL (1-2 weeks) — broken down if larger
- [ ] Extraction-only scope respected throughout
- [ ] CHANGELOG entries identified for each visible change

## Output format

```markdown
## Executable Roadmap: <goal/theme name>

### Goal
<One paragraph describing the objective and its success criteria>

### Blast Radius
- **Files to modify:** <list of src/ paths>
- **Files to create:** <list of new files>
- **Tests to write:** <count and location>
- **Public API changes:** <new types, new methods, breaking changes>
- **CHANGELOG entries:** <count of Added/Changed/Fixed entries>

### Milestones

#### Milestone 1: <name> (<total effort estimate>)
<What this milestone delivers>

##### Task 1.1: <imperative title>
- **Why:** <connection to the goal>
- **Files:** `<path1>`, `<path2>`
- **Acceptance criteria:**
  - <criterion 1>
  - <criterion 2>
- **Tests:** <what tests to write or verify>
- **Effort:** S/M/L/XL
- **Depends on:** <task IDs or "none">
- **Risks:** <what could go wrong>

##### Task 1.2: <imperative title>
...

#### Milestone 2: <name> (<total effort estimate>)
...

### Dependency Graph
```
Task 1.1 ──> Task 1.2 ──> Task 1.3
                              |
Task 2.1 ──> Task 2.2 ──────>+──> Task 3.1
```

### Risk Register
| Risk | Probability | Impact | Mitigation | Affected Tasks |
|------|------------|--------|------------|----------------|
| <risk> | HIGH/MED/LOW | HIGH/MED/LOW | <mitigation strategy> | Task X.Y |

### Total Estimate
- **Tasks:** <count>
- **Estimated effort:** <total range, e.g., "2-3 weeks">
- **Critical path:** Task 1.1 > Task 1.3 > Task 3.1 (<duration>)
```
