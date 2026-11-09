use std::fs;
use std::process::Command;

#[test]
fn managed_install_reports_its_pin_and_refuses_replacement_without_side_effects() {
    let fixture = tempfile::tempdir().unwrap();
    let bin = fixture.path().join("managed binary with spaces");
    let data = fixture.path().join("data");
    let home = fixture.path().join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    let executable = bin.join(if cfg!(windows) { "epic.exe" } else { "epic" });
    fs::copy(env!("CARGO_BIN_EXE_epic"), &executable).unwrap();
    let marker = bin.join("epic-managed.json");
    fs::write(&marker, r#"{"schema":1,"repository":"example/epic-harness","commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","automatic_updates":false}"#).unwrap();
    let original = fs::read(&marker).unwrap();
    for args in [
        vec!["update", "--check"],
        vec!["update"],
        vec!["update", "--force"],
    ] {
        let output = Command::new(&executable)
            .args(&args)
            .current_dir(fixture.path())
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", fixture.path())
            .env("PATH", "")
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .output()
            .unwrap();
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostics.contains("Managed:"), "{args:?}: {diagnostics}");
        assert!(diagnostics.contains("aaaaaaaaaaaa"), "{diagnostics}");
        assert!(
            !diagnostics.contains("Latest:"),
            "managed checks must not query upstream: {diagnostics}"
        );
        assert_eq!(
            output.status.success(),
            args.contains(&"--check"),
            "{diagnostics}"
        );
        assert!(!data.join(".last-binary-sync").exists());
        assert!(!data.join("memory.db").exists());
        assert_eq!(fs::read(&marker).unwrap(), original);
    }
    // Seed the existing legacy resolver for the ordinary control; the independent
    // --check path fix is tracked separately by #224.
    let plugin = fixture.path().join("legacy plugin");
    fs::create_dir_all(plugin.join("hooks/bin")).unwrap();
    fs::write(plugin.join("hooks/bin/epic-harness"), "resolver control").unwrap();
    // Opting back into updates must restore the ordinary check path.
    fs::write(
        &marker,
        String::from_utf8(original)
            .unwrap()
            .replace("false", "true"),
    )
    .unwrap();
    let output = Command::new(&executable)
        .args(["update", "--check"])
        .current_dir(fixture.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("HARNESS_DIR", &data)
        .env("HARNESS_ROOT", fixture.path())
        .env("PATH", "")
        .env("CLAUDE_PLUGIN_ROOT", &plugin)
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("Current:"), "{diagnostics}");
    assert!(!diagnostics.contains("Managed:"), "{diagnostics}");
}
