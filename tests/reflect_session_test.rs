//! Session-end `reflect` over observation fallback files, through the real
//! binary. Every run uses a temp HOME, HARNESS_DIR and cwd; records are synthetic.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
const HOST_ID: &str = "11111111-2222-4333-8444-555555555555";

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
        fs::create_dir_all(harness.join("projects").join(SLUG).join("obs")).unwrap();
        Self {
            _temp: temp,
            cwd,
            home,
            harness,
        }
    }

    fn project_dir(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG)
    }

    fn db(&self) -> PathBuf {
        self.harness.join("harness.db")
    }

    fn run(&self, args: &[&str], stdin: &str) -> (i32, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
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
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    /// One successful Read observation under the host session id.
    fn observe(&self, file: &str) {
        let input = serde_json::json!({
            "session_id": HOST_ID,
            "tool_name": "Read",
            "tool_input": {"file_path": file},
            "tool_response": "synthetic file body",
        });
        let (code, _) = self.run(&["observe"], &input.to_string());
        assert_eq!(code, 0);
    }

    fn fallback_file(&self) -> PathBuf {
        self.project_dir()
            .join("obs")
            .join(format!("session_{HOST_ID}.jsonl"))
    }

    /// Record three observations while SQLite is unavailable (an empty directory
    /// sits where the database file belongs), then clear the obstruction.
    fn record_fallback(&self) {
        fs::create_dir(self.db()).unwrap();
        for f in ["src/a.rs", "src/b.rs", "src/c.rs"] {
            self.observe(f);
        }
        assert!(self.fallback_file().is_file(), "observe wrote no fallback");
    }

    fn context(&self) -> serde_json::Value {
        let (code, out) = self.run(&["reflect", "--context"], "");
        assert_eq!(code, 0);
        serde_json::from_str(&out).unwrap()
    }
}

#[test]
fn host_session_fallback_reaches_reflection_once_sqlite_is_back() {
    let env = Env::new();
    env.record_fallback();
    fs::remove_dir(env.db()).unwrap();

    let (code, _) = env.run(&["reflect"], "{}");
    assert_eq!(code, 0);

    assert!(!env.fallback_file().exists(), "fallback was not backfilled");
    let ctx = env.context();
    assert_eq!(ctx["obs_stats"]["total"], 3, "{ctx}");
    assert_eq!(ctx["metrics_summary"]["total_sessions"], 1, "{ctx}");
}

#[test]
fn host_session_fallback_is_read_while_sqlite_stays_down() {
    let env = Env::new();
    env.record_fallback();

    let (code, _) = env.run(&["reflect"], "{}");
    assert_eq!(code, 0);

    // Nothing could be imported, so the file stays for the next successful run,
    // and the round was analyzed from it.
    assert!(env.fallback_file().exists());
    assert!(env.project_dir().join("metrics.json").is_file());
}

#[test]
fn old_records_in_a_fallback_file_are_not_counted_as_today() {
    let env = Env::new();
    fs::create_dir(env.db()).unwrap();
    let line = |n: u32| {
        serde_json::json!({
            "timestamp": format!("2020-01-01T00:00:0{n}Z"),
            "tool": "Read",
            "tool_category": "read",
            "action": format!("src/f{n}.rs"),
            "result": "success",
            "score": 1.0,
        })
        .to_string()
    };
    let body: Vec<String> = (0..3).map(line).collect();
    fs::write(env.fallback_file(), body.join("\n") + "\n").unwrap();

    let (code, _) = env.run(&["reflect"], "{}");
    assert_eq!(code, 0);

    // No record is from today, so there is nothing to analyze.
    assert!(!env.project_dir().join("metrics.json").exists());
}
