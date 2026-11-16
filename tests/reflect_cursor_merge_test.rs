//! `merge-project` must never write or reset a reflection cursor (ADR 0002,
//! decision 6), through the real binary. Temp HOME and HARNESS_DIR; synthetic data.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const CURSOR: &str = "reflect_cursor.json";

fn merge(root: &Path, from: &str, to: &str) {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["merge-project", "--from", from, "--to", to])
        .current_dir(root)
        .env("HOME", &home)
        .env("HARNESS_DIR", root.join("harness-root"))
        .env("GIT_CEILING_DIRECTORIES", root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "merge-project failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn project(root: &Path, slug: &str) -> std::path::PathBuf {
    let dir = root.join("harness-root").join("projects").join(slug);
    fs::create_dir_all(dir.join("obs")).unwrap();
    fs::write(dir.join("obs").join("session_x.jsonl"), "{}\n").unwrap();
    dir
}

#[test]
fn b_merge_project_leaves_cursor_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let a = project(tmp.path(), "a");
    let b = project(tmp.path(), "b");
    let a_cursor = br#"{"v":2,"obs_id":7,"ts_floor":"2026-08-31T10:00:00Z"}"#;
    let b_cursor = br#"{"v":2,"obs_id":42,"ts_floor":null}"#;
    fs::write(a.join(CURSOR), a_cursor).unwrap();
    fs::write(b.join(CURSOR), b_cursor).unwrap();

    merge(tmp.path(), "a", "b");

    assert_eq!(fs::read(b.join(CURSOR)).unwrap(), b_cursor);
    assert_eq!(fs::read(a.join(CURSOR)).unwrap(), a_cursor);
}

#[test]
fn b_merge_project_does_not_create_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let a = project(tmp.path(), "a");
    let b = project(tmp.path(), "b");
    fs::write(a.join(CURSOR), br#"{"v":2,"obs_id":7,"ts_floor":null}"#).unwrap();

    merge(tmp.path(), "a", "b");

    assert!(!b.join(CURSOR).exists());
}
