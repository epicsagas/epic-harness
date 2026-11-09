//! Source-built CLI regression in disposable project/data roots.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    data: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join("fixture-project");
        let data = root.join("data");
        for dir in [&cwd, &data, &root.join("home/.config/epic-harness")] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(
            root.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
        fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            cwd,
            data,
        }
    }

    fn eval_dir(&self) -> PathBuf {
        self.data.join("projects/fixture-project/eval")
    }

    fn command(&self) -> String {
        format!(
            "\"{}\" --exact fixture_command --nocapture",
            std::env::current_exe().unwrap().display()
        )
    }

    fn config(&self, yaml: &str) {
        fs::create_dir_all(self.eval_dir()).unwrap();
        fs::write(self.eval_dir().join("eval.yaml"), yaml).unwrap();
    }

    fn run(&self, scenario: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("home/.config"))
            .env("EPIC_EVAL_TEST_SCENARIO", scenario)
            .env("PATH", "")
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .output()
            .unwrap()
    }
}

#[test]
fn fixture_command() {
    let Ok(_scenario) = std::env::var("EPIC_EVAL_TEST_SCENARIO") else {
        return;
    };
    println!("test result: ok. 10 passed; 0 failed;");
    std::process::exit(0);
}

#[test]
fn native_timestamps_preserve_each_report_and_archived_baseline() {
    let f = Fixture::new();
    f.config(&format!("benchmarks:\n  - name: fixture\n    command: {}\ndimensions:\n  regression:\n    enabled: false\n", serde_json::to_string(&f.command()).unwrap()));
    let mut timestamps = Vec::new();
    for _ in 0..2 {
        let output = f.run("pass", &["eval", "--baseline-update", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let timestamp = report["timestamp"].as_str().unwrap().to_string();
        assert!(
            timestamp.starts_with("20"),
            "native UTC timestamp: {timestamp}"
        );
        timestamps.push(timestamp);
    }
    assert_ne!(timestamps[0], timestamps[1]);
    let reports = fs::read_dir(f.cwd.join("benchmarks/results"))
        .unwrap()
        .count();
    let baselines = fs::read_dir(f.eval_dir().join("baselines"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("BASELINE-"))
        .count();
    assert_eq!(reports, 2);
    assert_eq!(baselines, 2);
}
