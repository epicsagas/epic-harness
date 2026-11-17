//! A first session must get the cold-start presets it just seeded: they are
//! created before the evolved listing, so they are either injected on stdout
//! or announced as withheld on stderr. Every run uses a temp HOME, HARNESS_DIR
//! and cwd; the real `~/.harness` is never opened.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
const PRESET: &str = "evo-rs-care";

#[test]
fn first_session_sees_seeded_presets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cwd = root.join(SLUG);
    let harness = root.join("harness-root");
    fs::create_dir_all(&cwd).unwrap();
    fs::write(cwd.join("Cargo.toml"), "").unwrap();
    fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
    fs::write(
        root.join("home/.config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    fs::create_dir_all(&harness).unwrap();
    // No dashboard auto-launch: it would add dashboard.lock (and spawn a
    // server) whenever port 7700 is free, as on CI.
    fs::write(
        harness.join("config.toml"),
        "[dashboard]\nport = 0\nauto_open = false\n[model]\ninject_templates = true\n",
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("resume")
        .current_dir(&cwd)
        .env("HOME", root.join("home"))
        .env("HARNESS_DIR", &harness)
        .env_remove("HARNESS_ROOT")
        .env("GIT_CEILING_DIRECTORIES", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");

    assert!(
        harness
            .join("projects")
            .join(SLUG)
            .join("evolved")
            .join(PRESET)
            .is_dir(),
        "rust preset was not seeded"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // The holdout arm is date-hashed, so the preset is injected or announced.
    assert!(
        stdout.contains(PRESET) || stderr.contains(PRESET),
        "preset neither injected nor announced\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(!harness.join("dashboard.lock").exists());
}
