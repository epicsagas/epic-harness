//! `resume` must never move, copy or delete anything in the project's
//! `.harness/`; pre-v0.1.4 data there only produces a hint. Every run uses a
//! temp HOME, HARNESS_DIR and cwd; nothing outside the temp tree is touched.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
const GUARD: &str = "blocked:\n  - pattern: rm\\s+-rf | msg: synthetic rule\n";
const HINT: &str = "pre-v0.1.4";

const DASHBOARD_OFF: &str = "[dashboard]\nport = 0\nauto_open = false\n";

struct Env {
    _temp: tempfile::TempDir,
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
        fs::create_dir_all(cwd.join(".harness")).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        // No dashboard auto-launch: it would add dashboard.lock (and spawn a
        // server) whenever port 7700 is free, as on CI.
        fs::create_dir_all(&harness).unwrap();
        fs::write(harness.join("config.toml"), DASHBOARD_OFF).unwrap();
        Self {
            _temp: temp,
            cwd,
            home,
            harness,
        }
    }

    fn local(&self) -> PathBuf {
        self.cwd.join(".harness")
    }

    fn data(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG)
    }

    fn seed_user_files(&self) {
        fs::write(self.local().join("guard-rules.yaml"), GUARD).unwrap();
        fs::write(self.local().join("engagement.md"), "# scope\n").unwrap();
    }

    fn seed_legacy(&self) {
        self.seed_user_files();
        let local = self.local();
        fs::create_dir_all(local.join("obs")).unwrap();
        fs::write(local.join("obs/session_1.jsonl"), "{}\n").unwrap();
        fs::create_dir_all(local.join("evolved/x")).unwrap();
        fs::write(
            local.join("evolved/x/SKILL.md"),
            "---\nname: x\n---\nbody\n",
        )
        .unwrap();
        fs::write(local.join("metrics.json"), "{\"legacy\":true}").unwrap();
    }

    /// Runs the hook with this env's HOME/HARNESS_DIR; returns stderr.
    fn resume(&self, source: &str) -> String {
        self.run(source, &self.home, Some(&self.harness))
    }

    fn run(&self, source: &str, home: &Path, harness: Option<&Path>) -> String {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        cmd.arg("resume")
            .current_dir(&self.cwd)
            .env("HOME", home)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        match harness {
            Some(h) => cmd.env("HARNESS_DIR", h),
            None => cmd.env_remove("HARNESS_DIR"),
        };
        let mut child = cmd.spawn().unwrap();
        let payload = format!("{{\"session_id\":\"sid-1\",\"source\":\"{source}\"}}");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

/// Relative path to bytes for every file under `dir`.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().to_path_buf();
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn file_names(dir: &Path) -> Vec<PathBuf> {
    snapshot(dir).into_keys().collect()
}

#[test]
fn legacy_entries_only_produce_a_hint() {
    for source in ["startup", "compact", "resume", "clear"] {
        // Control run: what resume creates in the data dir with no legacy data.
        let control = Env::new();
        control.resume(source);
        let baseline = file_names(&control.data());

        let env = Env::new();
        env.seed_legacy();
        let before = snapshot(&env.local());
        let stderr = env.resume(source);
        assert_eq!(snapshot(&env.local()), before, "{source}");
        assert_eq!(stderr.matches(HINT).count(), 1, "{source}: {stderr}");
        assert!(
            stderr.contains(&env.data().display().to_string()),
            "{source}"
        );
        assert_eq!(file_names(&env.data()), baseline, "{source}");
        assert!(!env.data().join("obs/session_1.jsonl").exists());
        assert!(!env.data().join("evolved/x").exists());
    }
}

#[test]
fn user_owned_files_alone_do_not_trigger_the_hint() {
    let env = Env::new();
    env.seed_user_files();
    let before = snapshot(&env.local());
    let stderr = env.resume("startup");
    assert!(!stderr.contains(HINT), "{stderr}");
    assert_eq!(snapshot(&env.local()), before);
}

#[test]
fn cwd_equal_to_home_leaves_harness_root_alone() {
    // cwd == HOME and no HARNESS_DIR: the project .harness/ is the harness root.
    let env = Env::new();
    let root = env.cwd.join(".harness");
    fs::create_dir_all(root.join("obs")).unwrap();
    fs::write(root.join("obs/session_1.jsonl"), "{}\n").unwrap();
    fs::write(root.join("metrics.json"), "{\"legacy\":true}").unwrap();
    fs::write(root.join("config.toml"), DASHBOARD_OFF).unwrap();
    fs::create_dir_all(env.cwd.join(".config/epic-harness")).unwrap();
    fs::write(
        env.cwd.join(".config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    let stderr = env.run("startup", &env.cwd, None);
    assert!(!stderr.contains(HINT), "{stderr}");
    assert_eq!(fs::read(root.join("obs/session_1.jsonl")).unwrap(), b"{}\n");
    assert_eq!(
        fs::read(root.join("metrics.json")).unwrap(),
        b"{\"legacy\":true}"
    );
}
