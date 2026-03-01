# Security Review

Security-focused review for the Intently IDE project.

## Trigger

Activate when PRs or changes touch:
- Authentication, authorization, or credential handling
- Tauri IPC commands or permissions
- Filesystem access or shell command execution
- LLM sandbox or Task Protocol constraints
- Dependency updates (`Cargo.lock`, `package-lock.json`)

Keywords: "security review", "review security", "audit", "vulnerability", "secrets"

## What This Skill Does

1. **Secrets in Code/Logs** — Scan for leaked credentials
   - No API keys, tokens, passwords, or private keys in source code
   - No secrets in log output (structured logging masks sensitive fields)
   - `.env` files are in `.gitignore`
   - No secrets in error messages returned to users

2. **Input Validation** — Verify boundary validation
   - All external inputs (IPC, CLI, file reads) are validated at entry
   - Path inputs are canonicalized and checked against allowlist
   - No SQL injection, command injection, or path traversal vectors
   - Deserialization uses typed schemas, not arbitrary JSON

3. **IPC Security (Tauri)** — Check desktop-specific attack surface
   - Tauri capabilities follow least-privilege
   - No `dangerous-allow-*` permissions without justification
   - Frontend cannot invoke arbitrary shell commands
   - Webview CSP is configured to prevent XSS

4. **Filesystem Access** — Validate scope restrictions
   - File operations are restricted to workspace and config directories
   - No symlink following without validation
   - Temporary files use secure creation (`tempfile` crate or equivalent)
   - Cleanup of temporary files is guaranteed

5. **LLM Sandbox** — Review AI safety boundaries
   - LLM-generated code cannot escape sandbox
   - No credential access from sandbox context
   - Output validation before applying LLM-generated changes
   - Budget/limit enforcement on LLM operations

6. **Dependency Audit** — Check third-party risk
   - `cargo audit` passes with no known vulnerabilities
   - `npm audit` passes for frontend dependencies
   - No `unsafe` in dependencies without review
   - License compatibility verified

## What to Check

- [ ] No secrets in code, logs, or error messages
- [ ] Input validation at all system boundaries
- [ ] Tauri capabilities are least-privilege
- [ ] Filesystem access is scoped and safe
- [ ] LLM sandbox constraints are enforced
- [ ] `cargo audit` and `npm audit` pass
- [ ] No new `unsafe` without SAFETY documentation

## Output Format

```
## Security Review: <scope>

### Secrets Scan
- [PASS/FAIL] <detail>

### Input Validation
- [PASS/FAIL] <detail>

### IPC Security
- [PASS/FAIL] <detail>

### Filesystem Access
- [PASS/FAIL] <detail>

### LLM Sandbox
- [PASS/FAIL] <detail>

### Dependency Audit
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
Severity: [CRITICAL/HIGH/MEDIUM/LOW/NONE]
```
