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
fn local_team_sync_respects_scope_and_global_is_explicit() {
    let fixture = Fixture::new();
    let canonical = fixture.seed_team("alpha", "backend");
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    let host_roots = [
        ".codex",
        ".gemini/config/plugins/epic",
        ".cursor",
        ".config/opencode",
    ];
    for host in host_roots {
        let root = fixture.home.join(host);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("unrelated.txt"), "UNRELATED_HOST_CONFIG").unwrap();
    }

    for _ in 0..2 {
        fixture.success(&["sync", "backend", "--org", "alpha"]);
    }
    assert!(
        fixture
            .project
            .join(".claude/agents/backend/tester.md")
            .is_file()
    );
    let local_codex = fixture.project.join(".codex/agents/backend/tester.toml");
    let rendered: toml::Value = toml::from_str(&read(&local_codex)).unwrap();
    assert!(
        rendered["developer_instructions"]
            .as_str()
            .unwrap()
            .contains("PRESERVED_CUSTOM_AGENT")
    );
    for host in host_roots {
        let root = fixture.home.join(host);
        assert!(
            !root.join("agents/backend").exists(),
            "project sync must not install home agents for {host}"
        );
        assert_eq!(read(&root.join("unrelated.txt")), "UNRELATED_HOST_CONFIG");
    }
    fixture.success(&["unlink", "backend", "--org", "alpha"]);
    assert!(!fixture.project.join(".claude/agents/backend").exists());
    assert!(!fixture.project.join(".codex/agents/backend").exists());
    assert!(canonical.join("agents/tester.md").is_file());

    fixture.success(&["sync", "backend", "--org", "alpha", "--global"]);
    assert!(
        fixture
            .home
            .join(".claude/agents/backend/tester.md")
            .is_file()
    );
    assert!(
        fixture
            .home
            .join(".codex/agents/backend/tester.toml")
            .is_file()
    );
    for host in host_roots.into_iter().skip(1) {
        assert!(
            fixture
                .home
                .join(host)
                .join("agents/backend/tester.md")
                .is_file()
        );
    }
    assert!(
        !local_codex.exists(),
        "global sync must not recreate project agents"
    );
}
