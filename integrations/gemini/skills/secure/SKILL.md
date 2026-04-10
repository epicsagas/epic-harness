---
name: secure
description: "Security review. Use when auth, database, API, infrastructure, or secrets-related code is modified. OWASP Top 10 + injection + access control."
---

# Secure — Security Review

## When to Trigger
- Authentication or authorization code changed
- Database queries written or modified
- API endpoints added or changed
- Environment variables or secrets referenced
- File upload, user input parsing, or serialization code
- Infrastructure config (Docker, K8s, CI/CD)

## Checklist

### Injection
- [ ] All user input sanitized/escaped before use in queries
- [ ] Parameterized queries used (no string concatenation in SQL)
- [ ] No `eval()`, `exec()`, or template injection vectors

### Authentication & Authorization
- [ ] Auth checks on every protected endpoint
- [ ] Tokens validated and not just decoded
- [ ] Session expiry and refresh logic correct
- [ ] No hardcoded credentials or secrets in code

### Data Exposure
- [ ] Sensitive data not logged (passwords, tokens, PII)
- [ ] API responses don't leak internal details
- [ ] Error messages don't reveal system internals
- [ ] `.env` files in `.gitignore`

### Access Control
- [ ] RBAC/permissions checked before data access
- [ ] No IDOR (Insecure Direct Object Reference) vulnerabilities
- [ ] Rate limiting on authentication endpoints

See `references/security.md` for the full OWASP Top 10 checklist.

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Security review is overkill for this" | One missed injection is a breach. Every input surface matters. | Run the checklist. It takes 2 minutes, a breach takes months. |
| "We'll harden it before production" | Security bolted on later is always incomplete. | Build it secure now. Retrofitting misses edge cases. |
| "It's an internal API, no one will abuse it" | Lateral movement attacks start from internal APIs. | Internal ≠ trusted. Validate and authorize every request. |
| "I'll just disable CORS for development" | Dev shortcuts leak into production. | Use a proper CORS allow-list from day one. |

## Evidence Required

Before claiming security review is complete, show ALL applicable:

- [ ] Checklist above completed: each item marked ✓ or N/A with reason
- [ ] No secrets in code: `grep -r "sk-\|password\s*=" --include="*.{ts,js,py}" .` shows clean
- [ ] Parameterized queries used: show the query code (no string concat)
- [ ] Auth checks present on new endpoints: show the middleware/guard
- [ ] `.env` in `.gitignore`: confirmed

**A blank checklist is not a review. Each item needs ✓ or N/A.**

## Red Flags
- Storing secrets in code or config files committed to git
- Using `HTTP` instead of `HTTPS` for sensitive data
- `eval()` or string-concatenated SQL anywhere
