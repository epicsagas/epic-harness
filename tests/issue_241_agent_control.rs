use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn issue241_control_blocks_synchronous_spawn_and_tracks_lifecycle_without_false_running() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../.claude-plugin/hooks.json")).unwrap();
    let agent_hook = manifest["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["matcher"] == "Agent")
        .unwrap();
    assert_eq!(agent_hook["hooks"][0]["async"], false);
    for (action, expected) in [("pause", "blocked"), ("cancel", "failed")] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let work = temp.path().join("project");
        let data = temp.path().join("data");
        let orch = data.join("projects/project/orchestrator");
        std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&orch).unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        std::fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        std::fs::write(
            orch.join("control.json"),
            serde_json::json!({"action":action,"target":"worker","generation":1}).to_string(),
        )
        .unwrap();
        for (event, id, code) in [
            ("PreToolUse", "worker", 2),
            ("SubagentStart", "worker", 0),
            ("PreToolUse", "other", 0),
        ] {
            let input = serde_json::json!({"hook_event_name":event,"tool_name":"Agent","agent_id":id,"session_id":"synthetic","tool_input":{"agent_id":id,"description":"Synthetic controlled task"}});
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
                .write_all(input.to_string().as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(code),
                "action={action},event={event},id={id}"
            );
            if code == 2 {
                let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
            }
            let run: serde_json::Value =
                serde_json::from_slice(&std::fs::read(orch.join("run.json")).unwrap()).unwrap();
            let agent = run["agents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["id"] == id)
                .unwrap();
            assert_eq!(
                agent["status"],
                if id == "worker" { expected } else { "running" }
            );
        }
    }
}
