# Agent Coordination Patterns

This document defines reusable patterns for coordinating the 2 personas on the intently-core extraction library.

---

## 1. Parallel Review

**When to use:** A PR or changeset needs review from both architecture and security perspectives.

**Participants:**

| Reviewer | Reviews For |
|----------|------------|
| Kael | Rust quality, performance, IR correctness, architecture, module boundaries |
| Tomás | Security, dependency audit, input validation, error message safety |

**Coordination Flow:**
1. Launch review skills in parallel — each reviewer focuses on their domain
2. Collect all review outputs
3. Synthesize: identify conflicting feedback, prioritize by severity
4. Present unified review with clear action items

**Expected Artifacts:**
- Individual review reports per persona
- Unified review summary with deduplicated and prioritized findings
- Final verdict: APPROVE / REQUEST_CHANGES

---

## 2. Feature Implementation

**When to use:** Implementing a new feature (e.g., new language extractor, new graph analysis).

**Typical Flow:**
1. **Kael** — defines data model, implements core extraction logic
2. **Tomás** — security review of patterns and dependencies

**Expected Artifacts:**
- Core implementation with unit tests
- Integration tests covering the new feature
- CHANGELOG.md entry under `[Unreleased]`

---

## 3. Research Spike

**When to use:** Investigating a new technology or approach before committing to implementation.

**Coordination Flow:**
1. **Kael** investigates the technical approach
2. Documents findings: capabilities, limitations, risks, performance impact
3. **Tomás** evaluates security implications
4. Decision: adopt, reject, or defer with conditions

**Expected Artifacts:**
- Research document: problem statement, options explored, findings
- Benchmark results comparing options
- ADR documenting the decision (adopt/reject/defer with reasoning)

---

## 4. Release Gate

**When to use:** Preparing a release. All quality gates must pass before version tag.

**Coordination Flow:**
1. **Kael** runs benchmarks, verifies no performance regressions
2. **Tomás** runs `cargo audit`, reviews dependency changes
3. If any gate fails: ABORT, fix, restart from step 1

**Abort Criteria:**
- Any test failure
- Performance regression beyond threshold
- Known vulnerability in dependencies
- Incomplete CHANGELOG.md
