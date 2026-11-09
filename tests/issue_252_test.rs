use std::process::Command;

#[test]
fn reflection_does_not_import_observations_through_escaping_child_directory() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let data = temp.path().join("data");
    let project = data.join("projects/project");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    std::fs::write(outside.join("session_20990101.jsonl"), "{\"timestamp\":\"2099-01-01T00:00:00Z\",\"tool\":\"OutsideSyntheticTool\",\"tool_category\":\"other\",\"result\":\"success\",\"score\":1}\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, project.join("obs")).unwrap();
    #[cfg(windows)]
    {
        let out = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(project.join("obs").to_string_lossy().replace('/', "\\"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "mklink: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args([
            "reflect",
            "--context",
            "--since",
            "20200101",
            "--project",
            "project",
        ])
        .current_dir(&project)
        .env("HARNESS_DIR", &data)
        .env("HARNESS_ROOT", temp.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    if out.status.success() {
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            json["obs_stats"]["total"], 0,
            "escaped records must never be imported"
        );
    }
}
