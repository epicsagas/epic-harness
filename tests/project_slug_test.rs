//! `epic-harness slug` for separate git dirs, submodules and linked worktrees.
//! Every run uses a temp HOME, HARNESS_DIR and repo tree; git is isolated from
//! the user's config. Skipped when `git` is not installed.
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "protocol.file.allow=always"])
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Fx {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for d in ["home", "harness", "work"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        Self { _temp: temp, root }
    }

    fn work(&self, name: &str) -> PathBuf {
        let p = self.root.join("work").join(name);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Repo with one commit, optionally with a separate git dir.
    fn repo(&self, name: &str, separate_gitdir: Option<&Path>) -> PathBuf {
        let dir = self.work(name);
        match separate_gitdir {
            Some(g) => git(
                &dir,
                &["init", "--separate-git-dir", g.to_str().unwrap(), "."],
            ),
            None => git(&dir, &["init"]),
        }
        git(&dir, &["commit", "--allow-empty", "-m", "init"]);
        dir
    }

    fn slug(&self, cwd: &Path) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("slug")
            .current_dir(cwd)
            .env("HOME", self.root.join("home"))
            .env("HARNESS_DIR", self.root.join("harness"))
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

#[test]
fn normal_repo_root_and_subdir() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let repo = fx.repo("myrepo", None);
    fs::create_dir_all(repo.join("a/b")).unwrap();
    assert_eq!(fx.slug(&repo), "myrepo");
    assert_eq!(fx.slug(&repo.join("a/b")), "myrepo");
}

#[test]
fn linked_worktree_equals_primary() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let repo = fx.repo("myrepo", None);
    let wt = fx.root.join("work/orbit-wt");
    git(&repo, &["worktree", "add", wt.to_str().unwrap()]);
    assert_eq!(fx.slug(&wt), fx.slug(&repo));
    assert_eq!(fx.slug(&wt), "myrepo");
}

#[test]
fn separate_git_dir_primary_subdir_and_worktree_share_slug() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let backing = fx.root.join("metadata/backing.git");
    fs::create_dir_all(backing.parent().unwrap()).unwrap();
    let repo = fx.repo("proj", Some(&backing));
    fs::create_dir_all(repo.join("sub")).unwrap();
    let wt = fx.root.join("work/proj-wt");
    git(&repo, &["worktree", "add", wt.to_str().unwrap()]);
    assert_eq!(fx.slug(&repo), "backing");
    assert_eq!(fx.slug(&repo.join("sub")), "backing");
    assert_eq!(fx.slug(&wt), "backing");
}

#[test]
fn submodules_get_their_own_names() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let one = fx.repo("src-one", None);
    let two = fx.repo("src-two", None);
    let sup = fx.repo("super", None);
    for (name, src) in [("libone", &one), ("libtwo", &two)] {
        git(&sup, &["submodule", "add", src.to_str().unwrap(), name]);
    }
    assert_eq!(fx.slug(&sup.join("libone")), "libone");
    assert_eq!(fx.slug(&sup.join("libtwo")), "libtwo");
    assert_eq!(fx.slug(&sup), "super");
}

#[test]
fn non_git_dir_uses_dir_name() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    assert_eq!(fx.slug(&fx.work("plainproj")), "plainproj");
}

/// The pre-0.10 slug dir is only pointed at (stderr), never read or merged.
#[test]
fn resume_hints_legacy_slug_on_stderr_only() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let backing = fx.root.join("metadata/backing.git");
    fs::create_dir_all(backing.parent().unwrap()).unwrap();
    let repo = fx.repo("proj", Some(&backing));
    let harness = fx.root.join("harness");
    fs::write(
        harness.join("config.toml"),
        "[dashboard]\nport = 0\nauto_open = false\n",
    )
    .unwrap();
    let cfg = fx.root.join("home/.config/epic-harness");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("telemetry-consent"), "off").unwrap();
    fs::create_dir_all(harness.join("projects/metadata")).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("resume")
        .current_dir(&repo)
        .env("HOME", fx.root.join("home"))
        .env("HARNESS_DIR", &harness)
        .env("GIT_CEILING_DIRECTORIES", &fx.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"session_id":"sid-1","source":"startup"}"#)
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        err.contains("merge-project --from metadata --to backing"),
        "{err}"
    );
    assert!(!stdout.contains("pre-0.10"), "{stdout}");
    // Nothing merged: the new slug dir holds no copied data.
    assert!(
        !harness
            .join("projects/backing/obs")
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    );
}
