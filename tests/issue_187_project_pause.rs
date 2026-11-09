use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn issue187_edit_pause_uses_the_project_store_with_and_without_root_override() {
    for override_root in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let work = temp.path().join("project");
        let data = if override_root {
            temp.path().join("data")
        } else {
            home.join(".harness")
        };
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        let orch = data.join("projects/project/orchestrator");
        std::fs::create_dir_all(&orch).unwrap();
        std::fs::write(
            data.join("config.toml"),
            "[dashboard]\nport=0\nauto_open=false\n",
        )
        .unwrap();
        std::fs::write(
            orch.join("control.json"),
            r#"{"action":"pause","target":"worker","generation":1}"#,
        )
        .unwrap();
        for (id, expected) in [("worker", 2), ("other", 0)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
            command
                .arg("guard")
                .current_dir(&work)
                .env("HOME", &home)
                .env("USERPROFILE", &home)
                .env("HARNESS_ROOT", temp.path())
                .env("EPIC_ORCHESTRATION", "enabled")
                .env("EPIC_AGENT_ID", id)
                .env_remove("HARNESS_DB_URL")
                .env_remove("HARNESS_MEM_URL")
                .env_remove("DATABASE_URL")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if override_root {
                command.env("HARNESS_DIR", &data);
            } else {
                command.env_remove("HARNESS_DIR");
            }
            let mut child = command.spawn().unwrap();
            child.stdin.take().unwrap().write_all(br#"{"hook_event_name":"PreToolUse","tool_name":"apply_patch","tool_input":{"input":"*** Begin Patch\n*** Update File: src/f.rs\n@@\n-a\n+b\n*** End Patch"}}"#).unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(expected),
                "override={override_root}, id={id}, stdout={}",
                String::from_utf8_lossy(&output.stdout)
            );
            if expected == 2 {
                let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
            }
        }
    }
}
