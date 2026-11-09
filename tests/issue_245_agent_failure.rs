use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn issue245_explicit_failed_agent_result_overrides_done_text() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let work = temp.path().join("project");
    let data = temp.path().join("data");
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(data.join("projects/project")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    for payload in [
        serde_json::json!({"hook_event_name":"SubagentStart","agent_id":"worker","session_id":"synthetic"}),
        serde_json::json!({"hook_event_name":"SubagentStop","agent_id":"worker","session_id":"synthetic","exit_code":1,"tool_response":{"output":"## Status: DONE"}}),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("observe")
            .current_dir(&work)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", temp.path())
            .env("EPIC_ORCHESTRATION", "enabled")
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
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
    }
    let orch = data.join("projects/project/orchestrator");
    let run: serde_json::Value =
        serde_json::from_slice(&std::fs::read(orch.join("run.json")).unwrap()).unwrap();
    assert_eq!(run["agents"][0]["status"], "failed");
    assert!(run["agents"][0]["completed_at"].is_string());
    let status: serde_json::Value =
        serde_json::from_slice(&std::fs::read(orch.join("agents/worker/status.json")).unwrap())
            .unwrap();
    assert_eq!(status["status"], "failed");
    assert_eq!(status["phase"], "failed");
}
