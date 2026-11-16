//! `epic eval` gating through the real binary. Every run uses a temp HOME,
//! HARNESS_DIR and cwd; commands are small shell scripts written into the temp cwd.
#![cfg(unix)]

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
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::create_dir_all(harness.join("projects").join(SLUG).join("eval")).unwrap();
        Self {
            _temp: temp,
            cwd,
            home,
            harness,
        }
    }

    fn script(&self, name: &str, body: &str) {
        fs::write(self.cwd.join(name), body).unwrap();
    }

    fn config(&self, yaml: &str) {
        let path = self
            .harness
            .join("projects")
            .join(SLUG)
            .join("eval/eval.yaml");
        fs::write(path, yaml).unwrap();
    }

    /// Returns (exit code, stdout, stderr).
    fn eval(&self) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(["eval", "--json"])
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn correctness(&self, summary: &str, exit: i32, min_pass_rate: &str) -> (i32, String, String) {
        self.script("t.sh", &format!("echo '{summary}'\nexit {exit}\n"));
        self.config(&format!(
            "stack: rust\ndimensions:\n  correctness:\n    enabled: true\n    command: \"sh t.sh\"\n    extra:\n      min_pass_rate: {min_pass_rate}\n  regression:\n    enabled: false\n"
        ));
        self.eval()
    }

    fn composite(&self, script: &str) -> (i32, String, String) {
        self.script("b.sh", script);
        self.config(
            "dimensions:\n  regression:\n    enabled: false\nbenchmarks:\n  - name: demo\n    command: \"sh b.sh\"\n    result_type: composite\n",
        );
        self.eval()
    }
}

fn verdict(stdout: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    v["overall_verdict"].as_str().unwrap().to_string()
}

const PASSING: &str = "test result: ok. 10 passed; 0 failed; 0 ignored";
const NINETY: &str = "test result: FAILED. 9 passed; 1 failed; 0 ignored";

#[test]
fn nonzero_exit_fails_even_with_passing_summary() {
    let (code, out, _) = Env::new().correctness(PASSING, 1, "1.0");
    assert_eq!(verdict(&out), "FAIL");
    assert_eq!(code, 1);
}

#[test]
fn below_min_pass_rate_fails() {
    let (code, out, _) = Env::new().correctness(NINETY, 0, "1.0");
    assert_eq!(verdict(&out), "FAIL");
    assert_eq!(code, 1);
}

#[test]
fn min_pass_rate_is_honored() {
    let (code, out, _) = Env::new().correctness(NINETY, 0, "0.8");
    assert_eq!(verdict(&out), "WARN");
    assert_eq!(code, 0);
    let (code, out, _) = Env::new().correctness(NINETY, 1, "0.8");
    assert_eq!(verdict(&out), "FAIL");
    assert_eq!(code, 1);
}

#[test]
fn clean_run_passes() {
    let (code, out, _) = Env::new().correctness(PASSING, 0, "1.0");
    assert_eq!(verdict(&out), "PASS");
    assert_eq!(code, 0);
}

#[test]
fn invalid_min_pass_rate_is_rejected() {
    for bad in ["1.5", "-0.1", ".nan", ".inf"] {
        let (code, _, err) = Env::new().correctness(PASSING, 0, bad);
        assert_eq!(code, 1, "{bad}");
        assert!(err.contains("min_pass_rate"), "{bad}: {err}");
    }
}

#[test]
fn composite_parses_pretty_printed_json() {
    let (code, out, _) = Env::new().composite(
        "printf 'log line\\n{\\n  \"composite\": 0.75,\\n  \"detail\": {\"a\": 1}\\n}\\n'\n",
    );
    assert_eq!(verdict(&out), "PASS");
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["dimensions"]["benchmark:demo"]["score"], 0.75);
}

#[test]
fn composite_without_valid_score_fails() {
    for body in [
        "echo done\n",
        "echo '{\"other\": 1}'\n",
        "echo '{\"score\": \"high\"}'\n",
        "echo '{\"score\": 1.5}'\n",
        "echo '{\"score\": -0.2}'\n",
        "echo '{\"score\": 1.0}'\nexit 1\n",
    ] {
        let (code, out, _) = Env::new().composite(body);
        assert_eq!(verdict(&out), "FAIL", "{body}");
        assert_eq!(code, 1, "{body}");
    }
}
