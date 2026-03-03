---
name: reviewing-security
description: Reviews security aspects of intently-core code. Checks for secrets in code and logs, input validation at boundaries, filesystem access scoping, dependency audit, unsafe code documentation, and error message safety. Use when changes touch extractors, Cargo.toml dependencies, error handling, unsafe blocks, or file system access.
---

# Security Review

## Critical rules

**ALWAYS:**
- Run `cargo audit` on every dependency change — zero known vulnerabilities policy
- Validate all file paths at public API boundaries — canonicalize and check against project root
- Document every `unsafe` block with a `// SAFETY:` comment explaining invariants
- Review new crate dependencies for: license compatibility (MIT/Apache 2.0), maintenance status, transitive deps
- Verify error messages are safe for external consumers — no internal paths, no source code snippets

**NEVER:**
- Log, print, or include secrets (API keys, tokens, passwords) in error messages or tracing spans
- Follow symlinks during file walking without explicit validation — path traversal vector
- Accept a new `unsafe` block without a documented safe alternative that was considered and rejected
- Add a dependency with GPL license to this MIT/Apache 2.0 library — license incompatibility
- Expose internal file system structure (absolute paths, directory layout) in user-facing errors

## intently-core specific concerns

- **Extractors detect PII/secrets** in analyzed code — the library itself must not leak data in its own errors or logs
- **Filesystem access** must respect project root boundaries (no path traversal via malicious file paths)
- **Error messages** must not expose internal paths, source code snippets, or system details
- **Dependencies** are audited with `cargo audit` — new crates need license and supply chain review

## Checklist

- [ ] No secrets (API keys, tokens, passwords) in code, logs, or error messages
- [ ] All external inputs validated at system boundaries (public API methods, file paths)
- [ ] File operations scoped to project root, no symlink following without validation
- [ ] `cargo audit` passes with no known vulnerabilities
- [ ] No new `unsafe` without `// SAFETY:` documentation and justification
- [ ] Error messages don't leak internal paths or system details

## Output format

```
## Security Review: <scope>

### Findings
- [PASS/FAIL] <category>: <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
Severity: CRITICAL / HIGH / MEDIUM / LOW / NONE
```
