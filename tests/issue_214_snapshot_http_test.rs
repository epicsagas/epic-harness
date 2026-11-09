use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Dashboard {
    _temp: tempfile::TempDir,
    data: PathBuf,
    child: Child,
    port: u16,
}

impl Dashboard {
    fn start() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let data = root.join("data");
        let home = root.join("home");
        let cwd = root.join("project-a");
        for dir in [
            &data,
            &cwd,
            &home.join(".config/epic-harness"),
            &data.join("projects/project-a"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(["serve", &format!("--port={port}")])
            .current_dir(&cwd)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_ROOT", &root)
            .env("HARNESS_DIR", &data)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let dashboard = Self {
            _temp: temp,
            data,
            child,
            port,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < deadline, "fixture dashboard did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        dashboard
    }

    fn request(&self, method: &str, path: &str) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        let value = serde_json::from_str(body).unwrap_or_else(|_| json!({"raw": body}));
        (status, value)
    }

    fn file(&self, relative: &str, value: &Value) -> PathBuf {
        let file = self.data.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
        file
    }

    fn seed_snapshot(&self, project: &str, summary: &str, millis: i64) {
        let _ = self.request("GET", "/api/harness?cmd=get_orbit_pipelines");
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            sqlx::any::install_default_drivers();
            let pool = sqlx::AnyPool::connect(&format!(
                "sqlite:{}",
                self.data.join("harness.db").display()
            ))
            .await
            .unwrap();
            epic_harness::store::sessions::insert_snapshot_pool(
                &pool,
                &epic_harness::shared::types::SessionSnapshot {
                    timestamp: format!("2026-11-09T00:00:0{millis}Z"),
                    snap_type: "pre-compact".into(),
                    summary: summary.into(),
                    pending_tasks: vec!["pending synthetic task".into()],
                    context_usage: Some(0.4),
                    pipeline_state: None,
                },
                millis,
                project,
            )
            .await
            .unwrap();
            pool.close().await;
        });
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn issue_214_snapshot_api_returns_scoped_database_snapshot_objects() {
    let dashboard = Dashboard::start();
    dashboard.seed_snapshot("project-a", "local snapshot", 1);
    dashboard.seed_snapshot("project-b", "foreign snapshot", 2);
    let (status, body) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_session_snapshots&project=project-a",
    );
    assert_eq!(status, 200);
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["summary"], "local snapshot");
    assert_eq!(body[0]["type"], "pre-compact");
    assert_eq!(body[0]["context_usage"], 0.4);
    assert_eq!(body[0]["pending_tasks"][0], "pending synthetic task");
    assert!(body[0]["timestamp"].is_string());
    let (_, all) = dashboard.request("GET", "/api/harness?cmd=get_session_snapshots");
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[test]
fn issue_214_snapshot_api_parses_legacy_files_instead_of_returning_filenames() {
    let dashboard = Dashboard::start();
    dashboard.file("projects/project-a/sessions/snapshot-old.json", &json!({"timestamp":"2026-11-08T00:00:00Z","type":"pre-compact","summary":"legacy local","pending_tasks":[],"context_usage":0.2}));
    dashboard.file("projects/project-b/sessions/snapshot-other.json", &json!({"timestamp":"2026-11-09T00:00:00Z","type":"pre-compact","summary":"legacy foreign","pending_tasks":[],"context_usage":0.3}));
    let (_, body) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_session_snapshots&project=project-a",
    );
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["summary"], "legacy local");
    assert_eq!(body[0]["type"], "pre-compact");
    assert!(body[0]["timestamp"].is_string());
    assert_eq!(
        dashboard
            .request("GET", "/api/harness?cmd=get_session_snapshots")
            .1
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn issue_214_duplicate_snapshot_copies_preserve_distinct_projects() {
    let dashboard = Dashboard::start();
    dashboard.seed_snapshot("project-a", "same summary", 1);
    let snapshot = json!({"timestamp":"2026-11-09T00:00:01Z","type":"pre-compact","summary":"same summary","pending_tasks":["pending synthetic task"],"context_usage":0.4});
    dashboard.file("projects/project-a/sessions/copied.json", &snapshot);
    dashboard.file("projects/project-b/sessions/other-project.json", &snapshot);
    let (_, body) = dashboard.request("GET", "/api/harness?cmd=get_session_snapshots");
    let rows = body.as_array().unwrap();
    assert_eq!(
        rows.len(),
        2,
        "deduplicate the same project's file/DB copy, never another project's identical snapshot"
    );
    let projects: std::collections::HashSet<_> = rows
        .iter()
        .filter_map(|row| row["project"].as_str())
        .collect();
    assert_eq!(
        projects,
        std::collections::HashSet::from(["project-a", "project-b"])
    );
}
