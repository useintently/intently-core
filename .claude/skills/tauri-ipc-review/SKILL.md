# Tauri IPC Review

Review Tauri IPC commands and desktop integration in Intently IDE.

## Trigger

Activate when PRs or changes touch:
- `apps/desktop/src-tauri/` (any file)
- Tauri command definitions (`#[tauri::command]`)
- Tauri permissions, capabilities, or allowlists
- IPC event definitions between frontend and backend

Keywords: "tauri review", "IPC review", "review tauri", "desktop review", "tauri commands"

## What This Skill Does

1. **Typed Commands** — Verify IPC commands are properly typed
   - All `#[tauri::command]` functions have typed parameters and return types
   - Serialization/deserialization uses serde with explicit types
   - No `serde_json::Value` as catch-all parameter type
   - Error types implement `serde::Serialize` for frontend consumption

2. **Input Security** — Ensure no raw user input reaches shell or filesystem
   - Path parameters are validated and canonicalized
   - No `std::process::Command` with unsanitized arguments
   - No string interpolation into shell commands
   - SQL/query parameters are parameterized, not concatenated

3. **Filesystem Allowlist** — Validate filesystem access is scoped
   - Tauri `fs` scope is configured in `tauri.conf.json` or capabilities
   - Commands only access paths within the allowed scope
   - No path traversal vulnerabilities (`../` escapes)
   - Temporary files use designated temp directories

4. **Permissions** — Check Tauri v2 permission model
   - Capabilities are minimal (least privilege)
   - Each plugin permission is justified
   - No blanket `allow-all` permissions
   - Permissions are documented in the capability file

5. **Event Structure** — Verify frontend-backend events
   - Events have typed payloads (not arbitrary JSON)
   - Event names follow a consistent convention
   - No sensitive data in event payloads
   - Error events include actionable information

## What to Check

- [ ] All commands have explicit parameter and return types
- [ ] No unsanitized input reaches shell or filesystem operations
- [ ] Filesystem scope is configured and enforced
- [ ] Tauri capabilities follow least-privilege principle
- [ ] Events have typed, documented payloads
- [ ] Error types serialize cleanly for the frontend
- [ ] Unit tests cover command validation and error paths

## Output Format

```
## Tauri IPC Review: <file_path>

### Typed Commands
- [PASS/FAIL] <detail>

### Input Security
- [PASS/FAIL] <detail>

### Filesystem Scope
- [PASS/FAIL] <detail>

### Permissions
- [PASS/FAIL] <detail>

### Event Structure
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
