//! A host-supplied session id must not steer where hooks create files. Every run
//! uses a temp HOME, HARNESS_DIR and cwd; the id below is synthetic.
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
/// Separators and parent references in every form a host could send.
const EVIL_ID: &str = "x/../../../escaped\\..\\y";

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

impl Env {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::create_dir_all(harness.join("projects").join(SLUG).join("obs")).unwrap();
        // No dashboard auto-launch: it would add dashboard.lock (and spawn a
        // server) whenever port 7700 is free, as on CI.
        fs::write(
            harness.join("config.toml"),
            "[dashboard]\nport = 0\nauto_open = false\n",
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            cwd,
            home,
            harness,
        }
    }

    fn project(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG)
    }

    /// An empty directory where the database file belongs makes SQLite
    /// unavailable, so hooks write their JSONL fallback.
    fn block_sqlite(&self) {
        fs::create_dir(self.harness.join("harness.db")).unwrap();
    }

    fn hook(&self, name: &str, payload: serde_json::Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg(name)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    }

    /// Every file under the temp tree with the given extension.
    fn files_with_ext(&self, ext: &str) -> Vec<PathBuf> {
        fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
            for e in fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, ext, out);
                } else if p.extension().is_some_and(|x| x == ext) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, ext, &mut out);
        out
    }
}

fn assert_flat_in(files: &[PathBuf], dir: &Path) {
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].parent().unwrap(), dir, "{files:?}");
    let name = files[0].file_stem().unwrap().to_string_lossy();
    assert!(
        name.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.'),
        "{name}"
    );
    assert!(!name.contains(".."), "{name}");
}

#[test]
fn observe_fallback_file_stays_in_the_obs_dir() {
    let env = Env::new();
    env.block_sqlite();
    env.hook(
        "observe",
        serde_json::json!({
            "session_id": EVIL_ID,
            "tool_name": "Read",
            "tool_input": {"file_path": "src/a.rs"},
            "tool_response": "synthetic body",
        }),
    );
    assert_flat_in(&env.files_with_ext("jsonl"), &env.project().join("obs"));
}

#[test]
fn polish_fallback_file_stays_in_the_obs_dir() {
    let env = Env::new();
    env.block_sqlite();
    // A workspace-local tsc that reports an error, so polish records a failure.
    fs::write(env.cwd.join("tsconfig.json"), "{}").unwrap();
    fs::write(env.cwd.join("a.ts"), "export {};\n").unwrap();
    let bin = env.cwd.join("node_modules/.bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("tsc"),
        "#!/bin/sh\necho 'a.ts: error TS2304: synthetic'\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(bin.join("tsc"), fs::Permissions::from_mode(0o755)).unwrap();
    env.hook(
        "polish",
        serde_json::json!({
            "session_id": EVIL_ID,
            "tool_name": "Edit",
            "tool_input": {"file_path": "a.ts"},
        }),
    );
    assert_flat_in(&env.files_with_ext("jsonl"), &env.project().join("obs"));
}

#[test]
fn resume_lock_stays_in_the_project_dir() {
    let env = Env::new();
    env.hook(
        "resume",
        serde_json::json!({"session_id": EVIL_ID, "source": "startup"}),
    );
    assert_flat_in(&env.files_with_ext("lock"), &env.project());
}
