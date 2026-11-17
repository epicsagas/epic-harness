//! `resume` reads memory through hooks, with no model intent, so it must not
//! bump `access_count` (ADR 0003). Every run uses a temp HOME, HARNESS_DIR and
//! cwd; the real `~/.harness` is never opened.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
const BIN: &str = env!("CARGO_BIN_EXE_epic-harness");

/// Runs the binary in the temp env; `PATH` starts with the binary's directory
/// so resume's `epic-harness mem context` subprocess finds this build.
fn run(root: &Path, args: &[&str], stdin: &str) -> std::process::Output {
    let bin_dir = Path::new(BIN).parent().unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin_dir.to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(root.join(SLUG))
        .env("HOME", root.join("home"))
        .env("HARNESS_DIR", root.join("harness-root"))
        .env_remove("HARNESS_ROOT")
        .env("GIT_CEILING_DIRECTORIES", root)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    out
}

#[test]
fn c_resume_does_not_bump_access_count() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join(SLUG)).unwrap();
    fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
    fs::write(
        root.join("home/.config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    // Resume step 5a only runs when this directory exists.
    fs::create_dir_all(root.join("harness-root/memory")).unwrap();
    // No dashboard auto-launch: it would add dashboard.lock (and spawn a
    // server) whenever port 7700 is free, as on CI.
    fs::write(
        root.join("harness-root/config.toml"),
        "[dashboard]\nport = 0\nauto_open = false\n",
    )
    .unwrap();

    run(
        root,
        &[
            "mem",
            "add",
            "--title",
            "alpha decision",
            "--type",
            "decision",
            "--project",
            SLUG,
            "--body",
            "synthetic body",
        ],
        "",
    );

    run(root, &["resume"], "{}");

    // `mem list` omits access_count, so read it with a passive context call.
    let out = run(
        root,
        &["mem", "context", "--project", SLUG, "--passive"],
        "",
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["access_count"], 0, "{rows:?}");
}
