# Native task-context dispatch

Claude Code and Codex hosts supporting `UserPromptSubmit` run `epic dispatch`
before a submitted task. The plugin adds one command to that event and preserves
the existing lifecycle hooks. Antigravity has no verified equivalent prompt event
in Epic's adapter contract, so its lifecycle integration remains the supported path.

The payload must contain `hook_event_name: "UserPromptSubmit"` and a string
`prompt`. The process uses its working directory and existing project resolver;
payload paths cannot change the scope. It never runs payload text or selects
files/executables from that text. Ordinary empty, malformed, wrong-type,
unsupported-event and oversized input exits successfully without stdout.

Native selection delivers **advisory guidance**, not proof the model has invoked
or complied with every skill. Matching task words select up to two of the embedded
debug, secure, perf and document skills. Implementation words additionally select
tdd; selected tasks include verify. This bounded heuristic can miss implicit or
non-English intent; explicit skill invocation remains available. Embedded static
guidance takes priority and cannot load an arbitrary or holdout evolved skill.

The same hook calls the in-process smart recall API with the current project and
a normalized task hint. The existing memory API applies exact project membership
and stale filtering before limiting results. A relevant memory below the generic
SessionStart top set can therefore surface when its task arrives. No hint means
no recall; no matches and an unavailable store have distinct metadata statuses.
An unavailable store adds a short notice without failing the user's task.
Memory excerpts are masked for common credentials/paths and labeled as untrusted
reference data; do not treat stored text as executable instructions.

Budgets are explicit: 256 KiB input, a five-second stdin wait, 4,096 prompt
characters considered, a 256-character hint, four static skills at 1,600 characters
each, three memory excerpts with 120-character titles/600-character bodies, and
10,000 Unicode characters total. Input caps apply before JSON parsing; output caps
preserve UTF-8. Each nonempty response is exactly one JSON document containing
`hookSpecificOutput.hookEventName` and `hookSpecificOutput.additionalContext`.
The prompt is never passed through to stdout or copied to stderr.
Memory records are admitted whole within the remaining context budget, accounting
for JSON escape expansion. Delivery counts include only admitted records; a
`budget_exhausted` status distinguishes matches that could not fit.

`$HARNESS_DIR/projects/{slug}/dispatch/dispatch_YYYYMMDD.jsonl` records timestamp,
event, fixed source/trigger categories, selected static skill names, delivered counts, memory status, output length
and response kind. It records no prompt, hint, memory title/body, session ID or
free-form errors. Distinct turns are not deduplicated by timestamp or prompt hash.
These records establish selection/delivery only. They do not measure execution,
test success, policy enforcement or learning improvement.

Validate the native protocol with `cargo test --test prompt_dispatch_test`.
The Windows source CI job builds this changed source and runs those
native regressions; real-shell transport tests also cover literal host roots.
Existing released-artifact acceptance continues to check the released lifecycle
commands. It explicitly defers the new dispatch command to source-built checks,
because the prior release does not implement that CLI command.
Host acceptance additionally requires a real supported host session demonstrating
that synthetic private task memory reaches the model through `additionalContext`.
Native tests alone do not prove host consumption or model behavior.
