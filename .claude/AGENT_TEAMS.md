# Agent Coordination Patterns

This document defines reusable patterns for coordinating the 2 personas on the intently-core extraction library.

---

## Task Lifecycle

All coordination patterns use the same task lifecycle:

```
head-tech creates tasks (TaskCreate)
    ↓
head-tech assigns to persona (TaskUpdate: owner)
    ↓
Persona claims (TaskUpdate: status → in_progress)
    ↓
Persona implements → tests → verifies
    ↓
Persona marks done (TaskUpdate: status → completed)
    ↓
head-tech runs final verification (cargo test + clippy + fmt)
    ↓
head-tech reports to user
```

**Rules:**
- Only head-tech creates and assigns tasks (personas may create sub-tasks if they discover additional work)
- A persona MUST mark a task in_progress before starting work
- A persona MUST NOT mark a task completed if tests fail or clippy has warnings
- head-tech MUST run final verification before reporting success to the user

---

## 1. Parallel Review

**When to use:** A PR or changeset needs review from both architecture and security perspectives.

**Participants:**

| Reviewer | Reviews For |
|----------|------------|
| Kael | Rust quality, performance, IR correctness, architecture, module boundaries |
| Tomás | Security, dependency audit, input validation, error message safety |

**Coordination Flow:**
1. head-tech creates 2 review tasks (one per persona)
2. Launch review skills in parallel — each reviewer focuses on their domain
3. Collect all review outputs
4. Synthesize: identify conflicting feedback, prioritize by severity
5. Present unified review with clear action items

**Expected Artifacts:**
- Individual review reports per persona
- Unified review summary with deduplicated and prioritized findings
- Final verdict: APPROVE / REQUEST_CHANGES

---

## 2. Feature Implementation

**When to use:** Implementing a new feature (e.g., new graph analysis pass, new extraction capability, new model field).

**Coordination Flow:**
1. **head-tech** decomposes the feature into tasks (use `/roadmap-exec` for complex features)
2. **head-tech** assigns implementation tasks to Kael, security tasks to Tomás
3. **Kael** implements foundation first: types/traits → core logic → tests
4. **Tomás** implements security-scoped work: input validation, error safety, dependency evaluation
5. Each persona runs their own quality gates before marking tasks complete
6. **head-tech** runs cross-review: assigns Kael to review Tomás's code, Tomás to review Kael's code
7. **head-tech** runs final verification (`cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`)
8. **head-tech** verifies CHANGELOG.md is updated and reports completion to user

**Task ordering:**
- Types and data model tasks first (they unblock everything else)
- Implementation tasks next (may run in parallel if independent)
- Test tasks alongside implementation (not after — write tests as you implement)
- Documentation and CHANGELOG last

**Expected Artifacts:**
- Core implementation with unit tests
- Integration tests covering the new feature
- CHANGELOG.md entry under `[Unreleased]`
- All quality gates passing

---

## 3. Research Spike

**When to use:** Investigating a new technology or approach before committing to implementation.

**Coordination Flow:**
1. **head-tech** creates a research task for Kael with specific questions to answer
2. **Kael** investigates the technical approach
3. Documents findings: capabilities, limitations, risks, performance impact
4. **Tomás** evaluates security implications (dependency audit, supply chain, unsafe usage)
5. Decision: adopt, reject, or defer with conditions

**Expected Artifacts:**
- Research document: problem statement, options explored, findings
- Benchmark results comparing options (if applicable)
- ADR documenting the decision (adopt/reject/defer with reasoning)

---

## 4. Release Gate

**When to use:** Preparing a release. All quality gates must pass before version tag.

**Coordination Flow:**
1. **head-tech** invokes `/release-checklist` skill to start the process
2. **Kael** runs benchmarks, verifies no performance regressions
3. **Tomás** runs `cargo audit`, reviews dependency changes since last release
4. **head-tech** runs quality gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo tarpaulin`
5. If any gate fails: ABORT, fix, restart from step 1

**Abort Criteria:**
- Any test failure
- Performance regression beyond threshold
- Known vulnerability in dependencies
- Incomplete CHANGELOG.md
- Coverage below 80%

---

## 5. Bug Fix

**When to use:** Fixing a reported bug or a failing test.

**Coordination Flow:**
1. **head-tech** creates a bug fix task with: reproduction steps, expected vs actual behavior, affected files
2. **Kael** claims the task and follows this sequence:
   a. **Reproduce** — Write a failing test that demonstrates the bug (regression test)
   b. **Diagnose** — Read the code path to find the root cause
   c. **Fix** — Make the minimal change to fix the root cause (not symptoms)
   d. **Verify** — Run the regression test (must pass), run full test suite (no regressions)
3. **Tomás** reviews the fix if it touches: error handling, input validation, extractors, or dependencies
4. **head-tech** runs final verification and verifies CHANGELOG.md has a `### Fixed` entry

**Key rule:** The regression test MUST be written BEFORE the fix. If the test passes before your fix, you're testing the wrong thing.

**Expected Artifacts:**
- Regression test (committed separately or with the fix)
- Minimal fix targeting root cause
- CHANGELOG.md `### Fixed` entry
- All quality gates passing

---

## 6. New Language Extractor

**When to use:** Adding support for a new programming language or a new framework for an existing language.

### New Language (full pipeline)

**Coordination Flow:**
1. **head-tech** creates 8 ordered tasks (see below)
2. **Kael** implements all tasks sequentially (each builds on the previous)
3. **Tomás** reviews extractor patterns for false positive/negative rates and dependency safety

**Tasks (in order, each blocks the next):**

| # | Task | Files |
|---|------|-------|
| 1 | Add tree-sitter grammar dependency | `Cargo.toml` |
| 2 | Add language variant + detection | `src/parser/mod.rs` |
| 3 | Create language extractor | `src/model/extractors/<lang>.rs`, `src/model/extractors/mod.rs` |
| 4 | Implement LanguageBehavior | `src/model/extractors/language_behavior.rs` |
| 5 | Add symbol extraction queries | `src/model/extractors/symbols.rs` |
| 6 | Add call graph patterns | `src/model/extractors/call_graph.rs` |
| 7 | Create fixtures + integration tests | `tests/fixtures/<lang>_*/`, `tests/full_extraction.rs` |
| 8 | Update documentation | `CLAUDE.md` (language table + architecture tree), `CHANGELOG.md` |

### New Framework (for existing language)

**Tasks (in order):**

| # | Task | Files |
|---|------|-------|
| 1 | Add detection logic | `src/model/extractors/<lang>.rs` |
| 2 | Create fixture exercising patterns | `tests/fixtures/<framework>_*/` |
| 3 | Add integration test assertions | `tests/full_extraction.rs` |
| 4 | Update documentation | `CHANGELOG.md` |

**Expected Artifacts:**
- Working extractor with framework detection
- Fixture project with representative code patterns
- Integration tests asserting extraction correctness
- CHANGELOG.md `### Added` entry
- CLAUDE.md updated with new language/framework

---

## 7. Refactoring

**When to use:** Restructuring code without changing external behavior.

**Coordination Flow:**
1. **head-tech** creates the refactoring task with: what changes, why, and scope boundaries
2. **Kael** claims the task and follows this sequence:
   a. **Baseline** — Run `cargo test` and record all passing tests (this is your safety net)
   b. **Plan** — Identify all files that will change and all tests that cover them
   c. **Execute** — Make changes incrementally, running tests after each logical step
   d. **Verify** — ALL baseline tests must still pass. No regressions.
   e. **Verify public API** — If any `pub` signatures changed, this is a breaking change (semver MAJOR)
3. **Tomás** reviews if the refactoring touches: error types, input validation, or security patterns
4. **head-tech** runs final verification

**Key rules:**
- Refactoring does NOT change behavior — if tests break, either the refactoring introduced a bug or the test was testing implementation (fix the test only if it tested implementation, never to make a behavioral regression pass)
- If the refactoring is large, split into multiple tasks that each leave the codebase in a working state
- CHANGELOG.md: refactoring that doesn't affect public API gets NO entry (internal change, invisible to consumers)

**Expected Artifacts:**
- Refactored code with all baseline tests passing
- Any new tests written to cover gaps discovered during refactoring
- No CHANGELOG entry (unless public API changed)
