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
fn init_and_scaffold_json_modes_emit_one_valid_document() {
    let f = Fixture::new();
    let init = f.run("pass", &["eval", "--init", "--json"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&init.stdout).unwrap_or_else(|e| {
        panic!(
            "init invalid JSON: {e}: {}",
            String::from_utf8_lossy(&init.stdout)
        )
    });
    assert_eq!(value["action"], "init");
    assert!(PathBuf::from(value["path"].as_str().unwrap()).exists());
    fs::write(f.cwd.join("package.json"), "{}").unwrap();
    for _ in 0..2 {
        let scaffold = f.run("pass", &["eval", "--scaffold", "--json"]);
        assert!(scaffold.status.success());
        let value: serde_json::Value =
            serde_json::from_slice(&scaffold.stdout).unwrap_or_else(|e| {
                panic!(
                    "scaffold invalid JSON: {e}: {}",
                    String::from_utf8_lossy(&scaffold.stdout)
                )
            });
        assert_eq!(value["action"], "scaffold");
    }
}
