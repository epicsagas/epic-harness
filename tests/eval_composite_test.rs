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
        "composite-pretty" => println!("{{\n  \"composite\": 0.1\n}}"),
        "composite-missing" => println!("{{\"samples\":3}}"),
        "composite-out-of-range" => println!("{{\"composite\":2}}"),
        _ => println!("{{\"composite\":0.9}}"),
    }
    std::process::exit(0);
}

#[test]
fn composite_preserves_multiline_scores_and_rejects_missing_or_invalid_scores() {
    let f = Fixture::new();
    f.config(&format!("benchmarks:\n  - name: fixture\n    command: {}\n    result_type: composite\ndimensions:\n  regression:\n    enabled: false\n", serde_json::to_string(&f.command()).unwrap()));
    let pretty = f.run("composite-pretty", &["eval", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&pretty.stdout).unwrap();
    assert!(pretty.status.success());
    assert_eq!(report["dimensions"]["benchmark:fixture"]["score"], 0.1);
    for scenario in ["composite-missing", "composite-out-of-range"] {
        let output = f.run(scenario, &["eval", "--json"]);
        assert!(
            !output.status.success(),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
