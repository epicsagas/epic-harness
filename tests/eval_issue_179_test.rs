//! Eval commands run in disposable repositories/data roots; no user state is read.
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
    let Ok(scenario) = std::env::var("EPIC_EVAL_TEST_SCENARIO") else {
        return;
    };
    match scenario.as_str() {
        "composite-low" => println!("{{\"composite\":0.1}}"),
        "composite-high" => println!("{{\"composite\":0.95}}"),
        "composite-failure" => {
            println!("{{\"composite\":0.1}}");
            std::process::exit(7);
        }
        "latency-fast" => println!("{{\"latency_ms\":5}}"),
        "latency-slow" => println!("{{\"latency_ms\":250}}"),
        _ => println!("test result: ok. 10 passed; 0 failed;"),
    }
    std::process::exit(0);
}

#[test]
fn regression_selection_runs_measurements_and_rejects_empty_comparisons() {
    let f = Fixture::new();
    f.config(&format!(
        "benchmarks:\n  - name: fixture\n    command: {}\n    result_type: composite\n",
        serde_json::to_string(&f.command()).unwrap()
    ));
    let baselines = f.eval_dir().join("baselines");
    fs::create_dir_all(&baselines).unwrap();
    fs::write(
        baselines.join("latest.json"),
        r#"{"dimensions":{"benchmark:fixture":{"score":0.9}}}"#,
    )
    .unwrap();
    let output = f.run(
        "composite-low",
        &["eval", "--dimension", "regression", "--json"],
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(report["dimensions"]["benchmark:fixture"]["score"], 0.1);
    assert_eq!(
        report["dimensions"]["regression"]["details"]["deltas"]["benchmark:fixture"]["regressed"],
        true
    );
    assert!(
        f.run(
            "composite-high",
            &["eval", "--dimension", "regression", "--json"]
        )
        .status
        .success(),
        "a measured non-regression must pass"
    );

    fs::write(
        baselines.join("latest.json"),
        r#"{"dimensions":{"unrelated":{"score":0.9}}}"#,
    )
    .unwrap();
    let incomparable = f.run("composite-low", &["eval", "--json"]);
    assert!(
        !incomparable.status.success(),
        "an existing baseline without comparable inputs must fail"
    );

    f.config("dimensions:\n  regression:\n    enabled: true\n");
    let empty = f.run("pass", &["eval", "--json"]);
    assert!(
        !empty.status.success(),
        "no measurement must not be a successful check"
    );
}

#[test]
fn skipped_results_are_not_comparable_measurements() {
    for previous in [
        serde_json::json!({"quality":{"score":0,"verdict":"SKIPPED"}}),
        serde_json::json!({"quality":{"score":0,"verdict":"PASS"}}),
        serde_json::json!({"benchmark:fixture":{"score":0,"verdict":"SKIPPED"}}),
    ] {
        let f = Fixture::new();
        f.config(&format!("benchmarks:\n  - name: fixture\n    command: {}\n    result_type: composite\ndimensions:\n  quality:\n    enabled: true\n  regression:\n    enabled: true\n", serde_json::to_string(&f.command()).unwrap()));
        let baselines = f.eval_dir().join("baselines");
        fs::create_dir_all(&baselines).unwrap();
        fs::write(
            baselines.join("latest.json"),
            serde_json::to_vec(&serde_json::json!({"dimensions":previous})).unwrap(),
        )
        .unwrap();
        let output = f.run("composite-high", &["eval", "--json"]);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["dimensions"]["quality"]["verdict"], "SKIPPED");
        assert!(
            !output.status.success(),
            "skipped inputs are not measurement evidence: {report}"
        );
        assert_eq!(
            report["dimensions"]["regression"]["details"]["deltas"],
            serde_json::json!({})
        );
    }
}
