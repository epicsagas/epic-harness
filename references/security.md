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

---

## LLM / Agent File Security

Patterns specific to agent `.md` files that are ingested by LLMs as instructions.

### Unicode Prompt Injection
Invisible Unicode codepoints can be embedded in text to inject hidden instructions into LLM context.

**Blocked ranges (applied on both write and display paths):**
| Range | Block | Risk |
|---|---|---|
| U+0001–U+001F (excl. tab) | C0 controls | Encoded as `\xHH` in YAML |
| U+007F | DEL | Stripped |
| U+0080–U+009F | C1 controls (incl. CSI U+009B) | Stripped — ANSI injection via terminal |
| U+E0000–U+E007F | Unicode Tags block (Plane 14) | Stripped — primary LLM injection vector |
| U+E0080–U+E00FF | Reserved (Plane 14) | Stripped — contiguous with Tags block |
| U+E0100–U+E01EF | Variation Selectors Supplement | Stripped — secondary injection vector |

**Implementation (`src/hooks/team/store.rs`):**
- `yaml_quote` — write path for YAML frontmatter (strips C1 + Plane-14)
- `strip_line_breaks` — write path for Markdown body (strips Plane-14)
- `sanitize_mission` — write path for mission files (strips Plane-14 + `---` separators)
- `yaml_unescape_display` — display path (strips C0/DEL/C1/Plane-14)

### YAML Frontmatter Injection
Agent files use YAML frontmatter (`---`) for metadata. Malicious input can break out of the frontmatter block.

**Mitigations:**
- All frontmatter values written via `yaml_quote` (double-quoted scalars)
- `sanitize_mission` strips lines starting with `---` to prevent block-close injection
- Null bytes stripped before line splitting

### HTML Comment Injection
Playbook files use HTML comments for metadata (`<!-- project: … -->`).

**Mitigations in `append_playbook`:**
- `-->` → `-- >` (comment close)
- `<!--` → `<! --` (comment open)
- `--!>` → `--! >` (HTML5 bogus comment terminator)

### Path Traversal via Agent Names
Agent files are stored as `{name}.md`. A crafted name like `../../etc/passwd` could escape the agents directory.

**Mitigation:** `validate_agent_name` enforces allowlist `[a-zA-Z0-9_-]` — rejects `/`, `.`, spaces, homoglyphs, and device names. Applied in both `load_agent` and `save_agent`.

### ANSI Injection via Filenames
Invalid agent filenames printed in warnings could contain ANSI escape sequences that hijack the terminal.

**Mitigation:** `list_agents` sanitizes filenames with `is_ascii_graphic() || c == ' '` before printing.
