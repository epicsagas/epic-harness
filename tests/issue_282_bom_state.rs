use epic_harness::{orchestrate::state, shared};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Output, Stdio};

#[test]
fn issue282_shared_whole_file_readers_accept_one_leading_bom_only() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("state.json");
    for prefix in ["", "\u{feff}"] {
        std::fs::write(&file, format!("{prefix}{{\"text\":\"\u{feff}preserved\"}}")).unwrap();
        let expected = json!({"text":"\u{feff}preserved"});
        assert_eq!(shared::read_json(&file, Value::Null), expected);
        assert_eq!(state::read_json_opt::<Value>(&file), Some(expected));
    }
    for invalid in [
        "\u{feff}\u{feff}{}".as_bytes(),
        " \u{feff}{}".as_bytes(),
        "{\"x\":\u{feff}1}".as_bytes(),
        "\u{feff}{".as_bytes(),
        &[0xef, 0xbb, 0xbf, 0xff],
    ] {
        std::fs::write(&file, invalid).unwrap();
        assert_eq!(shared::read_json(&file, Value::Null), Value::Null);
        assert_eq!(state::read_json_opt::<Value>(&file), None);
    }
}

#[test]
fn issue282_native_orbit_and_pause_controls_accept_powershell_bom() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let home = temp.path().join("home");
    let data = temp.path().join("data");
    let base = data.join("projects/project");
    for dir in [
        cwd.clone(),
        home.join(".config/epic-harness"),
        base.join("orbit"),
        base.join("orchestrator"),
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let invoke = |args: &[&str], input: &str| -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&cwd)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", temp.path())
            .env("EPIC_ORCHESTRATION", "enabled")
            .env("EPIC_AGENT_ID", "worker")
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
        child.wait_with_output().unwrap()
    };
    let pipeline = base.join("orbit/PIPELINE-bom.json");
    std::fs::write(
        &pipeline,
        format!(
            "\u{feff}{}",
            json!({"id":"bom","status":"running","phase":"go","updated_at":shared::now_iso()})
        ),
    )
    .unwrap();
    for verb in ["lock", "unlock"] {
        assert_eq!(
            invoke(&["orbit", verb], "").status.code(),
            Some(1),
            "BOM active pipeline must prevent {verb}"
        );
    }
    std::fs::write(&pipeline, format!("\u{feff}{}", json!({"id":"bom","status":"complete","phase":"ship","audit_fail_count":4,"max_retries":3}))).unwrap();
    let completion = invoke(&["orbit", "unlock"], "");
    assert!(completion.status.success());
    let warnings = String::from_utf8_lossy(&completion.stderr);
    assert!(warnings.contains("exceeds max_retries"));
    assert!(warnings.contains("without a PR url"));
    std::fs::write(
        base.join("orchestrator/control.json"),
        "\u{feff}{\"action\":\"pause\",\"target\":\"worker\",\"generation\":1}",
    )
    .unwrap();
    assert!(matches!(
        state::read_control(&base).unwrap().action,
        state::ControlAction::Pause
    ));
}
