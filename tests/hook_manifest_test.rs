//! Hook manifest registration tests.
//!
//! The orchestrator's start/stop logic is fully implemented and unit-tested,
//! but an agent only reaches `Running` if the host is actually told to invoke
//! `epic observe` on the matching event. A missing manifest entry leaves every
//! one of those unit tests green while the feature is dead in the product, so
//! the registrations are pinned here.

use serde_json::Value;

fn manifest(path: &str) -> Value {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

/// Every command any hook registered for `event` with `matcher` would run.
fn commands_for(m: &Value, event: &str, matcher: &str) -> Vec<String> {
    m["hooks"][event]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry["matcher"].as_str() == Some(matcher))
        .flat_map(|entry| {
            entry["hooks"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter_map(|h| h["command"].as_str().map(String::from))
        })
        .collect()
}

/// True when `cmd` invokes `epic <sub>`. The Claude manifest calls the binary
/// directly (`epic observe`); the Codex and Grok manifests route through the
/// launcher (`node ${...}/registry/scripts/hooks/run.mjs <sub>`).
fn invokes(cmd: &str, sub: &str) -> bool {
    cmd.contains(&format!("epic {sub}"))
        // launcher form: `node ${PLUGIN_ROOT|GROK_PLUGIN_ROOT}/registry/scripts/hooks/run.mjs <sub>`
        || cmd.ends_with(&format!("run.mjs {sub}"))
}

const CLAUDE: &str = ".claude-plugin/hooks.json";
const CODEX: &str = ".codex-plugin/hooks.json";
const GROK: &str = ".grok-plugin/hooks.json";

/// The reported defect: `observe` ran only on PostToolUse, which carries the
/// agent's output, so `track_agent_spawn` always took the completion branch.
/// Agents jumped straight to Done and the live-agent view never showed one
/// running.
#[test]
fn claude_marks_subagents_running_before_they_finish() {
    let m = manifest(CLAUDE);
    let pre = commands_for(&m, "PreToolUse", "Agent");
    assert!(
        pre.iter().any(|c| invokes(c, "observe")),
        "PreToolUse/Agent must invoke observe, else agents never enter Running; got {pre:?}"
    );
}

/// Completion must stay observed too — the wildcard PostToolUse covers it.
#[test]
fn claude_observes_tool_completion() {
    let m = manifest(CLAUDE);
    let post = commands_for(&m, "PostToolUse", "*");
    assert!(
        post.iter().any(|c| invokes(c, "observe")),
        "PostToolUse/* must invoke observe; got {post:?}"
    );
}

/// Guard must not be dragged onto the Agent matcher: it is a Bash gate, and
/// running it per subagent spawn would cost a process for nothing.
#[test]
fn claude_guard_stays_scoped_to_bash() {
    let m = manifest(CLAUDE);
    assert!(
        commands_for(&m, "PreToolUse", "Bash")
            .iter()
            .any(|c| invokes(c, "guard")),
        "guard must stay registered for Bash"
    );
    assert!(
        !commands_for(&m, "PreToolUse", "Agent")
            .iter()
            .any(|c| invokes(c, "guard")),
        "guard must not run on Agent spawns"
    );
}

/// Codex reports subagents as lifecycle events rather than a tool call.
#[test]
fn codex_registers_subagent_lifecycle() {
    let m = manifest(CODEX);
    for event in ["SubagentStart", "SubagentStop"] {
        let cmds = commands_for(&m, event, "*");
        assert!(
            cmds.iter().any(|c| invokes(c, "observe")),
            "{event} must invoke observe; got {cmds:?}"
        );
    }
}

/// Edits are the other half of the evolution loop's input. Codex delivers them
/// as `apply_patch`, whose hook identity carries `Write`/`Edit` as matcher
/// aliases (codex-rs core/src/tools/hook_names.rs), so the three names must
/// share one matcher group — separate groups fire the same hooks once per
/// alias per patch event (measured: observe ran 6x for one edit on codex-cli
/// 0.155.0).
#[test]
fn codex_observes_edits_not_just_bash() {
    let m = manifest(CODEX);
    let cmds = commands_for(&m, "PostToolUse", "apply_patch|Edit|Write");
    assert!(
        cmds.iter().any(|c| invokes(c, "observe")),
        "PostToolUse edit matcher must invoke observe; got {cmds:?}"
    );
    assert!(
        cmds.iter().any(|c| invokes(c, "polish")),
        "PostToolUse edit matcher must invoke polish; got {cmds:?}"
    );
    for matcher in ["apply_patch", "Edit", "Write"] {
        let cmds = commands_for(&m, "PostToolUse", matcher);
        assert!(
            cmds.is_empty(),
            "PostToolUse/{matcher} must be merged into the combined edit matcher; got {cmds:?}"
        );
    }
}

/// All three hosts support PreCompact, and snapshots are what `resume`
/// restores.
#[test]
fn all_hosts_snapshot_before_compaction() {
    for path in [CLAUDE, CODEX, GROK] {
        let cmds = commands_for(&manifest(path), "PreCompact", "*");
        assert!(
            cmds.iter().any(|c| invokes(c, "snapshot")),
            "{path}: PreCompact must invoke snapshot; got {cmds:?}"
        );
    }
}

/// The Grok manifest rides the same launcher as Codex: guard stays scoped to
/// Bash, observe covers the PostToolUse wildcard, SessionStart resumes.
#[test]
fn grok_registers_guard_observe_and_resume() {
    let m = manifest(GROK);
    assert!(
        commands_for(&m, "PreToolUse", "Bash")
            .iter()
            .any(|c| invokes(c, "guard")),
        "guard must stay registered for Bash"
    );
    assert!(
        !commands_for(&m, "PreToolUse", "Bash")
            .iter()
            .any(|c| invokes(c, "observe")),
        "observe must not run pre-Bash (PostToolUse owns it)"
    );
    assert!(
        commands_for(&m, "PostToolUse", "*")
            .iter()
            .any(|c| invokes(c, "observe")),
        "PostToolUse/* must invoke observe"
    );
    assert!(
        commands_for(&m, "SessionStart", "*")
            .iter()
            .any(|c| invokes(c, "resume")),
        "SessionStart must invoke resume"
    );
}
