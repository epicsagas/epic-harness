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

}

impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn issue_221_empty_pipeline_id_rejects_without_deleting_other_projects() {
    let dashboard = Dashboard::start();
    let a = dashboard.file(
        "projects/project-a/orbit/PIPELINE-alpha.json",
        &json!({"id":"alpha"}),
    );
    let b = dashboard.file(
        "projects/project-b/orbit/PIPELINE-beta.json",
        &json!({"id":"beta"}),
    );
    let (status, body) = dashboard.request("DELETE", "/api/orbit/");
    assert_eq!(status, 400);
    assert_eq!(body["ok"], false);
    assert!(a.exists() && b.exists());
}

#[test]
fn issue_221_pipeline_id_matches_exactly_and_honors_selected_project() {
    let dashboard = Dashboard::start();
    let selected = dashboard.file(
        "projects/project-a/orbit/PIPELINE-alpha.json",
        &json!({"id":"alpha"}),
    );
    let prefix = dashboard.file(
        "projects/project-a/orbit/PIPELINE-alpha-extra.json",
        &json!({"id":"alpha-extra"}),
    );
    let other_project = dashboard.file(
        "projects/project-b/orbit/PIPELINE-alpha.json",
        &json!({"id":"alpha"}),
    );
    let (status, body) = dashboard.request("DELETE", "/api/orbit/alpha?project=project-a");
    assert_eq!(status, 200);
    assert_eq!(body["ok"], true);
    assert!(!selected.exists());
    assert!(prefix.exists() && other_project.exists());
}
