use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Dashboard {
    _temp: tempfile::TempDir,
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

    fn seed_memory(&self, node_type: &str, title: &str) {
        let _ = self.request("GET", "/api/search?q=initialize");
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            sqlx::any::install_default_drivers();
            let pool = sqlx::AnyPool::connect(&format!(
                "sqlite:{}",
                self._temp.path().join(".harness/memory.db").display()
            ))
            .await
            .unwrap();
            epic_harness::mem::store::write_node_pool(
                &pool,
                &epic_harness::mem::store::Node {
                    frontmatter: epic_harness::mem::store::NodeFrontmatter {
                        id: uuid::Uuid::new_v4().to_string(),
                        node_type: node_type.into(),
                        title: title.into(),
                        tags: vec!["fixture".into()],
                        projects: vec!["project-a".into()],
                        agents: vec!["synthetic-agent".into()],
                        created: "2026-11-09T00:00:00Z".into(),
                        updated: "2026-11-09T00:00:00Z".into(),
                        importance: 0.8,
                        ..Default::default()
                    },
                    body: "Zebrafixture body".into(),
                },
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
fn issue_268_memory_search_filters_type_before_limit() {
    let dashboard = Dashboard::start();
    for index in 0..25 {
        dashboard.seed_memory("concept", &format!("Zebrafixture concept {index}"));
    }
    dashboard.seed_memory("decision", "Zebrafixture decision");
    let (status, body) = dashboard.request("GET", "/api/search?type=decision&q=Zebrafixture");
    assert_eq!(status, 200);
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["type"], "decision");
    let (_, absent) = dashboard.request("GET", "/api/search?q=Zebrafixture&type=resolution");
    assert_eq!(absent, json!([]));
    let (_, all) = dashboard.request("GET", "/api/search?q=Zebrafixture");
    assert_eq!(all.as_array().unwrap().len(), 20);
}

#[test]
fn issue_268_memory_search_rejects_unknown_type() {
    let dashboard = Dashboard::start();
    let (status, body) = dashboard.request("GET", "/api/search?q=Zebrafixture&type=bogus");
    assert_eq!(status, 400);
    assert!(body["error"].is_string());
}

#[test]
fn issue_268_memory_search_preserves_rest_supported_types() {
    let dashboard = Dashboard::start();
    for node_type in ["instinct", "psychographic"] {
        dashboard.seed_memory(node_type, &format!("Zebrafixture {node_type}"));
        let (status, body) = dashboard.request(
            "GET",
            &format!("/api/search?q=Zebrafixture&type={node_type}"),
        );
        assert_eq!(status, 200);
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["type"], node_type);
    }
}
