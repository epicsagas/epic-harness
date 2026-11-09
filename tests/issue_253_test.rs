use std::process::Command;

#[test]
fn separate_git_directory_uses_checkout_identity_not_metadata_parent() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("Checkout Ž space");
    let metadata = temp.path().join("unrelated metadata");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir(&metadata).unwrap();
    let git = Command::new("git")
        .current_dir(&repo)
        .args(["init", "--separate-git-dir"])
        .arg(metadata.join("backing.git"))
        .output()
        .unwrap();
    assert!(
        git.status.success(),
        "{}",
        String::from_utf8_lossy(&git.stderr)
    );
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("slug")
        .current_dir(&repo)
        .env("HARNESS_DIR", temp.path().join("data"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "Checkout_Ž_space"
    );
}
