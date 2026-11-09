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

    fn seed_pipeline(&self, id: &str, project: &str, state: &str) {
        let _ = self.request("GET", "/api/harness?cmd=get_orbit_pipelines");
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            sqlx::any::install_default_drivers();
            let pool = sqlx::AnyPool::connect(&format!(
                "sqlite:{}",
                self.data.join("harness.db").display()
            ))
            .await
            .unwrap();
            epic_harness::store::orbit_store::upsert_pipeline_pool(
                &pool,
                id,
                project,
                "complete",
                Some("evolve"),
                Some("direct"),
                state,
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
fn issue_279_mixed_database_and_legacy_pipeline_lists_preserve_selected_projects() {
    let dashboard = Dashboard::start();
    dashboard.file(
        "projects/project-a/orbit/PIPELINE-legacy.json",
        &json!({"id":"legacy", "status":"running", "started_at":"2026-11-09T10:00:00Z"}),
    );
    let (_, before) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_orbit_pipelines&project=project-a",
    );
    assert_eq!(
        before[0]["id"], "legacy",
        "legacy-only control must be visible"
    );
    dashboard.seed_pipeline("database", "project-b", "{}");
    let (status, selected) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_orbit_pipelines&project=project-a",
    );
    assert_eq!(status, 200);
    assert_eq!(selected.as_array().unwrap().len(), 1);
    assert_eq!(selected[0]["id"], "legacy");
    let (_, all) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[test]
fn issue_279_primary_copies_win_without_hiding_foreign_same_id() {
    let dashboard = Dashboard::start();
    dashboard.seed_pipeline("shared-id", "project-a", "{}");
    let primary_copy = dashboard.file(
        "projects/project-a/orbit/PIPELINE-shared-id.json",
        &json!({"id":"shared-id", "status":"running"}),
    );
    dashboard.file(
        "projects/project-b/orbit/PIPELINE-shared-id.json",
        &json!({"id":"shared-id", "status":"running"}),
    );
    let before = fs::read(&primary_copy).unwrap();
    let (_, body) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["_project"] == row["project"]));
    assert_eq!(
        rows.iter()
            .find(|row| row["project"] == "project-a")
            .unwrap()["status"],
        "complete"
    );
    assert_eq!(
        rows.iter()
            .find(|row| row["project"] == "project-b")
            .unwrap()["status"],
        "running"
    );
    assert_eq!(
        fs::read(primary_copy).unwrap(),
        before,
        "listing must not import/rewrite files"
    );
}

#[test]
fn issue_279_legacy_pipeline_fallback_survives_unavailable_database() {
    let dashboard = Dashboard::start();
    fs::create_dir_all(dashboard.data.join("harness.db")).unwrap();
    dashboard.file(
        "projects/project-a/orbit/PIPELINE-legacy.json",
        &json!({"id":"legacy", "status":"running"}),
    );
    let (status, rows) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_orbit_pipelines&project=project-a",
    );
    assert_eq!(status, 200);
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["id"], "legacy");
}
