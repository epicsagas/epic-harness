//! grok reports a failed shell command or a missing-file read as a successful
//! `PostToolUse` carrying its own status (`exit_code`, `timed_out`,
//! `FileNotFound`), under its own tool names. Every run uses a temp HOME,
//! HARNESS_DIR and cwd with SQLite blocked (JSONL fallback); all payloads are
//! synthetic and shaped like the captured grok 1.0.89 ones (both key styles).
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";

/// Runs `observe` once with SQLite blocked (JSONL fallback) and returns the record.
fn observe(payload: serde_json::Value) -> serde_json::Value {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cwd = root.join(SLUG);
    let home = root.join("home");
    let harness = root.join("harness-root");
    let obs = harness.join("projects").join(SLUG).join("obs");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    fs::create_dir_all(&obs).unwrap();
    fs::create_dir(harness.join("harness.db")).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("observe")
        .current_dir(&cwd)
        .env("HOME", &home)
        .env("HARNESS_DIR", &harness)
        .env("GIT_CEILING_DIRECTORIES", root)
        .env_remove("GROK_HOOK_EVENT")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());

    // obs/ also holds telemetry_error_count_*.txt, so pick the log by name.
    let file = fs::read_dir(&obs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("session_") && name.ends_with(".jsonl")
        })
        .unwrap();
    let line = fs::read_to_string(file).unwrap();
    serde_json::from_str(line.lines().next().unwrap()).unwrap()
}

/// grok `PostToolUse`: snake_case and camelCase keys side by side.
fn grok(tool: &str, input: serde_json::Value, response: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "hookEventName": "post_tool_use",
        "sessionId": "synthetic-session",
        "toolName": tool,
        "toolInput": input,
        "toolResult": response,
        "hook_event_name": "PostToolUse",
        "session_id": "synthetic-session",
        "tool_name": tool,
        "tool_input": input,
        "tool_response": response,
    })
}

/// grok shell response; `output` is a byte array, the text is in `output_for_prompt`.
fn grok_shell(
    command: &str,
    text: &str,
    exit_code: serde_json::Value,
    timed_out: bool,
) -> serde_json::Value {
    grok(
        "run_terminal_command",
        serde_json::json!({"command": command}),
        serde_json::json!({
            "type": "Bash",
            "output": text.bytes().collect::<Vec<u8>>(),
            "output_for_prompt": text,
            "exit_code": exit_code,
            "command": command,
            "signal": null,
            "timed_out": timed_out,
        }),
    )
}

/// Golden: the captured `false` run (`output: []`, `output_for_prompt: "exit: 1\n"`).
#[test]
fn grok_shell_nonzero_exit_is_failure() {
    let rec = observe(grok_shell("false", "exit: 1\n", 1.into(), false));
    assert_eq!(rec["tool"], "run_terminal_command");
    assert_eq!(rec["tool_category"], "bash");
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(
        rec["error_snippet"]
            .as_str()
            .is_some_and(|s| s.contains("exit: 1")),
        "{rec}"
    );
}

#[test]
fn grok_shell_zero_exit_is_success() {
    let rec = observe(grok_shell("echo ok", "ok\n", 0.into(), false));
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null(), "{rec}");
}

#[test]
fn grok_shell_failing_tests_classify_from_output() {
    let rec = observe(grok_shell(
        "npx jest",
        "Tests: 3 failed, 5 passed\n",
        1.into(),
        false,
    ));
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "test_fail");
}

#[test]
fn grok_shell_timeout() {
    let rec = observe(grok_shell("sleep 999", "", serde_json::Value::Null, true));
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "timeout");
}

/// Golden: the captured missing-file read (`FileNotFound`, `target_file`).
#[test]
fn grok_read_missing_file_is_failure() {
    let missing = "/synthetic-root/nope-missing.txt";
    let rec = observe(grok(
        "read_file",
        serde_json::json!({"target_file": missing}),
        serde_json::json!({
            "type": "ReadFile",
            "FileNotFound": format!(
                "Error: {missing} does not exist.\nNote: your current working directory is /synthetic-root"
            ),
        }),
    ));
    assert_eq!(rec["tool_category"], "read");
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(
        rec["error_snippet"]
            .as_str()
            .is_some_and(|s| s.contains("does not exist")),
        "{rec}"
    );
    assert_eq!(rec["file_ext"], ".txt");
    // A path outside the project is masked; it is not a JSON blob.
    assert_eq!(rec["action"], "<PATH>");
}

#[test]
fn grok_read_success_with_keywords_is_success() {
    let rec = observe(grok(
        "read_file",
        serde_json::json!({"target_file": "src/a.rs"}),
        serde_json::json!({"type": "ReadFile", "content": "Error: boom\nTraceback"}),
    ));
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null(), "{rec}");
}

/// Claude `PostToolUseFailure` (M2 capture) keeps its own path.
#[test]
fn claude_failure_event_is_unchanged() {
    let rec = observe(serde_json::json!({
        "hook_event_name": "PostToolUseFailure",
        "session_id": "synthetic-session",
        "tool_name": "Bash",
        "tool_input": {"command": "false", "description": "Run the false command"},
        "error": "Exit code 1",
        "is_interrupt": false,
    }));
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(
        rec["error_snippet"]
            .as_str()
            .is_some_and(|s| s.contains("Exit code 1")),
        "{rec}"
    );
}

fn codex_bash(command: &str, response: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": "synthetic-session",
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "tool_response": response,
    })
}

/// Codex M4b capture: a read-only command on a merged-stream host stays success.
#[test]
fn codex_read_only_failure_text_is_still_success() {
    let rec = observe(codex_bash(
        "ls /nonexistent-dir-xyz",
        "ls: /nonexistent-dir-xyz: No such file or directory\n",
    ));
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null(), "{rec}");
}

#[test]
fn codex_build_failure_text_is_still_runtime_error() {
    let rec = observe(codex_bash("cargo build", "error: could not compile `x`\n"));
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
}
