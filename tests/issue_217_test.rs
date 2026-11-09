use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn resume(root: &Path, source: &str, session: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("resume")
        .current_dir(root.join("project"))
        .env("HARNESS_DIR", root.join("data"))
        .env("HARNESS_ROOT", root)
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(serde_json::json!({"hook_event_name":"SessionStart","source":source,"session_id":session}).to_string().as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    for dir in [
        "project/.harness/memory",
        "project/.harness/worktrees",
        "home/.config/epic-harness",
        "data",
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
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("project/.harness/worktrees/retain.txt"),
        "working tree",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("project/.harness/guard-rules.yaml"),
        "blocked:\n",
    )
    .unwrap();
    temp
}

#[test]
fn first_startup_migrates_legacy_memory_and_preserves_local_project_resources() {
    let temp = fixture();
    std::fs::write(
        temp.path().join("project/.harness/memory/rule.md"),
        "legacy rule",
    )
    .unwrap();
    let out = resume(temp.path(), "startup", "first");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("data/projects/project/memory/rule.md")).unwrap(),
        "legacy rule"
    );
    assert!(
        temp.path()
            .join("project/.harness/worktrees/retain.txt")
            .is_file()
    );
    assert!(
        temp.path()
            .join("project/.harness/guard-rules.yaml")
            .is_file()
    );
    assert!(
        temp.path()
            .join("project/.harness/memory/rule.md")
            .is_file(),
        "successful imports retain the original as a safety backup"
    );
}

#[test]
fn preexisting_partial_destination_is_retried_without_overwriting_conflicts() {
    let temp = fixture();
    let source = temp.path().join("project/.harness/memory");
    let target = temp.path().join("data/projects/project/memory");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(source.join("same.md"), "same").unwrap();
    std::fs::write(target.join("same.md"), "same").unwrap();
    std::fs::write(source.join("conflict.md"), "legacy distinct").unwrap();
    std::fs::write(target.join("conflict.md"), "current distinct").unwrap();
    let first = resume(temp.path(), "compact", "partial");
    assert!(String::from_utf8_lossy(&first.stderr).contains("Migration partial"));
    assert_eq!(
        std::fs::read_to_string(source.join("conflict.md")).unwrap(),
        "legacy distinct"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("conflict.md")).unwrap(),
        "current distinct"
    );
    std::fs::write(source.join("conflict.md"), "current distinct").unwrap();
    std::fs::write(source.join("repaired.md"), "recovered").unwrap();
    let second = resume(temp.path(), "compact", "partial");
    assert!(second.status.success());
    assert_eq!(
        std::fs::read_to_string(target.join("repaired.md")).unwrap(),
        "recovered"
    );
    assert!(
        source.exists(),
        "successful imports do not destructively remove legacy originals"
    );
    assert!(
        temp.path()
            .join("project/.harness/worktrees/retain.txt")
            .is_file()
    );
}

#[test]
fn legacy_import_preserves_file_permissions() {
    let temp = fixture();
    let source = temp.path().join("project/.harness/memory/readonly.md");
    std::fs::write(&source, "readonly legacy rule").unwrap();
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o555);
    }
    std::fs::set_permissions(&source, permissions).unwrap();
    let out = resume(temp.path(), "compact", "permissions");
    assert!(out.status.success());
    let copied = std::fs::metadata(temp.path().join("data/projects/project/memory/readonly.md"))
        .unwrap()
        .permissions();
    assert!(
        copied.readonly(),
        "an import must preserve original readonly/executable permission metadata"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(copied.mode() & 0o777, 0o555);
    }
}
