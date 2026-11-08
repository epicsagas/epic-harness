# Antigravity integration

Install and enable the plugin with `agy plugin install` and `agy plugin enable
epic` as described in the quickstart. Antigravity auto-discovers the root
`hooks.json` and `mcp_config.json`; `plugin.json` does not select a hook manifest.
The `.gemini/config` directory is Antigravity's configuration directory.

The named `epic` hook uses flat `PreInvocation` and `PostInvocation` handlers.
Each event invokes its lifecycle action once. No additional `SessionStart` or
`Stop` handlers are registered, and no pre-tool hooks are enabled here.
Other named hooks, including Orca's, can coexist in the global configuration.

The Node adapter chooses the first existing directory in `workspacePaths`,
ignoring invalid or inaccessible entries. If none is valid it uses the user's
home directory. `conversationId` becomes `session_id`. The documented
`transcriptPath` is preserved without guessing transcript contents or inventing
a Rust `transcript_path` field. Transcript ingestion is not currently supported.
Payload data travels only over stdin. The executable is `epic` on PATH, or the
trusted `EPIC_BINARY` environment override, and is spawned without a shell.

Resume context is translated from Epic's JSON to Antigravity's `injectSteps` /
`systemMessage` contract. Reflection emits `{}`. Child stderr is forwarded and
nonzero exit codes are preserved. Malformed input or invalid child JSON returns
`{}` with exit 1; a missing executable returns `{}` with exit 0 and a diagnostic
on stderr. Other spawn failures, signals, and the 25-second child timeout return
exit 1. Hook no-op behavior remains unchanged.

After updating a plugin that previously used a global workaround, run:

```sh
node registry/scripts/migrate-antigravity.mjs
```

Optional arguments select the configuration and plugin directories for testing
or custom installations. Back up configuration before migrating. The migration
checks the maintained plugin first, retires only recognized global Epic
invocation handlers, and preserves other named hooks and all MCP configuration.
It is safe to repeat. An unrecognized Epic handler stops migration without
writing configuration. This prevents the old global workaround and the plugin
from invoking the same lifecycle twice.

Validate with `agy plugin validate . --json` and run the behavioral regressions
with `node --test registry/scripts/hooks/antigravity.test.mjs`.
