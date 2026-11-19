//! agy (Antigravity CLI) SessionStart hook contract: stdout is exactly one
//! strict-protojson object (`{}` or `injectSteps`), and the project slug comes
//! from the agy workspace, not from the plugin-root cwd the hook runs in. Every
//! run uses a temp HOME, HARNESS_DIR and cwd; all payloads are synthetic.
#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        fs::create_dir_all(root.join("plugin_root")).unwrap();
        fs::create_dir_all(root.join("agy_proj")).unwrap();
        fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
        fs::write(
            root.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
        Env { _temp: temp, root }
    }

    fn harness(&self) -> PathBuf {
        self.root.join("harness-root")
    }

    fn proj(&self) -> PathBuf {
        self.root.join("agy_proj")
    }

    /// Runs `<sub>` with `payload` on stdin from the plugin-root cwd; returns
    /// (exit success, stdout).
    fn run(&self, sub: &str, payload: &Value) -> (bool, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg(sub)
            .current_dir(self.root.join("plugin_root"))
            .env("HOME", self.root.join("home"))
            .env("HARNESS_DIR", self.harness())
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (out.status.success(), String::from_utf8(out.stdout).unwrap())
    }
}

fn agy_payload(env: &Env, conversation: &str, event: &str) -> Value {
    json!({
        "artifactDirectoryPath": env.root.join("artifacts"),
        "conversationId": conversation,
        "cwd": env.proj(),
        "hookEventName": event,
        "modelName": "synthetic-model",
        "transcriptPath": env.root.join("transcript.jsonl"),
        "workspacePaths": [env.proj()],
    })
}

#[test]
fn agy_resume_injects_and_uses_workspace_slug() {
    let env = Env::new();
    let (ok, stdout) = env.run("resume", &agy_payload(&env, "conv-0001", "SessionStart"));
    assert!(ok);

    let out: Value = serde_json::from_str(&stdout).expect("one JSON object");
    let obj = out.as_object().unwrap();
    assert_eq!(obj.keys().collect::<Vec<_>>(), ["injectSteps"]);
    let steps = obj["injectSteps"].as_array().unwrap();
    assert_eq!(steps.len(), 1, "{out}");
    let step = steps[0].as_object().unwrap();
    assert_eq!(step.keys().collect::<Vec<_>>(), ["systemMessage"]);
    let inner = step["systemMessage"].as_object().unwrap();
    assert_eq!(inner.keys().collect::<Vec<_>>(), ["systemMessage"]);
    // The temp HOME makes the default-team hint deterministic. The first-run
    // banner is suppressed by the startup lock ordering in resume.rs (pre-existing
    // on all hosts, tracked separately).
    let msg = inner["systemMessage"].as_str().unwrap();
    assert!(!msg.is_empty() && msg.contains("[resume]"), "{out}");

    // The chdir must beat the cwd-derived slug LazyLock.
    assert!(env.harness().join("projects/agy_proj").exists());
    assert!(!env.harness().join("projects/plugin_root").exists());
}

#[test]
fn agy_resume_second_call_same_conversation_is_empty_object() {
    let env = Env::new();
    let payload = agy_payload(&env, "conv-0002", "SessionStart");
    let (_, first) = env.run("resume", &payload);
    assert!(first.contains("injectSteps"), "{first}");
    let (ok, second) = env.run("resume", &payload);
    assert!(ok);
    assert_eq!(second, "{}\n");
}

/// Stdout must be one object whose keys the agy protojson schema accepts.
fn assert_one_agy_object(stdout: &str) {
    let out: Value = serde_json::from_str(stdout).unwrap_or_else(|e| panic!("{e}: {stdout:?}"));
    assert!(stdout.ends_with('\n') && stdout.trim_end().lines().count() == 1);
    for key in out.as_object().expect("object").keys() {
        assert_eq!(key, "injectSteps", "unexpected key in {stdout}");
    }
}

#[test]
fn agy_stdout_is_always_one_json_object() {
    let missing = |env: &Env| {
        let mut p = agy_payload(env, "conv-0003", "SessionStart");
        p["workspacePaths"] = json!([env.root.join("no_such_dir")]);
        p["cwd"] = json!(env.root.join("no_such_dir"));
        p
    };
    let mismatch = |env: &Env| {
        let mut p = agy_payload(env, "conv-0004", "SessionStart");
        p["source"] = json!(5);
        p
    };
    let fresh = |env: &Env| agy_payload(env, "conv-0005", "SessionStart");
    type Variant = fn(&Env) -> Value;
    let variants: [(&str, Variant); 3] = [
        ("workspace missing", missing),
        ("type mismatch", mismatch),
        ("fresh", fresh),
    ];

    for sub in ["resume", "reflect"] {
        for (name, build) in variants {
            // Fresh harness, then the same payload again on an existing one.
            let env = Env::new();
            for round in ["fresh harness", "existing harness"] {
                let (ok, stdout) = env.run(sub, &build(&env));
                assert!(ok, "{sub} {name} {round}");
                assert_one_agy_object(&stdout);
            }
        }
    }
}

#[test]
fn claude_resume_still_echoes_stdin() {
    let env = Env::new();
    let payload = json!({"session_id": "s", "source": "startup"});
    let (ok, stdout) = env.run("resume", &payload);
    assert!(ok);
    assert_eq!(stdout, payload.to_string());
}

#[test]
fn codex_reflect_still_prints_continue() {
    let env = Env::new();
    let (ok, stdout) = env.run(
        "reflect",
        &json!({"session_id": "s", "hook_event_name": "Stop"}),
    );
    assert!(ok);
    assert_eq!(stdout, "{\"continue\":true}\n");
}

#[test]
fn grok_both_event_keys_keep_codex_branch() {
    let env = Env::new();
    let payload = json!({
        "conversationId": "conv-0006",
        "hookEventName": "stop",
        "hook_event_name": "Stop",
        "session_id": "s",
    });
    let (ok, stdout) = env.run("reflect", &payload);
    assert!(ok);
    assert_eq!(stdout, "{\"continue\":true}\n");
    assert!(!Path::new(&env.harness().join("projects/agy_proj")).exists());
}
