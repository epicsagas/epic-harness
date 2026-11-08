//! Exercise fresh CLI processes: project identity is cached per process.
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn checkout_subdirectory_and_linked_worktree_share_harness() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("main project");
    let linked = temp.path().join("different worktree name");
    let sub = repo.join("nested directory");
    std::fs::create_dir_all(&sub).unwrap();
    git(&repo, &["init"]);
    git(&repo, &["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-m", "init"]);
    git(&repo, &["worktree", "add", "--detach", linked.to_str().unwrap()]);
    for command in ["slug", "path"] {
        let values: Vec<_> = [&repo, &sub, &linked].iter().map(|dir| {
            let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
                .arg(command).current_dir(dir)
                .env("HARNESS_DIR", temp.path().join("isolated harness"))
                .output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap().trim().to_owned()
        }).collect();
        assert_eq!(values[0], values[1], "{command}: subdirectory");
        assert_eq!(values[0], values[2], "{command}: linked worktree");
    }
}
