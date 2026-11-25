//! Claude Code reads a PreToolUse block from the exit code OR from the JSON
//! decision (https://code.claude.com/docs/en/hooks, "PreToolUse decision
//! control"). Under `pwsh -Command` an external exit 2 becomes 1, a
//! non-blocking error, so on Claude every guard block exits 0 and prints one
//! `permissionDecision: deny` object. Codex, grok and unknown hosts keep exit 2.
//! Every run uses a temp HOME, HARNESS_DIR and cwd; payloads are synthetic.
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs `epic guard` with `payload` on stdin. `setup` receives the
/// orchestrator dir of the temp harness so a test can seed run/control state.
fn guard(
    payload: serde_json::Value,
    envs: &[(&str, &str)],
    setup: impl FnOnce(&std::path::Path),
) -> Out {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cwd = root.join(SLUG);
    let home = root.join("home");
    let harness = root.join("harness-root");
    let orch = harness.join("projects").join(SLUG).join("orchestrator");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    setup(&orch);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
    cmd.arg("guard")
        .current_dir(&cwd)
        .env("HOME", &home)
        .env("HARNESS_DIR", &harness)
        .env("GIT_CEILING_DIRECTORIES", root)
        .env_remove("EPIC_ORCHESTRATION")
        .env_remove("EPIC_AGENT_ID")
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("GROK_HOOK_EVENT")
        .env_remove("GROK_SESSION_ID")
        .env_remove("GROK_WORKSPACE_ROOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Out {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

fn no_setup(_: &std::path::Path) {}

const CLAUDE_ENV: &[(&str, &str)] = &[("CLAUDE_PROJECT_DIR", "/tmp/synthetic-project")];

fn claude_bash(command: &str) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "synthetic-session",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "permission_mode": "default",
        "tool_use_id": "toolu_synthetic",
    })
}

/// Exactly one JSON object on stdout, nothing else (no stdin echo).
fn only_json(stdout: &str) -> serde_json::Value {
    let mut it = serde_json::Deserializer::from_str(stdout).into_iter::<serde_json::Value>();
    let first = it.next().expect("stdout holds a JSON object").unwrap();
    assert!(it.next().is_none(), "more than one JSON value: {stdout}");
    first
}

fn assert_deny(out: &Out, reason_contains: &str) {
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let v = only_json(&out.stdout);
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["hookEventName"], "PreToolUse");
    assert_eq!(h["permissionDecision"], "deny");
    let reason = h["permissionDecisionReason"].as_str().unwrap();
    assert!(reason.contains(reason_contains), "reason: {reason}");
    assert!(
        out.stderr.contains(reason_contains),
        "stderr keeps the message: {}",
        out.stderr
    );
}

#[test]
fn claude_bash_block_is_exit_0_with_one_deny_object() {
    let out = guard(claude_bash("rm -rf /"), CLAUDE_ENV, no_setup);
    assert_deny(&out, "BLOCKED: rm -rf / blocked");
}

#[test]
fn claude_guard_rules_block_is_a_deny() {
    let out = guard(claude_bash("DROP TABLE prod_users"), CLAUDE_ENV, no_setup);
    assert_deny(&out, "BLOCKED: DROP on production DB blocked");
}

#[test]
fn claude_agent_spawn_block_is_a_deny() {
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "synthetic-session",
        "tool_name": "Agent",
        "tool_input": {"prompt": "synthetic task"},
    });
    let envs = [CLAUDE_ENV, &[("EPIC_ORCHESTRATION", "enabled")]].concat();
    let out = guard(payload, &envs, |orch| {
        fs::create_dir_all(orch).unwrap();
        fs::write(
            orch.join("run.json"),
            r#"{"id":"r","status":"running","agents":[],"created_at":"t","updated_at":"t"}"#,
        )
        .unwrap();
        fs::write(
            orch.join("control.json"),
            r#"{"action":"cancel","target":"all","message":null,"generation":2}"#,
        )
        .unwrap();
    });
    assert_deny(&out, "[orchestrator] Agent ");
    assert!(out.stderr.contains("cancelled by user directive (gen 2)"));
}

#[test]
fn claude_edit_orchestration_block_is_a_deny() {
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "synthetic-session",
        "tool_name": "Edit",
        "tool_input": {"file_path": "/src/main.rs"},
    });
    let envs = [
        CLAUDE_ENV,
        &[
            ("EPIC_ORCHESTRATION", "enabled"),
            ("EPIC_AGENT_ID", "agent-x"),
        ],
    ]
    .concat();
    let out = guard(payload, &envs, |orch| {
        fs::create_dir_all(orch).unwrap();
        fs::write(orch.join("control.json"), r#"{"pause":"agent-x"}"#).unwrap();
    });
    assert_deny(&out, "BLOCKED: control.json pause directive active");
}

/// Warn-only and allowed commands are untouched: exit 0, never a deny.
#[test]
fn claude_warn_and_allow_never_deny() {
    for cmd in ["git status", "rm -rf build/"] {
        let out = guard(claude_bash(cmd), CLAUDE_ENV, no_setup);
        assert_eq!(out.code, 0, "{cmd}: {}", out.stderr);
        assert!(
            !out.stdout.contains("permissionDecision"),
            "{cmd}: {}",
            out.stdout
        );
    }
}

#[test]
fn codex_payload_still_exits_2() {
    // Codex adds `turn_id`; CLAUDE_PROJECT_DIR is set on purpose to show the
    // payload marker wins.
    let mut p = claude_bash("rm -rf /");
    p["turn_id"] = "synthetic-turn".into();
    let out = guard(p, CLAUDE_ENV, no_setup);
    assert_eq!(out.code, 2, "stderr: {}", out.stderr);
}

#[test]
fn grok_payload_still_exits_2() {
    // grok sends both key styles and exports CLAUDE_PROJECT_DIR too.
    let p = serde_json::json!({
        "hookEventName": "pre_tool_use",
        "sessionId": "synthetic-session",
        "toolName": "Bash",
        "toolInput": {"command": "rm -rf /"},
        "hook_event_name": "PreToolUse",
        "session_id": "synthetic-session",
        "tool_name": "Bash",
        "tool_input": {"command": "rm -rf /"},
    });
    let out = guard(p.clone(), CLAUDE_ENV, no_setup);
    assert_eq!(out.code, 2, "camelCase keys: {}", out.stderr);
    // Even without the camelCase keys, grok's runner variables identify it.
    let mut snake = claude_bash("rm -rf /");
    snake["session_id"] = "synthetic-session".into();
    let envs = [CLAUDE_ENV, &[("GROK_HOOK_EVENT", "pre_tool_use")]].concat();
    assert_eq!(guard(snake, &envs, no_setup).code, 2);
}

/// Without the Claude environment the host is unknown: keep the exit-2 contract.
#[test]
fn unknown_host_still_exits_2() {
    let out = guard(claude_bash("rm -rf /"), &[], no_setup);
    assert_eq!(out.code, 2, "stderr: {}", out.stderr);
}
