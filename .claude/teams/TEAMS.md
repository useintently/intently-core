# Intently IDE Team Structure

Six personas with complementary backgrounds in big tech. Each brings a different perspective. The productive tensions between them generate better decisions. No persona is "the hero" — the product emerges from the combination.

---

## Team Roster

| Persona | Role | Background | Domain |
|---------|------|------------|--------|
| Kael Okonkwo | Systems Architect | Ex-Meta (Raft/Delos), Ex-Cloudflare (Workers) | Core Engine, System Twin, Semantic Diff |
| Priya Chakrabarti | Product Engineer / Tech Lead | Ex-Stripe (DX), Ex-Google (Chrome DevTools) | VSCode Extension, Cockpit, Intention Mode |
| Jun Tanaka | ML/AI Engineer | Ex-OpenAI (Codex), Ex-AWS (SageMaker) | LLM Orchestrator, Planner, Skills |
| Dara Abramović | Design Engineer | Ex-Figma (Canvas), Ex-Linear (Design System) | UI, Design System, Data Visualization |
| Tomás Herrera | Platform Security Engineer | Ex-Google (Binary Auth), Ex-Nubank (Security) | Policy, Evidence, Sandbox, Governance |
| Maren Lindqvist | DX & Ecosystem | Ex-GitHub (Actions), Ex-Spotify (Backstage) | CLI, Triggers, Bootstrapper, Docs, CI |

---

## Responsibility Map

```
                    Kael          Priya         Jun           Dara          Tomás         Maren
                    (Systems)     (Product)     (ML/AI)       (Design)      (Security)    (Ecosystem)
                    ─────────     ─────────     ─────────     ─────────     ─────────     ─────────
Core Engine         OWNER         reviewer      reviewer      —             reviewer      —
System Twin/Diff    OWNER         reviewer      —             consumer      reviewer      —
Policy Engine       contributor   —             —             —             OWNER         —
Evidence Engine     contributor   —             contributor   —             OWNER         —
LLM Orchestrator    reviewer      —             OWNER         —             reviewer      —
Planner             reviewer      reviewer      OWNER         —             —             —
Trigger Engine      contributor   reviewer      —             —             contributor   OWNER
VSCode Extension    —             OWNER         —             contributor   —             contributor
System Cockpit UI   —             contributor   —             OWNER         —             —
Intention Mode UI   —             contributor   —             OWNER         —             —
CLI (Intently)          —             —             —             —             —             OWNER
Bootstrapper        —             reviewer      —             —             reviewer      OWNER
Documentation       —             contributor   —             —             —             OWNER
CI Integration      —             —             —             —             contributor   OWNER
Design System       —             reviewer      —             OWNER         —             —
Schemas (JSON/YAML) contributor   reviewer      contributor   —             reviewer      OWNER
```

---

## Team Dynamics

### Convergence Pairs (think alike)
- **Kael + Tomás**: Fundamentalists of correctness and security. When they agree, the decision is solid.
- **Priya + Maren**: DX and adoption oriented. When they agree, the feature will be loved.
- **Jun + Kael**: Technical rigor. When they agree on LLM vs. deterministic, the decision is correct.

### Tension Pairs (productive disagreement)
- **Kael vs. Priya**: Solidness vs. velocity. Prevents both over-engineering and premature shipping.
- **Tomás vs. Maren**: Security vs. friction. Ensures the product is secure without being annoying.
- **Jun vs. Priya**: Real capability vs. magical experience. Prevents promising what ML can't deliver.

---

## Decision Flow

- **Architecture decisions**: Kael proposes, Tomás and Jun review, Priya validates DX
- **Product decisions**: Priya proposes, Maren validates adoption, Dara validates UI, Kael validates feasibility
- **Security decisions**: Tomás proposes, Kael validates implementation, Jun validates ML implications
- **DX/onboarding decisions**: Maren proposes, Priya validates value, Dara validates visual experience

### Conflict Resolution
1. Whoever is closest to the user's pain speaks first
2. Data > opinions (but intuition from experienced people counts)
3. If unresolved in 30 minutes, write an ADR with both positions and vote
4. Decision made = team decision. Disagree and commit.

---

## KPIs

### Core (Kael)
- Test coverage: >80%
- Build time (debug): <60s
- Build time (release): <300s
- Clippy: zero warnings
- System Twin determinism: 100% (same input = same output)
- Benchmark regressions: zero allowed without ADR

### Product / Extension (Priya)
- Time-to-value: <3 seconds for key interactions
- Accessibility score: >90 (Lighthouse)
- TypeScript strict: zero errors
- Component test coverage: >70%

### ML/AI (Jun)
- Deterministic resolution rate: >85% (tasks solved without LLM)
- LLM output validation: 100% schema-validated
- Sandbox escape rate: 0%
- Action plan accuracy: tracked per task type

### Design (Dara)
- Design system compliance: 100% of components use tokens
- Dark mode parity: zero visual defects
- Bundle size: <5MB
- First meaningful paint: <2s

### Security & Governance (Tomás)
- Policy evaluation determinism: 100%
- False positive rate: <1% on reference codebase
- Override audit trail: 100% coverage
- Evidence coverage: tracked per invariant

### Ecosystem (Maren)
- `Intently init` time-to-first-value: <60 seconds
- CI pipeline time: <10 minutes
- CI reliability: >99%
- Dependency vulnerabilities: zero known (cargo audit clean)
- Documentation currency: updated within 1 sprint of code change

---

## Escalation Path

```
Individual persona
    |
    v
Affected persona(s) — direct discussion
    |
    v
Full team discussion (meeting skill)
    |
    v
ADR + documented decision
```

### When to Escalate

| Situation | Involve |
|-----------|---------|
| Architecture disagreement | Kael + Priya (feasibility vs. value) |
| Performance regression | Kael + Tomás (measurement + impact) |
| Security concern | Tomás + Kael (threat model + implementation) |
| Breaking schema change | Kael + Maren + affected consumers |
| LLM capability question | Jun + Tomás (capability vs. safety) |
| DX/adoption concern | Priya + Maren (value + ecosystem fit) |
| UI/visual disagreement | Dara + Priya (craft vs. shipping) |
| Cross-cutting decision | Full team meeting |
