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

fn encoded(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn directory_link(link: &std::path::Path, target: &std::path::Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let result = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(target.to_string_lossy().replace('/', "\\"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn issue_220_project_selectors_reject_path_syntax_without_read_or_write() {
    let dashboard = Dashboard::start();
    let outside = dashboard.data.parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("manifests.jsonl"),
        "{\"marker\":\"OUTSIDE_SENTINEL\"}\n",
    )
    .unwrap();
    let registry = "{\"solved\":{\"session\":1,\"real_task\":0.8},\"total_solved\":2}";
    fs::write(outside.join("seesaw.json"), registry).unwrap();
    for project in [
        "../../outside".to_string(),
        "..\\..\\outside".to_string(),
        outside.to_string_lossy().into_owned(),
    ] {
        for cmd in [
            "get_manifests",
            "get_seesaw_registry",
            "get_variant_pool",
            "get_harness_snapshot",
        ] {
            let (status, body) = dashboard.request(
                "GET",
                &format!("/api/harness?cmd={cmd}&project={}", encoded(&project)),
            );
            assert_eq!(status, 400, "{cmd} must reject selector {project}");
            assert!(body.get("error").is_some());
            assert!(!body.to_string().contains("OUTSIDE_SENTINEL"));
        }
    }
    assert_eq!(
        fs::read_to_string(outside.join("seesaw.json")).unwrap(),
        registry
    );
    let manifest = dashboard.data.join("projects/project-a/manifests.jsonl");
    fs::write(manifest, "{\"marker\":\"INSIDE_CONTROL\"}\n").unwrap();
    let (status, body) =
        dashboard.request("GET", "/api/harness?cmd=get_manifests&project=project-a");
    assert_eq!(status, 200);
    assert_eq!(body[0]["marker"], "INSIDE_CONTROL");
}

#[test]
fn issue_220_seesaw_get_is_read_only() {
    let dashboard = Dashboard::start();
    let file = dashboard.file(
        "projects/project-a/seesaw.json",
        &json!({"solved":{"session":1,"real_task":0.8},"total_solved":2}),
    );
    let before = fs::read(&file).unwrap();
    let (status, _) = dashboard.request(
        "GET",
        "/api/harness?cmd=get_seesaw_registry&project=project-a",
    );
    assert_eq!(status, 200);
    assert_eq!(
        fs::read(file).unwrap(),
        before,
        "a dashboard GET must not migrate or rewrite its source"
    );
}

#[test]
fn issue_220_project_and_nested_junctions_cannot_escape_project_data() {
    let dashboard = Dashboard::start();
    let outside = dashboard.data.parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("manifests.jsonl"),
        "{\"marker\":\"OUTSIDE_SENTINEL\"}\n",
    )
    .unwrap();
    directory_link(&dashboard.data.join("projects/linked"), &outside);
    assert_eq!(
        dashboard
            .request("GET", "/api/harness?cmd=get_manifests&project=linked")
            .0,
        400
    );

    for (folder, cmd) in [
        ("evolved", "get_evolved_skills"),
        ("obs", "get_obs_summary"),
        ("orbit", "get_orbit_pipelines"),
        ("sessions", "get_session_snapshots"),
    ] {
        let fixture = Dashboard::start();
        let external = fixture.data.parent().unwrap().join("outside");
        fs::create_dir_all(&external).unwrap();
        directory_link(
            &fixture.data.join("projects/project-a").join(folder),
            &external,
        );
        assert_eq!(
            fixture
                .request("GET", &format!("/api/harness?cmd={cmd}&project=project-a"))
                .0,
            400,
            "{folder} descendant must remain contained"
        );
    }
}
