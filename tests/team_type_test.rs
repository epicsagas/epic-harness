//! Team operations run in fresh processes so HOME and cwd cannot leak across tests.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let data = temp.path().join("data");
        let project = temp.path().join("fixture project");
        for dir in [&home, &data, &project] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        Self {
            _temp: temp,
            home,
            data,
            project,
        }
    }

    fn seed_team(&self, org: &str, name: &str) -> PathBuf {
        let dir = self
            .home
            .join(".harness/orgs")
            .join(org)
            .join("teams")
            .join(name);
        fs::create_dir_all(dir.join("agents")).unwrap();
        fs::write(
            dir.join("config.json"),
            serde_json::to_vec(&serde_json::json!({
                "name": name, "org": org, "type": "stream", "projects": [],
                "created": "2026-01-01T00:00:00Z", "updated": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(dir.join("mission.md"), "Synthetic test mission").unwrap();
        fs::write(
            dir.join("agents/tester.md"),
            "---\nname: tester\ndescription: Synthetic custom tester\n---\nPRESERVED_CUSTOM_AGENT\n",
        )
        .unwrap();
        dir
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("team")
            .args(args)
            .current_dir(&self.project)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", self._temp.path())
            .env("CLAUDE_CONFIG_DIR", self.home.join(".claude"))
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("APPDATA", self.home.join("AppData/Roaming"))
            .env("LOCALAPPDATA", self.home.join("AppData/Local"))
            .env_remove("HARNESS_MEM_URL")
            .env_remove("HARNESS_DB_URL")
            .env_remove("DATABASE_URL")
            .env_remove("EPIC_ORG")
            .env_remove("EPIC_AGENT_ID")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn success(&self, args: &[&str]) {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "team {args:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn updating_team_type_persists_selected_type_and_synced_context() {
    let fixture = Fixture::new();
    let canonical = fixture.seed_team("alpha", "backend");
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    let custom = read(&canonical.join("agents/tester.md"));

    for team_type in ["platform", "platform", "subsystem"] {
        fixture.success(&[
            "--yes",
            "--org",
            "alpha",
            "--name",
            "backend",
            "--type",
            team_type,
            "--mission",
            "Synthetic updated mission",
        ]);
        let config: serde_json::Value =
            serde_json::from_str(&read(&canonical.join("config.json"))).unwrap();
        assert_eq!(
            config["type"], team_type,
            "selected type must persist before sync"
        );
        assert_eq!(config["projects"].as_array().unwrap().len(), 1);
        assert_eq!(
            read(&canonical.join("agents/tester.md")),
            custom,
            "custom agents must survive append/merge"
        );
        let label = if team_type == "platform" {
            "Platform"
        } else {
            "Subsystem"
        };
        for entry in fs::read_dir(fixture.project.join(".claude/agents/backend")).unwrap() {
            let content = read(&entry.unwrap().path());
            assert!(content.contains(&format!("**Team**: backend ({label})")));
            assert!(content.contains("Synthetic updated mission"));
            assert!(!content.contains("Stream-aligned"));
        }
        for entry in fs::read_dir(fixture.project.join(".codex/agents/backend")).unwrap() {
            let content: toml::Value = toml::from_str(&read(&entry.unwrap().path())).unwrap();
            assert!(
                content["developer_instructions"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("**Team**: backend ({label})"))
            );
        }
    }
    assert!(canonical.join("agents/api-designer.md").exists());
    assert!(canonical.join("agents/domain-specialist.md").exists());
    assert!(canonical.join("agents/tester.md").exists());
}
