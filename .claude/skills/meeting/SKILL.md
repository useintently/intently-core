---
name: conducting-technical-meetings
description: Conducts structured technical debates between Kael (systems architect) and Tomás (security reviewer) for intently-core decisions. Produces meeting minutes with positions, evidence, trade-offs, and actionable recommendations. Use for architectural discussions, trade-off evaluations, RFCs, or decisions needing multiple viewpoints.
---

# Technical Meeting

## Critical rules

**ALWAYS:**
- Cite specific files, line numbers, and patterns from the codebase as evidence — no vague claims
- Include BOTH personas' perspectives — even if one domain seems less relevant, surface their concerns
- Identify trade-offs explicitly: what we gain AND what we lose with each option
- Produce actionable follow-up items with assigned owners — meetings without actions are wasted
- Record the decision as a concrete recommendation, not "it depends" — pick a path

**NEVER:**
- Accept a position without codebase evidence — "I think" is not enough, show the code
- Skip a persona's perspective because the topic seems outside their domain — cross-cutting risks hide there
- Leave disagreements unresolved without clear options — document Option A vs Option B with trade-offs
- Produce meeting minutes without follow-up actions and owners — every decision needs a next step
- Invent personas beyond Kael and Tomás — the intently-core team has exactly 2 members

## Participants

| Persona | Domain | Challenges with |
|---------|--------|----------------|
| Kael Okonkwo | Architecture, performance, correctness, determinism | "Show me the numbers" |
| Tomás Herrera | Security, dependency audit, input validation | "What if the agent does X when it shouldn't?" |

Both participate in every meeting. Primary/secondary depends on topic:

| Topic | Primary | Secondary |
|-------|---------|-----------|
| Schema/IR, engine features, performance, architecture | Kael | Tomás |
| Safety/compliance, dependency decisions | Tomás | Kael |

## Meeting structure

1. **Topic statement** — What are we deciding? What are the constraints?
2. **Perspectives** — Each persona states position with codebase evidence (cite specific files and patterns)
3. **Debate** — Points of agreement, points of disagreement with options
4. **Conclusion** — Recommendation with reasoning, follow-up actions with owners

## Output format

```
## Meeting Minutes: <topic>
Date: <date>

### Topic
<clear statement of the question>

### Participants
- Kael: <perspective summary>
- Tomás: <perspective summary>

### Discussion

#### Kael Position
<position with codebase evidence>

#### Tomás Position
<position with codebase evidence>

### Points of Agreement
- <agreed item>

### Points of Disagreement
- <disagreement>: Option A vs Option B

### Recommendation
<recommended path with reasoning>

### Follow-Up Actions
- [ ] <action> — Owner: <persona>
```
