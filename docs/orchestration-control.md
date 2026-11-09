# Orchestration controls and host hooks

With `EPIC_ORCHESTRATION=enabled`, a matching `pause` or `cancel` directive
prevents a synchronous Agent `PreToolUse` invocation. The observer returns exit
code 2 and an explicit `permissionDecision: "deny"` response. The Claude plugin
runs this particular observer synchronously so the host can enforce the result.
Unrelated agent targets continue normally.

`SubagentStart` is a lifecycle notification: the host has already started the
agent. It records a paused agent as `blocked` or a cancelled agent as `failed`,
then returns success. It cannot claim to terminate a host process. Stopping an
already-running host agent remains the host's responsibility. `SubagentStop`
and `PostToolUse` record completion, including outputless completion events.

Without full orchestration enabled, lifecycle tracking retains its ordinary
behavior. Post-tool observations remain asynchronous. An existing initialized
project harness is required, as for other observations.

Regression: `cargo test --test issue_241_agent_control` uses synthetic isolated
project roots to exercise targeted pause/cancel, explicit denial, lifecycle
tracking and an unrelated allowed agent through the built hook executable.
