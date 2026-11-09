use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn startup_session_identity_cannot_create_lock_outside_project() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let data = temp.path().join("data");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let input = serde_json::json!({"hook_event_name":"SessionStart","source":"startup","session_id":"x/../../../../outside/fixture"}).to_string();
    for _ in 0..2 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("resume")
            .current_dir(&cwd)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", temp.path())
            .env("HOME", &home)
            .env("USERPROFILE", &home)
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
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(!temp.path().join("outside/fixture.startup.lock").exists());
    let project = data.join("projects/project");
    let locks: Vec<_> = std::fs::read_dir(project)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("resume."))
        .collect();
    assert_eq!(
        locks.len(),
        1,
        "stable identity must still deduplicate startup"
    );
}
