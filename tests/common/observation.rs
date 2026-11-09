#![allow(dead_code)]

use sqlx::sqlite::SqliteRow;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct Fixture {
    _temp: tempfile::TempDir,
    pub root: PathBuf,
    pub data: PathBuf,
    pub home: PathBuf,
    pub cwd: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let data = root.join("data");
        let home = root.join("home");
        let cwd = root.join("fixture-project");
        for dir in [
            &data,
            &home,
            &cwd,
            &home.join(".config/epic-harness"),
            &data.join("projects/fixture-project"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        Self {
            _temp: temp,
            root,
            data,
            home,
            cwd,
        }
    }

    pub fn observe(&self, payload: serde_json::Value) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        command
            .arg("observe")
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", &self.root)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("EPIC_HOOK_PROFILE", "standard")
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&payload).unwrap().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "observe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub async fn rows(&self) -> Vec<SqliteRow> {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            self.data.join("harness.db").display()
        ))
        .await
        .unwrap();
        let rows = sqlx::query("SELECT * FROM observations ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        pool.close().await;
        rows
    }
}
