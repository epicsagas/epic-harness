use std::fs;
use std::process::Command;

#[test]
fn update_check_reports_the_running_binary_without_path_tools_or_installers() {
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path().join("home");
    let data = fixture.path().join("data");
    let bin = fixture.path().join("binary with spaces");
    fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let executable = bin.join(if cfg!(windows) { "epic.exe" } else { "epic" });
    fs::copy(env!("CARGO_BIN_EXE_epic"), &executable).unwrap();
    let output = Command::new(executable)
        .args(["update", "--check"])
        .current_dir(fixture.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("HARNESS_DIR", &data)
        .env("HARNESS_ROOT", fixture.path())
        .env("PATH", "")
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .env_remove("CLAUDE_PLUGIN_ROOT")
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("Current:"), "{diagnostics}");
    assert!(
        !diagnostics.contains("installing"),
        "a check may never install: {diagnostics}"
    );
    assert!(
        output.status.success(),
        "offline check must retain its existing return contract: {diagnostics}"
    );
    assert!(!data.join(".last-binary-sync").exists());
    assert!(
        !data.join("memory.db").exists(),
        "check must not register MCP or launch its database"
    );
}
