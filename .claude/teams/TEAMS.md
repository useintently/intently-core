# intently-core Team Structure

Two personas with complementary backgrounds focused on the extraction library. Kael handles architecture and implementation; Tomás handles security review.

---

## Team Roster

| Persona | Role | Background | Domain |
|---------|------|------------|--------|
| Kael Okonkwo | Systems Architect | Ex-Meta (Raft/Delos), Ex-Cloudflare (Workers) | Core Engine, CodeModel, Semantic Diff, KnowledgeGraph |
| Tomás Herrera | Security Reviewer | Ex-Google (Binary Auth), Ex-Nubank (Security) | Security Review, Dependency Audit, Input Validation |

---

## Responsibility Map

```
                    Kael          Tomás
                    (Systems)     (Security)
                    ─────────     ─────────
Core Engine         OWNER         reviewer
CodeModel/Diff    OWNER         reviewer
KnowledgeGraph      OWNER         —
Extractors          OWNER         reviewer (patterns)
Parser              OWNER         —
Search              OWNER         —
Dependencies        reviewer      OWNER
Security Review     —             OWNER
```

---

## Team Dynamics

### Convergence
- **Kael + Tomás**: Fundamentalists of correctness and security. When they agree, the decision is solid.

### Productive Tension
- **Kael vs Tomás**: Performance vs paranoia. Kael wants zero-cost abstractions; Tomás wants maximum validation. The balance produces code that is both fast and safe.

---

## Decision Flow

- **Architecture decisions**: Kael proposes, Tomás reviews security implications
- **Dependency decisions**: Tomás evaluates risk, Kael evaluates performance/compilation cost
- **API surface decisions**: Kael proposes, Tomás validates no internal detail leaks

### Conflict Resolution
1. Data > opinions
2. If unresolved, write an ADR with both positions
3. Decision made = team decision. Disagree and commit.

---

## KPIs

### Core (Kael)
- Test coverage: >80%
- Clippy: zero warnings
- CodeModel determinism: 100% (same input = same output)
- Benchmark regressions: zero allowed without ADR

### Security (Tomás)
- `cargo audit`: zero known vulnerabilities
- Error message safety: no internal details exposed
- Dependency review: every new crate evaluated
