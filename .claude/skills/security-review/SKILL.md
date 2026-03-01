# Security Review

Security-focused review for the intently-core extraction library.

## Trigger

Activate when PRs or changes touch:
- Credential or secret pattern detection in extractors
- Dependency updates (`Cargo.toml`, `Cargo.lock`)
- Error handling or error message formatting
- `unsafe` code blocks
- File system access patterns

Keywords: "security review", "review security", "audit", "vulnerability", "secrets"

## What This Skill Does

1. **Secrets in Code/Logs** — Scan for leaked credentials
   - No API keys, tokens, passwords, or private keys in source code
   - No secrets in log output (structured logging masks sensitive fields)
   - `.env` files are in `.gitignore`
   - No secrets in error messages

2. **Input Validation** — Verify boundary validation
   - All external inputs (file paths, source code) are validated at entry
   - Path inputs are canonicalized and checked
   - No path traversal vectors in file walking
   - Deserialization uses typed schemas

3. **Filesystem Access** — Validate scope restrictions
   - File operations respect project root boundaries
   - No symlink following without validation
   - Temporary files use secure creation (`tempfile` crate)
   - Cleanup of temporary files is guaranteed

4. **Dependency Audit** — Check third-party risk
   - `cargo audit` passes with no known vulnerabilities
   - No `unsafe` in dependencies without review
   - License compatibility verified
   - Transitive dependency tree is reasonable

5. **Unsafe Code** — Review safety guarantees
   - Every `unsafe` block has a `// SAFETY:` comment
   - Invariants are documented and tested
   - Prefer safe alternatives where possible

## What to Check

- [ ] No secrets in code, logs, or error messages
- [ ] Input validation at all system boundaries
- [ ] Filesystem access is scoped and safe
- [ ] `cargo audit` passes
- [ ] No new `unsafe` without SAFETY documentation
- [ ] Error messages don't leak internal paths or details

## Output Format

```
## Security Review: <scope>

### Secrets Scan
- [PASS/FAIL] <detail>

### Input Validation
- [PASS/FAIL] <detail>

### Filesystem Access
- [PASS/FAIL] <detail>

### Dependency Audit
- [PASS/FAIL] <detail>

### Unsafe Code
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
Severity: [CRITICAL/HIGH/MEDIUM/LOW/NONE]
```
