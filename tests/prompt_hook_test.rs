//! `epic-harness prompt` (UserPromptSubmit, ADR 0008): output contract, privacy,
//! read-only recall, and fail-open behavior. Every run uses a temp HOME,
//! HARNESS_DIR and cwd, and environment variables go on the child only; the
//! real `~/.harness` is never opened.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SLUG: &str = "fixture_project";
const BIN: &str = env!("CARGO_BIN_EXE_epic-harness");
const MARKER: &str = "PRIVATE-MARKER-123";

/// The first exec of the binary from a cargo-launched test process can stall
/// for 1 to 2 s inside dyld before `main` runs (measured on macOS: `sample`
/// shows the child in `_dyld_start`; the second exec takes about 8 ms). That is
/// OS launch cost, not hook latency, so absorb it once, untimed, before any
/// test measures wall time.
fn warm_up_binary() {
    static WARM: std::sync::Once = std::sync::Once::new();
    WARM.call_once(|| {
        let _ = Command::new(BIN)
            .arg("version")
            .stdout(Stdio::null())
            .status();
    });
}

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Env {
    /// `config` is the `[prompt]` body; `project_dir` pre-creates
    /// `HARNESS_DIR/projects/<slug>` as SessionStart would have.
    fn new(config: &str, project_dir: bool) -> Self {
        warm_up_binary();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        fs::create_dir_all(root.join(SLUG)).unwrap();
        fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
        fs::write(
            root.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
        fs::create_dir_all(root.join("harness-root")).unwrap();
        fs::write(
            root.join("harness-root/config.toml"),
            format!("[prompt]\n{config}\n"),
        )
        .unwrap();
        if project_dir {
            fs::create_dir_all(root.join("harness-root/projects").join(SLUG)).unwrap();
        }
        Self { _temp: temp, root }
    }

    fn enabled() -> Self {
        Self::new("enabled = true", true)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .current_dir(self.root.join(SLUG))
            .env("HOME", self.root.join("home"))
            .env("HARNESS_DIR", self.root.join("harness-root"))
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env_remove("HARNESS_ROOT")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("HARNESS_DB_URL")
            .env_remove("EPIC_HOOK_PROFILE")
            .env_remove("GROK_HOOK_EVENT");
        cmd
    }

    fn run(&self, mut cmd: Command, stdin: &str) -> std::process::Output {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // The child may exit before reading a large payload; ignore EPIPE.
        let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
        child.wait_with_output().unwrap()
    }

    fn mem_add(&self, project: &str, title: &str, body: &str) {
        let out = self.run(
            self.command(&[
                "mem",
                "add",
                "--title",
                title,
                "--type",
                "decision",
                "--project",
                project,
                "--body",
                body,
            ]),
            "",
        );
        assert!(out.status.success(), "{out:?}");
    }

    fn hook(&self, prompt: &str) -> std::process::Output {
        self.hook_with(self.command(&["prompt"]), &claude_payload(prompt))
    }

    fn hook_with(&self, cmd: Command, stdin: &str) -> std::process::Output {
        let out = self.run(cmd, stdin);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        out
    }

    fn project_dir(&self) -> PathBuf {
        self.root.join("harness-root/projects").join(SLUG)
    }

    fn memory_db(&self) -> PathBuf {
        self.root.join("harness-root/memory.db")
    }

    fn dispatch_lines(&self) -> Vec<serde_json::Value> {
        let dir = self.project_dir().join("dispatch");
        let mut lines = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            for line in fs::read_to_string(entry.unwrap().path()).unwrap().lines() {
                lines.push(serde_json::from_str(line).unwrap());
            }
        }
        lines
    }
}

fn claude_payload(prompt: &str) -> String {
    serde_json::json!({
        "session_id": "s-1",
        "transcript_path": "/tmp/synthetic/t.jsonl",
        "cwd": "/tmp/synthetic/proj",
        "permission_mode": "default",
        "hook_event_name": "UserPromptSubmit",
        "prompt": prompt,
    })
    .to_string()
}

fn context_of(out: &std::process::Output) -> String {
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string()
}

/// True when any file under `dir` contains `needle`.
fn tree_contains(dir: &Path, needle: &str) -> bool {
    fs::read_dir(dir).unwrap().any(|e| {
        let path = e.unwrap().path();
        if path.is_dir() {
            tree_contains(&path, needle)
        } else {
            fs::read(&path)
                .map(|b| b.windows(needle.len()).any(|w| w == needle.as_bytes()))
                .unwrap_or(false)
        }
    })
}

#[test]
fn pr_claude_output_contract() {
    let env = Env::enabled();
    env.mem_add(
        SLUG,
        "Refresh policy NONCE-7Q2",
        "kestrel rotates refresh tokens",
    );
    let out = env.hook("why does the kestrel login fail");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let top = v.as_object().unwrap();
    assert_eq!(top.len(), 1, "{v}");
    let inner = top["hookSpecificOutput"].as_object().unwrap();
    assert_eq!(inner.len(), 2, "{v}");
    assert_eq!(inner["hookEventName"], "UserPromptSubmit");
    let s = inner["additionalContext"].as_str().unwrap();
    assert!(s.contains("NONCE-7Q2"), "{s}");
    assert!(s.contains("debug") && s.contains("secure"), "{s}");
    assert!(s.chars().count() <= 1500, "{s}");
}

#[test]
fn pr_no_match_is_silent() {
    let env = Env::enabled();
    let out = env.hook("hello there");
    assert!(out.stdout.is_empty(), "{out:?}");
    let lines = env.dispatch_lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["selected_skills"], serde_json::json!([]));
}

#[test]
fn pr_disabled_is_silent_and_inert() {
    let env = Env::new("enabled = false", false);
    let started = Instant::now();
    let out = env.hook("why does the kestrel login fail");
    // Generous: the disabled path takes milliseconds once the binary is warm.
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(!env.root.join("harness-root/projects").exists());
    assert!(!env.memory_db().exists());
}

#[test]
fn pr_default_config_is_disabled() {
    let env = Env::new("", true);
    let out = env.hook("why does the kestrel login fail");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(!env.project_dir().join("dispatch").exists());
}

#[test]
fn pr_minimal_profile_disables() {
    let env = Env::enabled();
    let mut cmd = env.command(&["prompt"]);
    cmd.env("EPIC_HOOK_PROFILE", "minimal");
    let out = env.hook_with(cmd, &claude_payload("why does the kestrel login fail"));
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(!env.project_dir().join("dispatch").exists());
    assert!(!env.memory_db().exists());
}

#[test]
fn pr_never_echoes_prompt() {
    let env = Env::enabled();
    // Manual run: no hook_event_name, so main takes the passthrough branch.
    let payload = serde_json::json!({"prompt": format!("{MARKER} fix the failing auth test")});
    let out = env.hook_with(env.command(&["prompt"]), &payload.to_string());
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains(MARKER));
    assert!(!tree_contains(&env.root, MARKER));
}

#[test]
fn pr_dispatch_log_has_no_prompt_text() {
    let env = Env::enabled();
    env.mem_add(
        SLUG,
        "Refresh policy NONCE-7Q2",
        "kestrel rotates refresh tokens",
    );
    let out = env.hook(&format!("{MARKER} kestrel zebrafish login fail"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains(MARKER));
    let lines = env.dispatch_lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let obj = lines[0].as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want = [
        "timestamp",
        "source",
        "trigger_signal",
        "selected_skills",
        "context_hint",
        "recalled",
        "memory_ids",
        "delivered_chars",
        "truncated",
        "timed_out",
        "elapsed_ms",
        "session",
    ];
    want.sort_unstable();
    assert_eq!(keys, want);
    assert_eq!(obj["source"], "prompt_hook");
    assert_eq!(obj["trigger_signal"], "user_prompt_submit");
    assert_eq!(
        obj["selected_skills"],
        serde_json::json!(["debug", "secure"])
    );
    assert_eq!(obj["context_hint"], "debug.failure,secure.sensitive");
    assert_eq!(obj["recalled"], 1);
    assert_eq!(obj["session"], "s-1");

    let log = fs::read_to_string(
        fs::read_dir(env.project_dir().join("dispatch"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap()
    .to_lowercase();
    for needle in [
        "private",
        "marker",
        "kestrel",
        "zebrafish",
        "login",
        "nonce-7q2",
        "/tmp",
    ] {
        assert!(!log.contains(needle), "dispatch log leaks {needle}: {log}");
    }
    assert!(!tree_contains(&env.root, MARKER));
}

#[test]
fn pr_recall_is_read_only() {
    let env = Env::enabled();
    env.mem_add(
        SLUG,
        "Refresh policy NONCE-7Q2",
        "kestrel rotates refresh tokens",
    );
    let before = fs::read(env.memory_db()).unwrap();
    let out = env.hook("why does the kestrel login fail");
    // The node was found, so the read path ran against the real file.
    assert!(context_of(&out).contains("NONCE-7Q2"));
    assert_eq!(fs::read(env.memory_db()).unwrap(), before);
}

#[test]
fn pr_project_scope() {
    let env = Env::enabled();
    env.mem_add(
        "other_project",
        "Refresh policy NONCE-7Q2",
        "kestrel rotates refresh tokens",
    );
    let out = env.hook("why does the kestrel login fail");
    let s = context_of(&out);
    assert!(s.contains("debug"), "{s}");
    assert!(!s.contains("- ["), "{s}");
    assert!(!s.contains("NONCE-7Q2"), "{s}");
}

/// Holds an exclusive lock on memory.db so any other connection stalls in
/// SQLite's busy wait (sqlx default 5 s) until the hook's own cap fires. A
/// Postgres URL cannot stand in: this build has no Postgres driver, so it
/// fails at once instead of hanging. Keep the returned values alive.
fn lock_memory_db(db: &Path) -> (tokio::runtime::Runtime, sqlx::AnyPool) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(async {
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db.display()))
            .await
            .unwrap();
        sqlx::query("PRAGMA locking_mode=EXCLUSIVE")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("BEGIN EXCLUSIVE").execute(&pool).await.unwrap();
        pool
    });
    (rt, pool)
}

#[test]
fn pr_timeout_fails_open() {
    let env = Env::new("enabled = true\ntimeout_ms = 100", true);
    env.mem_add(
        SLUG,
        "Refresh policy NONCE-7Q2",
        "kestrel rotates refresh tokens",
    );
    let _lock = lock_memory_db(&env.memory_db());
    let started = Instant::now();
    let out = env.hook("why does the kestrel login fail");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let s = context_of(&out);
    assert!(s.contains("debug") && !s.contains("- ["), "{s}");
    let lines = env.dispatch_lines();
    assert_eq!(lines[0]["timed_out"], true, "{lines:?}");
}

#[test]
fn pr_malformed_stdin() {
    let env = Env::enabled();
    for stdin in ["not json", "{}", ""] {
        let out = env.hook_with(env.command(&["prompt"]), stdin);
        assert!(out.stdout.is_empty(), "{stdin:?}: {out:?}");
    }
}

#[test]
fn pr_no_memory_db_not_created() {
    let env = Env::enabled();
    let out = env.hook("fix the failing auth test");
    let s = context_of(&out);
    assert!(
        s.contains("debug") && s.contains("secure") && !s.contains("- ["),
        "{s}"
    );
    assert!(!env.memory_db().exists());
}

#[test]
fn pr_hostile_prompts_still_exit_zero() {
    let env = Env::enabled();
    for prompt in [
        "x".repeat(1_000_000),
        "\u{1}\u{7}\u{1b}".repeat(500),
        "<pasted_content id=\"1\">\nfix the failing auth test".to_string(),
    ] {
        let out = env.hook(&prompt);
        assert!(out.stdout.is_empty(), "{out:?}");
    }
}
