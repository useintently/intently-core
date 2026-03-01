# Agent Coordination Patterns

This document defines reusable patterns for coordinating the 6 personas on the Intently IDE project. Each pattern describes when to use it, which personas participate, and what artifacts are produced.

---

## 1. Parallel Review

**When to use:** A PR or changeset touches multiple domains and needs simultaneous review from different perspectives.

**Participants (select 2-4 based on scope):**

| Reviewer | Reviews For |
|----------|------------|
| Kael | Rust quality, performance, IR correctness, architecture |
| Priya | Product value, DX, extension integration |
| Jun | LLM safety, planner logic, evaluation coverage |
| Dara | UI quality, design system compliance, accessibility |
| Tomás | Security, policy correctness, evidence coverage, sandbox |
| Maren | CLI ergonomics, schema quality, CI integration, docs |

**Coordination Flow:**
1. Identify which domains the changeset touches
2. Launch review skills in parallel — each reviewer focuses on their domain
3. Collect all review outputs
4. Synthesize: identify conflicting feedback, prioritize by severity
5. Present unified review with clear action items

**Expected Artifacts:**
- Individual review reports per persona
- Unified review summary with deduplicated and prioritized findings
- Final verdict: APPROVE / REQUEST_CHANGES (unanimous required for APPROVE)

---

## 2. Feature End-to-End

**When to use:** Implementing a new feature that spans from data model to user-facing UI.

**Typical Flow:**
1. **Kael** — defines data model, schema, core engine implementation
2. **Jun** — adds planner/orchestrator support if AI is involved
3. **Dara** — builds the UI components
4. **Priya** — wires extension integration and validates DX
5. **Tomás** — adds policy/evidence coverage and security review
6. **Maren** — updates CLI commands, docs, and CI integration

**Handoff Protocol:**
- Kael -> Jun/Dara: Rust types + JSON schema + Tauri command signatures
- Jun -> Dara: Structured output format for visualization
- Dara -> Priya: Component API + interaction patterns
- Kael -> Tomás: Testable interfaces + expected behaviors
- Maren: Updates docs and CLI in parallel with implementation

**Expected Artifacts:**
- Schema definition (JSON schema or Rust types)
- Core implementation with unit tests
- UI components with component tests
- Evidence suite covering the full feature
- CLI command support
- CHANGELOG.md entry under `[Unreleased]`

---

## 3. Schema Evolution

**When to use:** A schema change that requires coordinated updates across the stack.

**Coordination Flow:**
1. **Maren** proposes schema change with migration path and ergonomic analysis
2. **Kael** reviews structural impact and adapts parsers/types
3. **Tomás** validates security implications and evidence schema updates
4. **Dara** updates UI to handle new/changed fields
5. **Priya** validates DX impact
6. **Jun** updates planner output if action_plan schema changes

**Breaking Change Protocol:**
- Bump schema major version
- Provide migration utility or documentation
- Deprecation period: one minor release before removal
- CHANGELOG.md entry under `Changed` (or `Removed` if removing fields)
- ADR documenting the rationale

---

## 4. Release Gate

**When to use:** Preparing a release. All quality gates must pass before version tag.

**Coordination Flow:**
1. **Kael** runs benchmarks, verifies no performance regressions
2. **Tomás** runs full test suite, evidence collection, security audit
3. **Maren** validates CI pipeline, dependency audit, builds release artifacts
4. **Priya** reviews CHANGELOG.md completeness, verifies documentation is current
5. **Dara** verifies UI consistency and dark mode across all views
6. If any gate fails: ABORT, fix, restart from step 1

**Abort Criteria:**
- Any test failure
- Performance regression beyond threshold
- Known vulnerability in dependencies
- Incomplete CHANGELOG.md
- UI inconsistency in critical views

---

## 5. Research Spike

**When to use:** Investigating a new technology or approach before committing to implementation.

**Coordination Flow:**
1. **Primary researcher** (Kael for systems, Jun for ML, Maren for ecosystem) investigates
2. Researcher documents findings: capabilities, limitations, risks, costs
3. **Kael** evaluates architectural impact
4. **Priya** evaluates product/DX alignment
5. **Tomás** evaluates security implications
6. Decision: adopt, reject, or defer with conditions

**Expected Artifacts:**
- Research document: problem statement, options explored, findings
- Proof-of-concept code (in a branch, not merged)
- Benchmark results comparing options
- ADR documenting the decision (adopt/reject/defer with reasoning)

**Time-Box:**
- Research spikes are time-boxed (typically 2-5 days)
- If no conclusion within time-box, document findings and defer
- Never extend a spike without explicit decision to do so

---

## 6. Architecture Decision

**When to use:** A decision with long-term impact that affects multiple domains.

**Coordination Flow:**
1. Proposer writes RFC with context, options, and recommendation
2. Each affected persona reviews from their domain perspective:
   - **Kael**: implementation feasibility, performance, correctness
   - **Priya**: DX impact, user value
   - **Jun**: ML implications, capability assessment
   - **Dara**: visualization needs, UI complexity
   - **Tomás**: security, governance, compliance
   - **Maren**: ecosystem fit, adoption impact, documentation burden
3. Structured debate: agreements, disagreements, tradeoffs
4. Decision recorded as ADR

**Conflict Resolution:**
1. Whoever is closest to the user's pain speaks first
2. Data > opinions (but experienced intuition counts)
3. If unresolved in 30 minutes, write ADR with both positions and vote
4. Decision made = team decision. Disagree and commit.
