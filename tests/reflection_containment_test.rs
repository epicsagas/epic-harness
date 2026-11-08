use std::path::Path;
use std::process::{Command, Output};

fn context(root: &Path, slug: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["reflect", "--context", "--project", slug])
        .current_dir(root.join("projects/valid"))
        .env("HARNESS_DIR", root)
        .output()
        .unwrap()
}

#[test]
fn existing_project_passes_and_traversal_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("harness with spaces");
    std::fs::create_dir_all(root.join("projects/valid")).unwrap();
    let valid = context(&root, "valid");
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(serde_json::from_slice::<serde_json::Value>(&valid.stdout).is_ok());
    assert!(!context(&root, "../outside").status.success());
    assert!(!context(&root, "..\\outside").status.success());
}

#[test]
fn project_directory_link_cannot_escape_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("harness");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(root.join("projects/valid")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let link = root.join("projects").join("escape");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    {
        let result = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let result = context(&root, "escape");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("escapes harness root"));
}

#[test]
fn alcove_vault_uses_canonical_home_and_rejects_escape() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("isolated home");
    let root = home.join("harness");
    let project = root.join("projects/valid");
    let vault = home.join("vault");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(vault.join("note.md"), "sample note").unwrap();
    for (path, allowed) in [(&vault, true), (&outside, false)] {
        let config = format!(
            "[context.alcove]\nvault_path = {}\n",
            serde_json::to_string(path).unwrap()
        );
        std::fs::write(root.join("config.toml"), config).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(["reflect", "--context", "--source", "alcove"])
            .current_dir(&project)
            .env("HARNESS_DIR", &root)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let alcove = &output["extra_sources"]["alcove"];
        if allowed {
            assert_eq!(alcove["docs_collected"], 1, "{alcove}");
        } else {
            assert!(alcove["error"].as_str().unwrap().contains("escapes home"));
        }
    }
}
