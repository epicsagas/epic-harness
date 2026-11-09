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
fn snapshot_never_creates_or_initializes_the_metrics_database() {
    let fresh = Fixture::new();
    let snapshot = fresh.snapshot();
    assert_eq!(snapshot["metrics_summary"]["total_sessions"], 0);
    assert!(
        !fresh.data.join("harness.db").exists(),
        "a read created harness.db"
    );

    let existing = Fixture::new();
    let database = existing.data.join("harness.db");
    sqlx::any::install_default_drivers();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::any::AnyPoolOptions::new().max_connections(1)
            .connect(&format!("sqlite:{}?mode=rwc", database.display())).await.unwrap();
        for ddl in [
            "CREATE TABLE metrics_state (key TEXT, value TEXT, project TEXT, PRIMARY KEY(key,project))",
            "CREATE TABLE score_history (id INTEGER PRIMARY KEY, timestamp TEXT, success_rate REAL, avg_score REAL, observations INTEGER, dim_success REAL, dim_quality REAL, dim_cost REAL, project TEXT)",
            "CREATE TABLE skill_attribution (skill_name TEXT, sessions_active INTEGER, avg_score_with REAL, avg_score_without REAL, first_seen TEXT, sessions_holdout INTEGER, project TEXT)",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        for (project, count) in [("snapshot-project", "7"), ("other-project", "13")] {
            sqlx::query("INSERT INTO metrics_state (key,value,project) VALUES ('total_sessions',?,?)")
                .bind(count).bind(project).execute(&pool).await.unwrap();
        }
        pool.close().await;
    });
    fs::write(
        existing.data.join("config.toml"),
        format!(
            "[dashboard]\nport=0\nauto_open=false\n[db]\nharness_url={}\n",
            serde_json::to_string(&format!("sqlite:{}?mode=rwc", database.display())).unwrap()
        ),
    )
    .unwrap();
    let before = fs::read(&database).unwrap();
    let snapshot = existing.snapshot();
    assert_eq!(snapshot["metrics_summary"]["total_sessions"], 7);
    assert_eq!(
        fs::read(&database).unwrap(),
        before,
        "a read modified database bytes"
    );
}
