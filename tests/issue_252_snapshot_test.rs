use std::path::Path;
use std::process::Command;

fn directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let out = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(target)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn snapshot(root: &Path) -> serde_json::Value {
    let home = root.join("home");
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        root.join("data/config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["harness", "snapshot"])
        .current_dir(root.join("project"))
        .env("HARNESS_DIR", root.join("data"))
        .env("HARNESS_ROOT", root)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn snapshot_omits_escaping_evolved_directory() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("project")).unwrap();
    std::fs::create_dir_all(root.join("data/projects/project")).unwrap();
    std::fs::create_dir_all(root.join("outside/OUTSIDE_SNAPSHOT_SENTINEL")).unwrap();
    directory_link(
        &root.join("outside"),
        &root.join("data/projects/project/evolved"),
    );
    let result = snapshot(root);
    assert_eq!(
        result["active_skills"],
        serde_json::json!([]),
        "snapshot must not enumerate an escaping evolved directory"
    );
    assert_eq!(result["evolved_skills"], serde_json::json!([]));
}

#[test]
fn snapshot_checks_each_evolved_child_and_preserves_valid_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("project")).unwrap();
    let evolved = root.join("data/projects/project/evolved");
    std::fs::create_dir_all(evolved.join("safe-skill")).unwrap();
    std::fs::create_dir(root.join("outside")).unwrap();
    directory_link(&root.join("outside"), &evolved.join("outside-skill"));
    let result = snapshot(root);
    assert_eq!(result["active_skills"], serde_json::json!(["safe-skill"]));
    assert_eq!(result["evolved_skills"], serde_json::json!(["safe-skill"]));
}

#[test]
fn snapshot_omits_escaping_project_and_accepts_missing_evolved_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("project")).unwrap();
    std::fs::create_dir_all(root.join("data/projects")).unwrap();
    assert_eq!(snapshot(root)["active_skills"], serde_json::json!([]));
    std::fs::create_dir_all(root.join("outside/evolved/OUTSIDE_PROJECT_SENTINEL")).unwrap();
    directory_link(&root.join("outside"), &root.join("data/projects/project"));
    assert_eq!(snapshot(root)["active_skills"], serde_json::json!([]));
}
