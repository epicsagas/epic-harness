use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn issue246_dependency_transition_updates_run_and_status_together() {
    for resolved in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let work = temp.path().join("project");
        let data = temp.path().join("data");
        let orch = data.join("projects/project/orchestrator");
        std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(orch.join("agents/tester")).unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        std::fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        let now = epic_harness::shared::helpers::now_iso();
        let agent = |id: &str, status: &str| serde_json::json!({"id":id,"role":"tester","task":"synthetic","satisfies":[],"status":status,"started_at":now});
        let run = serde_json::json!({"id":"synthetic-run","status":"running","agents":[agent("builder","running"),agent("tester","blocked"),agent("other",if resolved{"done"}else{"running"})],"dependency_graph":{"tester":["builder","other"]},"created_at":now,"updated_at":now});
        std::fs::write(orch.join("run.json"), run.to_string()).unwrap();
        std::fs::write(orch.join("agents/tester/status.json"),serde_json::json!({"agent_id":"tester","phase":"waiting","progress":0,"last_heartbeat":now,"status":"blocked"}).to_string()).unwrap();
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
        child.stdin.take().unwrap().write_all(br#"{"hook_event_name":"SubagentStop","agent_id":"builder","session_id":"synthetic"}"#).unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
        let run: serde_json::Value =
            serde_json::from_slice(&std::fs::read(orch.join("run.json")).unwrap()).unwrap();
        let status: serde_json::Value =
            serde_json::from_slice(&std::fs::read(orch.join("agents/tester/status.json")).unwrap())
                .unwrap();
        let expected = if resolved { "pending" } else { "blocked" };
        assert_eq!(run["agents"][1]["status"], expected);
        assert_eq!(
            status["status"], expected,
            "per-agent status disagrees with dependency state"
        );
        if resolved {
            assert_eq!(status["phase"], "pending");
            assert!(
                std::fs::read_to_string(orch.join("agents/tester/inbox.jsonl"))
                    .unwrap()
                    .contains("unblocked")
            );
        }
    }
}
