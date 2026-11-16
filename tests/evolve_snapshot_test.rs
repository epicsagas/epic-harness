//! `epic harness snapshot` and `diff` through the real binary. Every run uses a temp
//! HOME, HARNESS_DIR and cwd.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SLUG: &str = "fixture_project";

struct Env {
    _temp: tempfile::TempDir,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

impl Env {
    fn new(guard_rules: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        let project = harness.join("projects").join(SLUG);
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("guard-rules.yaml"), guard_rules).unwrap();
        Self {
            _temp: temp,
            cwd,
            home,
            harness,
        }
    }

    fn harness(&self, args: &[&str]) -> (bool, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("harness")
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    fn snapshot(&self) -> serde_json::Value {
        let (ok, stdout) = self.harness(&["snapshot"]);
        assert!(ok, "{stdout}");
        serde_json::from_str(&stdout).unwrap()
    }
}

const BLOCKED: &str = "blocked:\n  - pattern: synthetic-probe | msg: synthetic rule\nwarned:\n";
const WARNED: &str = "blocked:\nwarned:\n  - pattern: synthetic-probe | msg: synthetic rule\n";

#[test]
fn snapshot_distinguishes_blocked_from_warned_rules() {
    let blocked = Env::new(BLOCKED).snapshot();
    let warned = Env::new(WARNED).snapshot();
    assert_ne!(blocked["guard_rules"], warned["guard_rules"]);
    assert_ne!(blocked["hash"], warned["hash"]);
}

#[test]
fn snapshot_diff_reports_a_rule_moving_from_blocked_to_warned() {
    let before = Env::new(BLOCKED);
    let after = Env::new(WARNED);
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = [&before, &after]
        .iter()
        .enumerate()
        .map(|(i, env)| {
            let path = dir.path().join(format!("snap{i}.json"));
            fs::write(&path, env.snapshot().to_string()).unwrap();
            path
        })
        .collect();
    let (_, stdout) = before.harness(&[
        "diff",
        paths[0].to_str().unwrap(),
        paths[1].to_str().unwrap(),
    ]);
    assert!(!stdout.contains("no differences"), "{stdout}");
}

#[test]
fn snapshot_keeps_rule_order_and_ignores_comments() {
    let one = "# note\nblocked:\n  - pattern: a | msg: first\n  - pattern: b | msg: second\n";
    let swapped = "blocked:\n  - pattern: b | msg: second\n  - pattern: a | msg: first\n";
    let commented =
        "blocked:\n  # other note\n  - pattern: a | msg: first\n\n  - pattern: b | msg: second\n";
    let base = Env::new(one).snapshot();
    assert_ne!(
        base["guard_rules"],
        Env::new(swapped).snapshot()["guard_rules"]
    );
    assert_eq!(
        base["guard_rules"],
        Env::new(commented).snapshot()["guard_rules"]
    );
}
