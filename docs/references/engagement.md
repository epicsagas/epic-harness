# Engagement Context — Security Assessment Scoping

## Overview

An engagement context file (`.harness/engagement.md`) defines the authorized scope for security assessments. When present, the `secure` skill and `audit:security` mode auto-load it to constrain checks to the approved scope.

**Opt-in**: Without this file, existing OWASP checklist behavior applies unchanged.

## File Location

Place in your project root: `.harness/engagement.md`

## Template

```markdown
# Security Engagement Context

## Authorization
- **Approved by**: <name or team>
- **Date**: YYYY-MM-DD
- **Reference**: <ticket/issue/JIRA link>

## Scope
### In Scope
- <system or component 1>
- <system or component 2>
- <specific endpoints or services>

### Out of Scope
- <excluded systems>
- <excluded endpoints>

## Constraints
- **Method**: <black-box / white-box / grey-box>
- **Rules of engagement**: <rate limits, no-DoS, no-exfil, etc.>
- **Disclosure path**: <where to report findings>

## Environment
- **Target environment**: <staging / production / dev>
- **Credentials provided**: <yes/no — if yes, stored in secrets vault>
- **Network access**: <VPN / direct / restricted>

## Exclusions
- <Known issues that should not be flagged>
- <Third-party services not under assessment>
```

## Auto-Loading

When `epic secure` or `audit:security` runs:

1. Check for `.harness/engagement.md` in project root
2. If found:
   - Load scope, constraints, and exclusions
   - Restrict security checks to in-scope components
   - Skip findings matching exclusion patterns
   - Enforce rules of engagement (e.g., no active exploitation if grey-box)
   - Include authorization reference in audit report header
3. If not found:
   - Apply full OWASP Top 10 checklist (default behavior)
   - No scope restrictions

## Report Integration

When engagement context is active, the security audit report includes:

```
## Security Audit (Engagement Context)
- Engagement: <reference from engagement.md>
- Scope: <in-scope summary>
- Method: <black-box/white-box/grey-box>
- Findings restricted to authorized scope

<standard security findings>
```

## Safety

- Engagement context is **advisory** — it guides where to look, not whether to report
- CRITICAL findings in out-of-scope areas are still reported (as informational)
- Engagement file is **never** committed to public repos (add `.harness/engagement.md` to `.gitignore`)
- If engagement.md contains conflicting or malformed sections, fall back to full OWASP checklist with a warning
