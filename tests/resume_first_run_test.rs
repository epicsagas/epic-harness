//! The first-run banner must print on the first startup even when the host
//! sends a session id (the startup lock creates the project dir). Claude keeps
//! the banner on stderr and echoes stdin; Codex gets it inside its single JSON
//! document. Every run uses a temp HOME, HARNESS_DIR and cwd.
#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn resume(root: &Path, payload: &Value) -> (String, String) {
    fs::create_dir_all(root.join("proj")).unwrap();
    fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
    fs::write(
        root.join("home/.config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("resume")
        .current_dir(root.join("proj"))
        .env("HOME", root.join("home"))
        .env("HARNESS_DIR", root.join("harness-root"))
        .env_remove("HARNESS_ROOT")
        .env("GIT_CEILING_DIRECTORIES", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn claude_first_startup_prints_banner_once() {
    let temp = tempfile::tempdir().unwrap();
    let first = json!({"session_id": "s1", "source": "startup"});
    let (stdout, stderr) = resume(temp.path(), &first);
    assert_eq!(stdout, first.to_string(), "stdin echo must stay intact");
    assert!(stderr.contains("[resume] Initialized"), "{stderr}");

    let (_, stderr) = resume(
        temp.path(),
        &json!({"session_id": "s2", "source": "startup"}),
    );
    assert!(!stderr.contains("[resume] Initialized"), "{stderr}");
}

#[test]
fn codex_first_startup_banner_stays_in_one_json_document() {
    let temp = tempfile::tempdir().unwrap();
    let (stdout, _) = resume(
        temp.path(),
        &json!({"session_id": "s1", "source": "startup", "hook_event_name": "SessionStart"}),
    );
    let out: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout:?}"));
    let ctx = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("[resume] Initialized"), "{ctx}");
}
