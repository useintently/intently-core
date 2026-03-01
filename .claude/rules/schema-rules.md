# Schema Rules

JSON Schema governance for all Intently artifacts. Schemas are the contract between components — they define what is valid, not the code.

## Schema Location & Naming
- All schemas live in `schemas/` at the repository root
- Naming convention: `{artifact_name}.schema.json`
- Required schemas:
  - `schemas/intent.schema.json` — Intent definition
  - `schemas/system_twin.schema.json` — System Twin (IR)
  - `schemas/semantic_diff.schema.json` — Semantic Diff
  - `schemas/policy_report.schema.json` — Policy evaluation results
  - `schemas/evidence_report.schema.json` — Evidence collection results
  - `schemas/action_plan.schema.json` — Planner output

## Schema Structure
- Every schema MUST include `$schema` pointing to the JSON Schema draft version
- Every schema MUST include a `version` field in the defined object
- Use `"additionalProperties": false` on strict artifact types to catch typos
- Use `$ref` for shared definitions within and across schemas
- Shared definitions go in `schemas/common.schema.json` (reusable types like component_id, policy_id)

## Versioning & Compatibility
- Schemas follow Semantic Versioning independently from the application
- **Backward compatible changes** (minor version bump):
  - Adding new optional fields
  - Adding new enum values (if consumers handle unknown values)
  - Relaxing validation constraints
- **Breaking changes** (major version bump):
  - Removing or renaming fields
  - Changing field types
  - Adding new required fields
  - Tightening validation constraints
- Breaking schema changes MUST have an ADR documenting the rationale

## Validation Rules
- Core Engine MUST validate ALL artifacts against their schemas before processing
- Validation happens at system boundaries: file read, IPC receive, API input
- Invalid artifacts are rejected immediately with a clear error referencing the schema
- NEVER skip validation, even in development or testing
- Use `jsonschema` crate (Rust) for validation — do not hand-roll validators

## Schema-First Development
- When adding a new artifact type: write the schema FIRST
- When modifying an artifact: update the schema FIRST, then update code to match
- Rust structs that represent artifacts MUST derive `serde::Serialize` and `serde::Deserialize`
- Rust structs SHOULD match the schema field-for-field — no hidden transformations
- Generate TypeScript types from schemas for frontend consumption where applicable

## Schema Testing
- Every schema MUST have validation tests with valid and invalid examples
- Valid examples: `schemas/examples/{artifact_name}/valid/`
- Invalid examples: `schemas/examples/{artifact_name}/invalid/`
- Tests verify: valid examples pass, invalid examples fail with expected error
- Schema tests run as part of `just test` and CI pipeline

## Governance
- Schema changes require review from at least one core maintainer
- NEVER edit a schema without updating:
  1. The corresponding Rust types
  2. The validation tests (valid + invalid examples)
  3. The CHANGELOG.md entry
- Schema files are protected by the `protect-schemas.py` hook — changes trigger review
- Schema documentation: each schema should have a `description` on every field
