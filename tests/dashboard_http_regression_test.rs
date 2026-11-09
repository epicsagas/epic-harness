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
fn issue_222_nonobject_pipeline_files_do_not_terminate_dashboard() {
    let dashboard = Dashboard::start();
    dashboard.file("projects/project-a/orbit/PIPELINE-invalid.json", &json!([]));
    dashboard.file(
        "projects/project-a/orbit/PIPELINE-valid.json",
        &json!({"id":"valid", "status":"complete"}),
    );
    let (status, body) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    assert_eq!(
        status, 200,
        "non-object sidecar must be skipped, not crash the server"
    );
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["id"], "valid");
    assert_eq!(dashboard.request("GET", "/api/projects").0, 200);
}

#[test]
fn issue_222_nonobject_pipeline_database_rows_do_not_terminate_dashboard() {
    let dashboard = Dashboard::start();
    dashboard.seed_pipeline("invalid", "project-a", "[]");
    dashboard.seed_pipeline("valid", "project-a", "{}");
    let (status, body) = dashboard.request("GET", "/api/harness?cmd=get_orbit_pipelines");
    assert_eq!(status, 200, "non-object database state must be skipped");
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["id"], "valid");
    assert_eq!(dashboard.request("GET", "/api/projects").0, 200);
}
