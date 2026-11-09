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
        let cwd = root.join("snapshot-project");
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

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("home/.config"))
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .output()
            .unwrap()
    }

    fn snapshot(&self) -> serde_json::Value {
        let output = self.run(&["harness", "snapshot"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[test]
fn snapshot_distinguishes_blocked_and_warned_rule_sections() {
    let f = Fixture::new();
    fs::create_dir_all(f.cwd.join(".harness")).unwrap();
    let rules = f.cwd.join(".harness/guard-rules.yaml");
    fs::write(
        &rules,
        "blocked:\n  - pattern: synthetic-command | msg: synthetic rule\nwarned:\n",
    )
    .unwrap();
    let blocked = f.snapshot();
    fs::write(
        &rules,
        "blocked:\nwarned:\n  - pattern: synthetic-command | msg: synthetic rule\n",
    )
    .unwrap();
    let warned = f.snapshot();
    assert_ne!(blocked["guard_rules"], warned["guard_rules"]);
    assert_ne!(blocked["hash"], warned["hash"]);
}
