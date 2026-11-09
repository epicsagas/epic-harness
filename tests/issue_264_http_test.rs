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
fn issue_264_dismiss_removes_primary_pipeline_and_sidecar() {
    let dashboard = Dashboard::start();
    dashboard.seed_pipeline("PIPELINE-persisted", "project-a", "{}");
    dashboard.seed_pipeline("PIPELINE-persisted-extra", "project-a", "{}");
    let sidecar = dashboard.file(
        "projects/project-a/orbit/PIPELINE-persisted.json",
        &json!({"id":"PIPELINE-persisted"}),
    );
    let (status, body) =
        dashboard.request("DELETE", "/api/orbit/PIPELINE-persisted?project=project-a");
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    assert!(!sidecar.exists());
    let (_, remaining) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    let rows = remaining.as_array().unwrap();
    assert_eq!(
        rows.len(),
        1,
        "successful dismissal must also remove the primary DB row"
    );
    assert_eq!(rows[0]["id"], "PIPELINE-persisted-extra");
}

#[test]
fn issue_264_database_only_pipeline_can_be_dismissed_with_project_scope() {
    let dashboard = Dashboard::start();
    dashboard.seed_pipeline("PIPELINE-db-only", "project-b", "{}");
    let (wrong_status, wrong_body) =
        dashboard.request("DELETE", "/api/orbit/PIPELINE-db-only?project=project-a");
    assert_eq!(wrong_status, 404);
    assert_eq!(wrong_body["ok"], false);
    let (_, before) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    assert_eq!(before.as_array().unwrap().len(), 1);
    let (status, body) =
        dashboard.request("DELETE", "/api/orbit/PIPELINE-db-only?project=project-b");
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    assert_eq!(
        dashboard
            .request("GET", "/api/harness?cmd=get_orbit_pipelines")
            .1,
        json!([])
    );
}
