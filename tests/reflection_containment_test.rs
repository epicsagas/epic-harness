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
    let link = root.join("projects/escape");
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
