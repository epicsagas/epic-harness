//! `epic harness snapshot` through the real binary: read-only, validates flags.
//! Every run uses a temp HOME, HARNESS_DIR and cwd.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

impl Env {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join("fixture_project");
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        Self {
            _temp: temp,
            root,
            cwd,
            home,
            harness,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("harness")
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .output()
            .unwrap()
    }
}

fn contains_harness_db(dir: &Path) -> bool {
    fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
        let p = e.path();
        p.file_name().is_some_and(|n| n == "harness.db") || (p.is_dir() && contains_harness_db(&p))
    })
}

#[test]
fn snapshot_on_empty_store_is_json_and_creates_no_db() {
    let env = Env::new();
    let out = env.run(&["snapshot"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&out.stdout).expect("stdout is JSON");
    assert!(
        !contains_harness_db(&env.root),
        "snapshot must not create harness.db"
    );
}

#[test]
fn snapshot_help_exits_zero() {
    let env = Env::new();
    assert_eq!(env.run(&["snapshot", "--help"]).status.code(), Some(0));
}

#[test]
fn snapshot_unknown_flag_exits_two() {
    let env = Env::new();
    let out = env.run(&["snapshot", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown flag '--bogus'"));
}
