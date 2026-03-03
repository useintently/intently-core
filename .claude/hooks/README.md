# Claude Code Hooks for intently-core

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
| Generated Files Protection | `protect-generated-files.py` | Edit/Write to `target/`, `dist/`, `*.generated.*` | **Blocks.** Generated and build artifacts must not be edited manually. |
| Read Protection | `read-protection.py` | Read with `offset`/`limit` on critical files (rules, agents, Cargo.toml) | **Blocks.** Critical configuration files must be read in full to avoid partial context. |
| Architecture First | `architecture-first.py` | Write or Edit in core architecture directories (`src/model/`, `src/parser/`, `src/search/`, `src/engine.rs`) | **Advisory.** Suggests documenting design decisions in `docs/adrs/`. |
| Task Completed | `task-completed.sh` | Post-task completion | **Advisory.** Prints a Definition of Done checklist: tests, clippy, formatting, changelog. |
| Teammate Idle | `teammate-idle.sh` | Teammate goes idle | **Advisory.** Reminds to check the task list for pending unblocked work. |

## Hook Details

### protect-generated-files.py (Blocking)

Prevents editing build outputs and auto-generated code, including:
- Rust build artifacts (`target/`)
- Any file matching the `*.generated.*` pattern

### read-protection.py (Blocking)

Forces complete reads of critical configuration files. Partial reads (with `offset` or `limit`) risk the agent working with incomplete context.

Protected paths:
- `.claude/agents/*.md`
- `.claude/rules/*.md`
- `Cargo.toml`

### architecture-first.py (Advisory)

Prints a reminder when files are created or edited in core engine directories (`src/model/`, `src/parser/`, `src/search/`, `src/engine.rs`). Changes in these areas may represent architectural decisions that should be documented.

### task-completed.sh (Advisory)

Outputs a Definition of Done checklist after task completion to ensure quality gates are met.

### teammate-idle.sh (Advisory)

Notifies about pending tasks when a teammate session goes idle, promoting continuous flow of work.
