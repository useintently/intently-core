# Claude Code Hooks for Intently IDE

This directory contains hook scripts that enforce project conventions, protect critical files, and provide advisory guidance during development with Claude Code.

## How Hooks Work

Hooks receive tool call information as JSON on stdin with the structure:

```json
{
  "tool_name": "Read|Edit|Write",
  "tool_input": {
    "file_path": "...",
    ...
  }
}
```

Exit codes determine behavior:

- **Exit 0 (no output):** Allow the operation silently.
- **Exit 0 (with output):** Allow with an advisory message printed to the agent.
- **Exit 2 (with output):** Block the operation and print the reason.

## Hook Summary

| Hook | File | Trigger | Behavior |
|------|------|---------|----------|
| Schema Protection | `protect-schemas.py` | Edit/Write to `schemas/*.schema.json` | **Blocks.** Schemas are governed artifacts. Requires an ADR in `docs/adr/` before modification. |
| Generated Files Protection | `protect-generated-files.py` | Edit/Write to `target/`, `dist/`, `node_modules/`, `*_bindings.ts`, `bindings.ts`, `*.generated.*` | **Blocks.** Generated and build artifacts must not be edited manually. |
| Crate Boundary Guard | `crate-boundary-guard.py` | Edit/Write in `crates/Intently_core/` that references `Intently_cli` | **Blocks.** Enforces the dependency direction: `Intently_cli -> Intently_core`, never the reverse. |
| Read Protection | `read-protection.py` | Read with `offset`/`limit` on critical files (schemas, rules, agents, Cargo.toml, tauri.conf.json, intent.yaml) | **Blocks.** Critical configuration files must be read in full to avoid partial context. |
| Intent Guard | `intent-guard.py` | Edit/Write to any `intent.yaml` file | **Advisory.** Reminds the agent to maintain valid intent.yaml schema structure. |
| Architecture First | `architecture-first.py` | Write (new file) in core architecture directories (`ir/`, `diff/`, `policy/`, `planner/`, `evidence/`) | **Advisory.** Suggests documenting design decisions in `docs/adr/` when adding new modules to core architecture. |
| Task Completed | `task-completed.sh` | Post-task completion | **Advisory.** Prints a Definition of Done checklist: tests, clippy, formatting, changelog, schema validation. |
| Teammate Idle | `teammate-idle.sh` | Teammate goes idle | **Advisory.** Reminds to check the task list for pending unblocked work. |

## Hook Details

### protect-schemas.py (Blocking)

Prevents direct modification of JSON Schema files in `schemas/`. These files define the contract for Intently's artifact system and changes must go through an Architecture Decision Record process.

### protect-generated-files.py (Blocking)

Prevents editing build outputs and auto-generated code, including:
- Rust build artifacts (`target/`)
- Frontend build output (`dist/`)
- Node dependencies (`node_modules/`)
- Tauri TypeScript bindings (`*_bindings.ts`, `bindings.ts`)
- Any file matching the `*.generated.*` pattern

### crate-boundary-guard.py (Blocking)

Enforces the crate dependency graph in the Rust workspace:
- `Intently_core` is the foundation and must have zero reverse dependencies.
- `Intently_cli` depends on `Intently_core`.
- `apps/desktop/src-tauri/` can depend on both.

Any `use Intently_cli` or `Intently_cli::` reference inside `crates/Intently_core/` is blocked.

### read-protection.py (Blocking)

Forces complete reads of critical configuration files. Partial reads (with `offset` or `limit`) risk the agent working with incomplete context on files where full understanding is essential.

Protected paths:
- `schemas/*.schema.json`
- `.claude/agents/*.md`
- `.claude/rules/*.md`
- `Cargo.toml` (root and crate-level)
- `apps/desktop/src-tauri/tauri.conf.json`
- `intent.yaml`

### intent-guard.py (Advisory)

Prints a reminder whenever an `intent.yaml` file is modified. Intent files are central to Intently's governance model and must maintain valid structure.

### architecture-first.py (Advisory)

Prints a reminder when new files are created in core engine directories (`ir/`, `diff/`, `policy/`, `planner/`, `evidence/`). New modules in these areas represent architectural decisions that should be documented.

### task-completed.sh (Advisory)

Outputs a Definition of Done checklist after task completion to ensure quality gates are met before moving on.

### teammate-idle.sh (Advisory)

Notifies about pending tasks when a teammate session goes idle, promoting continuous flow of work.
