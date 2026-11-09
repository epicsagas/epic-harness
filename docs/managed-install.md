# Managed source installations

A verified source or fork rollout can prevent plugin bootstrap and `epic update`
from replacing its executables with an upstream release. The rollout writes
`epic-managed.json` beside both executable aliases:

```json
{
  "schema": 1,
  "repository": "owner/epic-harness",
  "commit": "0123456789abcdef0123456789abcdef01234567",
  "automatic_updates": false
}
```

The commit must be a full 40-character hexadecimal SHA. Extra fields such as
artifact hashes are allowed. This metadata records the rollout's identity; it
does not verify artifact provenance or replace the rollout's hash checks.

With this marker, SessionStart bootstrap keeps the effective PATH executable
even when the installed plugin version is newer. `epic update --check` reports
the version and pin without contacting upstream. `epic update` and
`epic update --force` refuse replacement with exit code 1 before MCP, dashboard,
or sync-marker side effects. Updating a managed installation is the rollout's
responsibility, including updating the marker after verified replacement.

Ordinary installers do not create this marker. Missing, malformed, unsupported,
or update-enabled markers retain normal behavior. Remove the marker explicitly
to resume upstream updates. Bootstrap considers only the first effective
`epic-harness` executable on PATH, so a later unrelated installation cannot pin
the active one. CLI checks use the marker beside the running executable.
