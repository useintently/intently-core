# Technical Meeting

Multi-perspective technical discussion and debate with structured output.

## Trigger

Activate for architectural discussions, trade-off evaluations, or decisions needing multiple viewpoints.

Keywords: "meeting", "debate", "discussion", "consensus", "trade-off", "decision", "RFC"

## What This Skill Does

1. **Receive Topic** — Clarify the question or decision to be made
   - What is the specific technical question?
   - What are the constraints and requirements?
   - What is the impact scope (one crate, cross-cutting, user-facing)?

2. **Select Participants** — Choose relevant personas (3-5)
   - Map the topic domain to affected personas from the routing table
   - Each persona brings domain-specific concerns and their unique background
   - Every persona challenges vague claims from their domain perspective

3. **Conduct Structured Debate** — Present arguments with evidence
   - Each persona states their position with codebase evidence
   - Cite specific files, patterns, or precedents from the repo
   - Identify trade-offs explicitly (what we gain vs what we lose)
   - Surface risks and unknowns

4. **Synthesize** — Produce actionable conclusions
   - Identify points of agreement
   - Highlight unresolved disagreements with clear options
   - Recommend a path forward with reasoning
   - Define follow-up actions and owners

## Meeting Structure

```
1. TOPIC STATEMENT (2 min)
   - What are we deciding?
   - What are the constraints?

2. PERSPECTIVES (per persona, ~3 min each)
   - Position + evidence from codebase
   - Concerns and risks from their domain

3. DEBATE (structured)
   - Points of agreement
   - Points of disagreement + options

4. CONCLUSION
   - Recommendation with reasoning
   - Follow-up actions + owners
```

## Participant Selection Guide

| Topic Domain | Required Personas | Optional |
|-------------|-------------------|----------|
| Schema changes | Kael (types/IR), Maren (ergonomics), affected consumers | Priya, Jun |
| New engine feature | Kael (architecture), Tomás (security), Jun (if ML) | Priya (DX) |
| UI/UX decisions | Dara (design), Priya (product) | Kael (data contracts) |
| Safety/compliance | Tomás (security), Jun (LLM safety), Kael (implementation) | Priya (DX impact) |
| Performance | Kael (benchmarks), Tomás (evidence) | Maren (CI impact) |
| DX/onboarding | Maren (ecosystem), Priya (product), Dara (visual) | Tomás (security) |
| Release planning | Maren (CI/artifacts), Kael (core), Tomás (quality gates) | all |
| Architecture decisions | Kael (proposal), all others (review from their domain) | — |

### Persona Perspectives

| Persona | Demands | Challenges |
|---------|---------|------------|
| Kael Okonkwo | Benchmarks, determinism proofs, property-based tests | "Show me the numbers" |
| Priya Chakrabarti | User value evidence, DX metrics, adoption data | "How does the dev benefit in 3 seconds?" |
| Jun Tanaka | ML evaluation metrics, confidence intervals, failure modes | "Can this be done deterministically instead?" |
| Dara Abramović | Visual hierarchy, design system compliance, accessibility | "Show me it works in dark mode" |
| Tomás Herrera | Threat model, audit trail, sandbox boundaries | "What if the agent does X when it shouldn't?" |
| Maren Lindqvist | Time-to-value, onboarding friction, ecosystem fit | "Can a new dev use this in 60 seconds?" |

## Output Format

```
## Meeting Minutes: <topic>
Date: <date>

### Topic
<clear statement of the question>

### Participants
- <persona_name>: <perspective summary>

### Discussion

#### <persona_name> Position
<position with codebase evidence>

#### <persona_name> Position
<position with codebase evidence>

### Points of Agreement
- <agreed item>

### Points of Disagreement
- <disagreement>: Option A vs Option B

### Recommendation
<recommended path with reasoning>

### Follow-Up Actions
- [ ] <action> — Owner: <persona_name>
```
