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
fn snapshot_diff_reports_positional_scoring_weight_changes() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut after = before.clone();
    after["config_summary"]["scoring_weights"] = serde_json::json!([0.3, 0.5, 0.2]);
    let a = f.root.join("before.json");
    let b = f.root.join("after.json");
    fs::write(&a, serde_json::to_vec(&before).unwrap()).unwrap();
    fs::write(&b, serde_json::to_vec(&after).unwrap()).unwrap();
    let output = f.run(&["harness", "diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(output.status.success());
    let diff = String::from_utf8(output.stdout).unwrap();
    assert!(diff.contains("config_summary.scoring_weights"), "{diff}");
    let identical = f.run(&["harness", "diff", a.to_str().unwrap(), a.to_str().unwrap()]);
    assert!(
        String::from_utf8(identical.stdout)
            .unwrap()
            .contains("no differences")
    );
}
