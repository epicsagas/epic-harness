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
        "composite-failure" => {
            println!("{{\"composite\":0.1}}");
            std::process::exit(7);
        }
        "latency-fast" => println!("{{\"latency_ms\":5}}"),
        "latency-threshold" => println!("{{\"latency_ms\":6}}"),
        "latency-slow" => println!("{{\"latency_ms\":250}}"),
        "latency-invalid" => println!("{{\"latency_ms\":\"invalid\"}}"),
        "throughput-base" => println!("{{\"throughput\":100}}"),
        "throughput-improved" => println!("{{\"throughput\":120}}"),
        "throughput-threshold" => println!("{{\"throughput\":80}}"),
        "throughput-slow" => println!("{{\"throughput\":79}}"),
        _ => println!("test result: ok. 10 passed; 0 failed;"),
    }
    std::process::exit(0);
}

#[test]
fn measured_performance_regressions_honor_the_configured_percentage() {
    let f = Fixture::new();
    f.config(&format!("dimensions:\n  performance:\n    enabled: true\n    command: {}\n    extra:\n      max_regression_pct: 20\n  regression:\n    enabled: true\n", serde_json::to_string(&f.command()).unwrap()));
    let baseline = f.run("latency-fast", &["eval", "--baseline-update", "--json"]);
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );
    let slow = f.run("latency-slow", &["eval", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&slow.stdout).unwrap();
    assert!(!slow.status.success(), "slow measurement passed: {report}");
    assert_eq!(
        report["dimensions"]["regression"]["details"]["deltas"]["performance"]["regressed"],
        true
    );
    assert_eq!(
        report["dimensions"]["performance"]["details"]["metric"],
        "latency_ms"
    );
}

#[test]
fn performance_direction_threshold_and_invalid_measurements_have_controls() {
    for (baseline_scenario, controls) in [
        (
            "latency-fast",
            vec![
                ("latency-threshold", true),
                ("latency-slow", false),
                ("latency-invalid", false),
            ],
        ),
        (
            "throughput-base",
            vec![
                ("throughput-improved", true),
                ("throughput-threshold", true),
                ("throughput-slow", false),
            ],
        ),
    ] {
        let f = Fixture::new();
        f.config(&format!("dimensions:\n  performance:\n    enabled: true\n    command: {}\n    extra:\n      max_regression_pct: 20\n  regression:\n    enabled: true\n", serde_json::to_string(&f.command()).unwrap()));
        let baseline = f.run(baseline_scenario, &["eval", "--baseline-update", "--json"]);
        assert!(
            baseline.status.success(),
            "{}",
            String::from_utf8_lossy(&baseline.stderr)
        );
        for (scenario, expected_success) in controls {
            let output = f.run(scenario, &["eval", "--json"]);
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                output.status.success(),
                expected_success,
                "scenario {scenario}: {report}"
            );
        }
        assert!(
            !f.run("composite-failure", &["eval", "--json"])
                .status
                .success(),
            "child failure must fail independently of comparison"
        );
    }
}

#[test]
fn explicit_performance_metric_changes_cannot_use_legacy_wall_duration() {
    for (previous_details, expected_success) in [
        (
            serde_json::json!({"metric":"latency_ms", "metric_value":5}),
            false,
        ),
        (
            serde_json::json!({"metric":"throughput", "metric_value":5}),
            false,
        ),
        (serde_json::json!({"metric":42, "metric_value":5}), false),
        (serde_json::json!({}), true),
    ] {
        let f = Fixture::new();
        f.config(&format!("dimensions:\n  performance:\n    enabled: true\n    command: {}\n    extra:\n      max_regression_pct: 20\n  regression:\n    enabled: true\n", serde_json::to_string(&f.command()).unwrap()));
        let baselines = f.eval_dir().join("baselines");
        fs::create_dir_all(&baselines).unwrap();
        let baseline = serde_json::json!({"dimensions":{"performance":{
            "score":1, "duration_ms":1000000, "details":previous_details
        }}});
        fs::write(
            baselines.join("latest.json"),
            serde_json::to_vec(&baseline).unwrap(),
        )
        .unwrap();
        let output = f.run("wall-duration", &["eval", "--json"]);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["dimensions"]["performance"]["details"]["metric"],
            "duration_ms"
        );
        assert_eq!(output.status.success(), expected_success, "{report}");
        assert_eq!(
            report["dimensions"]["regression"]["details"]["deltas"]["performance"]["comparison_error"],
            !expected_success
        );
    }
}
