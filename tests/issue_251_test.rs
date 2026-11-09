use std::process::Command;

#[test]
fn explicit_memory_url_survives_missing_and_unreadable_config() {
    for invalid in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let data = temp.path().join("data");
        std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        if invalid {
            std::fs::write(data.join("config.toml"), [0xff, 0xfe]).unwrap();
        }
        let selected = temp.path().join("selected memory.db");
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args([
                "mem",
                "add",
                "--title",
                "Fixture URL selection",
                "--body",
                "synthetic",
            ])
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", temp.path())
            .env("HARNESS_MEM_URL", format!("sqlite:{}", selected.display()))
            .env_remove("HARNESS_DB_URL")
            .env_remove("DATABASE_URL")
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            selected.is_file(),
            "explicit URL ignored when invalid={invalid}"
        );
        assert!(!temp.path().join(".harness/memory.db").exists());
    }
}
