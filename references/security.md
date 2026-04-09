# Security Checklist

## OWASP Top 10 (2021)

### A01: Broken Access Control
- [ ] Every endpoint has authorization checks
- [ ] No IDOR — object access verified against user permissions
- [ ] CORS configured restrictively (not `*`)
- [ ] Directory listing disabled
- [ ] JWT/session validated on every request

### A02: Cryptographic Failures
- [ ] Sensitive data encrypted at rest and in transit
- [ ] No deprecated algorithms (MD5, SHA1 for passwords)
- [ ] Secrets in environment variables, not code
- [ ] HTTPS enforced for all connections

### A03: Injection
- [ ] Parameterized queries for all SQL
- [ ] User input escaped in HTML output (XSS)
- [ ] No `eval()`, `exec()`, or `Function()` with user input
- [ ] Command injection prevented (no shell interpolation)

### A04: Insecure Design
- [ ] Rate limiting on sensitive endpoints (login, password reset)
- [ ] Account lockout after N failed attempts
- [ ] Input validation on both client and server

### A05: Security Misconfiguration
- [ ] Debug mode off in production
- [ ] Default credentials changed
- [ ] Error messages don't expose stack traces
- [ ] Unnecessary features/endpoints disabled

### A06: Vulnerable Components
- [ ] Dependencies up to date (`npm audit`, `pip-audit`)
- [ ] No known vulnerabilities in transitive deps
- [ ] Lock files committed (package-lock.json, poetry.lock)

### A07: Authentication Failures
- [ ] Passwords hashed with bcrypt/scrypt/argon2
- [ ] Session tokens regenerated after login
- [ ] Multi-factor available for sensitive operations
- [ ] Token expiry enforced

### A08: Data Integrity Failures
- [ ] CI/CD pipeline signed or integrity-checked
- [ ] Dependencies from trusted sources only
- [ ] No unsigned deserialization of untrusted data

### A09: Logging & Monitoring
- [ ] Auth failures logged
- [ ] Access to sensitive data logged
- [ ] Logs don't contain passwords or tokens
- [ ] Alerting on anomalous patterns

### A10: Server-Side Request Forgery (SSRF)
- [ ] External URL inputs validated and restricted
- [ ] Internal network access blocked from user-supplied URLs
- [ ] DNS rebinding protections in place
