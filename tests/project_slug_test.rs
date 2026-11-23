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

/// Runs git in `dir`, returns trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Fx {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for d in ["home/.config/epic-harness", "harness", "work"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        // No dashboard auto-launch and no telemetry prompt for `resume`.
        fs::write(
            root.join("harness/config.toml"),
            "[dashboard]\nport = 0\nauto_open = false\n",
        )
        .unwrap();
        fs::write(
            root.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
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
        };
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

    /// Runs the `resume` hook in `cwd`; returns (stdout, stderr).
    fn resume(&self, cwd: &Path, source: &str) -> (String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg("resume")
            .current_dir(cwd)
            .env("HOME", self.root.join("home"))
            .env("HARNESS_DIR", self.root.join("harness"))
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let payload = format!(r#"{{"session_id":"sid-1","source":"{source}"}}"#);
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

/// The 0.9.x rule from real git output: an absolute common dir gives its
/// parent dirname, otherwise the toplevel dirname.
fn slug_0_9(cwd: &Path) -> String {
    let common = git(cwd, &["rev-parse", "--git-common-dir"]);
    let top = git(cwd, &["rev-parse", "--show-toplevel"]);
    let path = if Path::new(&common).is_absolute() {
        Path::new(&common).parent().unwrap().to_path_buf()
    } else {
        PathBuf::from(top)
    };
    path.file_name().unwrap().to_string_lossy().into_owned()
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
    assert_eq!(fx.slug(&repo), slug_0_9(&repo));
    assert_eq!(fx.slug(&repo.join("a/b")), slug_0_9(&repo.join("a/b")));
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
    assert_eq!(fx.slug(&wt), slug_0_9(&wt));
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

/// Bare-container layout: `<name>/.bare` + `<name>/.git` file + worktrees. A
/// dot-named git dir must not collapse every such project into one slug.
#[test]
fn dot_bare_container_layouts_keep_their_own_slug() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    for name in ["alpha", "beta"] {
        let src = fx.repo(&format!("src-{name}"), None);
        let container = fx.work(name);
        git(
            &container,
            &["clone", "--bare", src.to_str().unwrap(), ".bare"],
        );
        fs::write(container.join(".git"), "gitdir: ./.bare\n").unwrap();
        git(&container, &["worktree", "add", "main", "main"]);
        assert_eq!(fx.slug(&container), name);
        assert_eq!(fx.slug(&container.join("main")), name);
    }
}

/// The pre-0.10 slug dir is only pointed at (stderr), never read or merged,
/// and the ack file silences the hint.
#[test]
fn resume_hints_legacy_slug_on_stderr_only() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let backing = fx.root.join("metadata/backing.git");
    fs::create_dir_all(backing.parent().unwrap()).unwrap();
    let repo = fx.repo("proj", Some(&backing));
    let projects = fx.root.join("harness/projects");
    fs::create_dir_all(projects.join("metadata")).unwrap();

    let (stdout, stderr) = fx.resume(&repo, "startup");
    assert!(
        stderr.contains("merge-project --from metadata --to backing"),
        "{stderr}"
    );
    assert!(!stderr.contains("Submodules"), "{stderr}");
    assert!(!stdout.contains("pre-0.10"), "{stdout}");
    // Nothing merged: the new slug dir holds no copied data.
    assert!(
        !projects
            .join("backing/obs")
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    );

    fs::write(projects.join("backing/.legacy-slug-ack"), "").unwrap();
    let (_, stderr) = fx.resume(&repo, "resume");
    assert!(!stderr.contains("pre-0.10"), "{stderr}");
}

/// `.git` symlinked to a git dir elsewhere: git prints a relative `.git`, the
/// slug follows the link, and 0.9.x used the toplevel dirname.
#[test]
fn symlinked_dot_git_changes_slug_and_hints() {
    if !git_available() {
        return;
    }
    let fx = Fx::new();
    let real = fx.root.join("store/realrepo.git");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    let repo = fx.repo("r2", Some(&real));
    fs::remove_file(repo.join(".git")).unwrap();
    std::os::unix::fs::symlink(&real, repo.join(".git")).unwrap();
    assert_eq!(fx.slug(&repo), "realrepo");
    assert_eq!(slug_0_9(&repo), "r2");
    fs::create_dir_all(fx.root.join("harness/projects/r2")).unwrap();

    let (_, stderr) = fx.resume(&repo, "startup");
    assert!(
        stderr.contains("merge-project --from r2 --to realrepo"),
        "{stderr}"
    );
}
