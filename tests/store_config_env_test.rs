//! store_config_env_test.rs — HARNESS_MEM_URL must survive a missing or unreadable config.toml.
//! Runs the binary with temp HOME and HARNESS_DIR so no real harness state is touched.

use std::process::Command;

#[test]
fn explicit_memory_url_survives_missing_and_unreadable_config() {
    for invalid in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let data = tmp.path().join("data");
        std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        if invalid {
            // Invalid UTF-8: read_to_string fails with a non-NotFound error.
            std::fs::write(data.join("config.toml"), [0xff, 0xfe]).unwrap();
        }
        let selected = tmp.path().join("selected.db");

        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(["mem", "add", "--title", "env marker", "--body", "synthetic"])
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("HARNESS_DIR", &data)
            .env("HARNESS_ROOT", tmp.path())
            .env("HARNESS_MEM_URL", format!("sqlite:{}", selected.display()))
            .env_remove("HARNESS_DB_URL")
            .env_remove("DATABASE_URL")
            .current_dir(tmp.path())
            .output()
            .unwrap();

        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(selected.is_file(), "override ignored (invalid={invalid})");
        assert!(
            !tmp.path().join(".harness/memory.db").exists(),
            "default memory.db written (invalid={invalid})"
        );
    }
}
