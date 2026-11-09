use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn hook(root: &Path, project: &str, command: &str, input: serde_json::Value) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg(command)
        .current_dir(root.join(project))
        .env("HARNESS_DIR", root.join("data"))
        .env("HARNESS_ROOT", root)
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("EPIC_HOOK_PROFILE", "standard")
        .env("EPIC_ORCHESTRATION", "disabled")
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
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn documented_project_opt_in_exports_patterns_and_controls_receiving_hints() {
    let temp = tempfile::tempdir().unwrap();
    for dir in [
        "source",
        "receiver",
        "data/projects/source",
        "data/projects/receiver",
        "home/.config/epic-harness",
    ] {
        std::fs::create_dir_all(temp.path().join(dir)).unwrap();
    }
    std::fs::write(
        temp.path()
            .join("home/.config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("data/config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n[model]\nclass=\"light\"\n",
    )
    .unwrap();
    std::fs::write(
        temp.path()
            .join("data/projects/source/.cross-project-enabled"),
        "",
    )
    .unwrap();
    for round in 0..2 {
        for attempt in 0..6 {
            hook(
                temp.path(),
                "source",
                "observe",
                serde_json::json!({
                    "hook_event_name":"PostToolUse","session_id":format!("cross-{round}"),"tool_name":"Bash",
                    "tool_input":{"command":format!("node fixture{attempt}.js --round {round} --attempt {attempt}")},
                    "tool_response":{"output":"TypeError: synthetic cross-project fixture failure","exit_code":1}
                }),
            );
        }
        hook(
            temp.path(),
            "source",
            "reflect",
            serde_json::json!({"hook_event_name":"Stop","session_id":format!("cross-{round}")}),
        );
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }
    let exported = temp.path().join("data/global_patterns.jsonl");
    assert!(
        exported.is_file(),
        "documented opt-in did not export to documented location"
    );
    assert_eq!(
        std::fs::read_to_string(exported).unwrap().lines().count(),
        2
    );
    let disabled = hook(
        temp.path(),
        "receiver",
        "resume",
        serde_json::json!({"hook_event_name":"SessionStart","source":"compact","session_id":"receiver"}),
    );
    assert!(!String::from_utf8_lossy(&disabled.stdout).contains("Cross-project:"));
    std::fs::write(
        temp.path()
            .join("data/projects/receiver/.cross-project-enabled"),
        "",
    )
    .unwrap();
    let enabled = hook(
        temp.path(),
        "receiver",
        "resume",
        serde_json::json!({"hook_event_name":"SessionStart","source":"compact","session_id":"receiver"}),
    );
    assert!(String::from_utf8_lossy(&enabled.stdout).contains("Cross-project:"));
}
