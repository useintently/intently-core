# Intently Concepts

Core domain concepts of the Intently intent-governed development platform. This is the foundational vocabulary — every contributor MUST understand these concepts before writing code.

## Intent (`intent.yaml`)
- The intent declares WHAT must be true about the system — not how to implement it
- Contains: policies to enforce, invariants to maintain, evidence requirements
- Schema-validated against `schemas/intent.schema.json`
- Intent is the single source of truth for governance — if it is not in the intent, it is not enforced
- Intent is version-controlled alongside the codebase
- Changes to intent trigger full re-evaluation of the System Twin

## System Twin (IR) — `system_twin.json`
- Semantic intermediate representation of the entire codebase
- Contains: components, dependencies, contracts, data flows, module boundaries
- Built by the Core Engine through static analysis of source code
- NOT a 1:1 mapping of files — it captures behavioral semantics, not syntax
- Schema-validated against `schemas/system_twin.schema.json`
- The Twin is regenerated on every relevant code change
- All governance decisions operate on the Twin, never on raw source

## Semantic Diff — `semantic_diff.json`
- Computes the behavioral delta between two System Twin states
- Captures: added/removed/modified components, changed contracts, new dependencies, broken invariants
- This is NOT a textual diff — it understands what changed semantically
- Schema-validated against `schemas/semantic_diff.schema.json`
- The Semantic Diff is the input to Policy evaluation and Evidence collection
- A diff with zero semantic impact may still have textual changes (formatting, comments)

## Policies
- Enforceable rules that the system must satisfy at all times
- Categories: `SEC-*` (security), `REL-*` (reliability), `ARC-*` (architecture), `PERF-*` (performance)
- Policies are evaluated against the System Twin and Semantic Diff
- Output: `policy_report.json` — per-policy pass/fail with evidence references
- Schema-validated against `schemas/policy_report.schema.json`
- Policies are declarative — they state conditions, not implementations
- New policies are added to `intent.yaml`, never hardcoded in engine

## Evidence
- Executable proofs that demonstrate correctness of the system
- Evidence is REQUIRED, not optional — unverified claims are not accepted
- Types: test results, static analysis, coverage data, benchmark results, manual attestation
- Output: `evidence_report.json` — per-requirement evidence status
- Schema-validated against `schemas/evidence_report.schema.json`
- Evidence is collected automatically where possible, requested from developers where not
- Stale evidence (from a previous Twin state) is invalid and must be regenerated

## Planner — `action_plan.json`
- Generates a concrete action plan from policy and evidence reports
- Prefers deterministic patches (auto-fixable) over LLM-generated tasks
- Each action has: type (patch | task | manual), priority, affected components, rationale
- Schema-validated against `schemas/action_plan.schema.json`
- The Planner never executes — it only plans. Execution is a separate step requiring approval
- Actions are idempotent — applying the same plan twice produces the same result

## Skills
- Explicit, registered agent capabilities described in `SKILL.md` format
- A skill declares: what it can do, what inputs it needs, what outputs it produces
- If no skill is registered for an action, that action is FORBIDDEN
- Skills are the permission boundary for automated actions
- Skills are composable — complex operations chain multiple skills

## Gate
- The final pass/fail decision for a change
- Gate evaluates: all policies pass + all required evidence present and valid
- Gate is binary — there is no "partial pass"
- A failing gate blocks the change from proceeding
- Gate results are logged and auditable

## Artifact Flow
```
intent.yaml
    |
    v
[Core Engine: Source Analysis]
    |
    v
system_twin.json  -->  semantic_diff.json
    |                       |
    v                       v
policy_report.json    evidence_report.json
    |                       |
    +----------+------------+
               |
               v
        action_plan.json
               |
               v
           [Gate: pass/fail]
```

## Rules
- ALL artifacts are JSON with schemas in `schemas/`
- Schemas are the contract — code must conform to schemas, not the other way around
- Never bypass schema validation, even in development
- The Core Engine is the ONLY producer of System Twin and Semantic Diff
- The intent is the ONLY source of policy definitions
