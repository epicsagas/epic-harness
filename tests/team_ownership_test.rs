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
fn unlink_rejects_other_org_before_removing_any_local_mirror() {
    let fixture = Fixture::new();
    let alpha = fixture.seed_team("alpha", "backend");
    let beta = fixture.seed_team("beta", "backend");
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    fixture.success(&["sync", "backend", "--org", "alpha"]);
    let claude = fixture.project.join(".claude/agents/backend/tester.md");
    let codex = fixture.project.join(".codex/agents/backend/tester.toml");
    let before = [
        read(&claude),
        read(&codex),
        read(&alpha.join("config.json")),
        read(&beta.join("config.json")),
    ];

    for operation in ["unlink", "delete"] {
        let result = fixture.run(&[operation, "backend", "--org", "beta"]);
        assert!(
            !result.status.success(),
            "{operation} must reject another org's local team"
        );
        assert_eq!(read(&claude), before[0]);
        assert_eq!(read(&codex), before[1]);
        assert_eq!(read(&alpha.join("config.json")), before[2]);
        assert_eq!(read(&beta.join("config.json")), before[3]);
    }
    fixture.success(&["unlink", "backend", "--org", "alpha"]);
    let config: serde_json::Value =
        serde_json::from_str(&read(&alpha.join("config.json"))).unwrap();
    assert_eq!(config["projects"], serde_json::json!([]));
    assert!(alpha.join("agents/tester.md").exists());
    assert_eq!(read(&beta.join("config.json")), before[3]);
}

#[test]
fn unlink_infers_org_from_codex_only_mirror() {
    let fixture = Fixture::new();
    let canonical = fixture.seed_team("alpha", "backend");
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    fixture.success(&["sync", "backend", "--org", "alpha"]);
    let claude = fixture.project.join(".claude/agents/backend");
    assert!(
        claude
            .canonicalize()
            .unwrap()
            .starts_with(fixture.project.canonicalize().unwrap())
    );
    fs::remove_dir_all(claude).unwrap();
    fixture.success(&["unlink", "backend"]);
    assert!(!fixture.project.join(".codex/agents/backend").exists());
    let config: serde_json::Value =
        serde_json::from_str(&read(&canonical.join("config.json"))).unwrap();
    assert_eq!(
        config["projects"],
        serde_json::json!([]),
        "inferred owner must be deregistered"
    );
}

#[test]
fn unlink_rejects_conflicting_mirror_owners() {
    let fixture = Fixture::new();
    fixture.seed_team("alpha", "backend");
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    fixture.success(&["sync", "backend", "--org", "alpha"]);
    let codex = fixture.project.join(".codex/agents/backend/tester.toml");
    fs::write(
        &codex,
        read(&codex).replace("org = \"alpha\"", "org = \"beta\""),
    )
    .unwrap();
    let result = fixture.run(&["unlink", "backend", "--org", "alpha"]);
    assert!(
        !result.status.success(),
        "all installed mirrors must have the requested owner"
    );
    assert!(
        fixture
            .project
            .join(".claude/agents/backend/tester.md")
            .exists()
    );
    assert!(codex.exists());
}
