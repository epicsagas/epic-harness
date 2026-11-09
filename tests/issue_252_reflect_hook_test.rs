use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Fixture {
    temp: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    work: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let data = temp.path().join("data");
        let work = temp.path().join("issue-project");
        let project = data.join("projects/issue-project");
        for directory in [&work, &project, &home.join(".config/epic-harness")] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        std::fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        Self {
            temp,
            home,
            data,
            work,
            project,
        }
    }

    fn call(&self, args: &[&str]) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&self.work)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", self.temp.path())
            .env("EPIC_HOOK_PROFILE", "standard")
            .env("EPIC_ORCHESTRATION", "disabled")
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"hook_event_name\":\"Stop\"}")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn assert_observation_count(&self, expected: i64) {
        use sqlx::Connection;
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let options =
                sqlx::sqlite::SqliteConnectOptions::new().filename(self.data.join("harness.db"));
            let mut connection = sqlx::SqliteConnection::connect_with(&options)
                .await
                .unwrap();
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM observations")
                .fetch_one(&mut connection)
                .await
                .unwrap();
            assert_eq!(
                count, expected,
                "outside fallback data must not enter SQLite"
            );
        });
    }
}

fn link(target: &Path, path: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, path).unwrap();
    #[cfg(windows)]
    {
        let out = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(path.to_string_lossy().replace('/', "\\"))
            .arg(target)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn observations(directory: &Path) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let source = directory.join(format!(
        "session_{}_boundary.jsonl",
        epic_harness::shared::helpers::today()
    ));
    let records: String = (0..3).map(|index| serde_json::json!({
        "timestamp":epic_harness::shared::helpers::now_iso(), "tool":"BoundarySyntheticTool", "tool_category":"other",
        "action":format!("boundary-{index}"),"result":"success","score":1.0
    }).to_string()+"\n").collect();
    std::fs::write(&source, records).unwrap();
    source
}

#[test]
fn ordinary_reflect_rejects_escaping_obs_before_reading_or_deleting() {
    let f = Fixture::new();
    f.call(&["reflect", "--context"]);
    let outside = f.home.join("outside");
    let source = observations(&outside);
    link(&outside, &f.project.join("obs"));
    f.call(&["reflect"]);
    assert!(source.exists(), "outside original must remain intact");
    assert!(!f.project.join("metrics.json").exists());
    f.assert_observation_count(0);
}

#[test]
fn ordinary_reflect_rejects_escaping_project_before_state_changes() {
    let f = Fixture::new();
    let outside = f.home.join("outside-project");
    let source = observations(&outside.join("obs"));
    std::fs::remove_dir(&f.project).unwrap();
    link(&outside, &f.project);
    f.call(&["reflect"]);
    assert!(source.exists());
    assert!(!outside.join("metrics.json").exists());
    assert!(!outside.join("reflect_watermark.txt").exists());
}

#[test]
fn ordinary_reflect_still_imports_contained_fallback() {
    let f = Fixture::new();
    f.call(&["reflect", "--context"]);
    let source = observations(&f.project.join("obs"));
    f.call(&["reflect"]);
    assert!(
        !source.exists(),
        "contained fallback must still be backfilled"
    );
    f.assert_observation_count(3);
}

#[cfg(unix)]
#[test]
fn ordinary_reflect_skips_escaping_individual_fallback() {
    let f = Fixture::new();
    f.call(&["reflect", "--context"]);
    let outside = observations(&f.home.join("outside"));
    std::fs::create_dir(f.project.join("obs")).unwrap();
    std::os::unix::fs::symlink(
        &outside,
        f.project.join("obs").join(outside.file_name().unwrap()),
    )
    .unwrap();
    f.call(&["reflect"]);
    assert!(outside.exists());
    f.assert_observation_count(0);
}
