//! Read/Glob/Grep output is quoted material: keywords inside it must not mark
//! the call as a failure. Only an explicit host error flag does. Every run uses
//! a temp HOME, HARNESS_DIR and cwd; all payloads are synthetic.
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

fn payload(tool: &str, response: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "session_id": "synthetic-session",
        "tool_name": tool,
        "tool_input": {"file_path": "src/a.rs"},
        "tool_response": response,
    })
}

#[test]
fn read_body_with_error_keywords_is_success() {
    let rec = observe(payload(
        "Read",
        serde_json::json!("Error: boom\nTraceback (most recent call last)\n  at Foo.Bar(x)"),
    ));
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null(), "{rec}");
}

#[test]
fn grep_object_body_with_error_keywords_is_success() {
    let rec = observe(payload(
        "Grep",
        serde_json::json!({"output": "src/a.cs: Assert.Equal(1,x) at Foo.Bar("}),
    ));
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null(), "{rec}");
}

#[test]
fn read_with_explicit_host_error_is_failure() {
    let rec = observe(payload(
        "Read",
        serde_json::json!({"is_error": true, "error": "File does not exist."}),
    ));
    assert_eq!(rec["result"], "error");
    assert!(rec["failure_category"].is_string(), "{rec}");
}

#[test]
fn read_host_error_text_is_the_snippet() {
    let rec = observe(payload(
        "Read",
        serde_json::json!({"is_error": true, "error": "File does not exist."}),
    ));
    assert!(
        rec["error_snippet"]
            .as_str()
            .is_some_and(|s| s.contains("File does not exist.")),
        "{rec}"
    );
}

#[test]
fn read_host_error_without_body_has_no_snippet() {
    let rec = observe(payload("Read", serde_json::json!({"is_error": true})));
    assert_eq!(rec["result"], "error");
    assert!(rec["error_snippet"].is_null(), "{rec}");
}

#[test]
fn non_read_tool_keyword_classification_is_unchanged() {
    let rec = observe(payload(
        "WebFetch",
        serde_json::json!("Traceback (most recent call last)"),
    ));
    assert_eq!(rec["result"], "error");
    assert!(rec["failure_category"].is_string(), "{rec}");
}
