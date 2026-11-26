//! agy (Antigravity CLI) hook contract. SessionStart: stdout is exactly one
//! strict-protojson object (`{}` or `injectSteps`). Tool events: guard prints
//! one `{"decision":"deny","reason":...}` line on a block and nothing on an
//! allow (agy reads any other stdout object, even `{}`, as a deny), observe
//! prints nothing and reads the outcome from the root `error` string, and Stop
//! runs reflect once per batch. In every case the project slug comes from the
//! agy workspace, not from the plugin-root cwd the hook runs in. Every run uses
//! a temp HOME, HARNESS_DIR and cwd; all payloads are synthetic.
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
        let (code, stdout, _) = self.run_code(sub, payload);
        (code == 0, stdout)
    }

    /// Like `run`, returning (exit code, stdout, stderr).
    fn run_code(&self, sub: &str, payload: &Value) -> (i32, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .arg(sub)
            .current_dir(self.root.join("plugin_root"))
            .env("HOME", self.root.join("home"))
            .env("HARNESS_DIR", self.harness())
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    }

    /// Blocks SQLite (a directory where harness.db goes) so observe falls back
    /// to the per-conversation JSONL under the workspace slug.
    fn jsonl_only(&self) {
        fs::create_dir_all(self.harness().join("projects/agy_proj/obs")).unwrap();
        fs::create_dir(self.harness().join("harness.db")).unwrap();
    }

    fn obs_file(&self) -> PathBuf {
        self.harness()
            .join("projects/agy_proj/obs/session_conv-agy-0001.jsonl")
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

/// agy tool event shaped like the captured ones; `error` only on PostToolUse.
fn agy_tool(
    env: &Env,
    event: &str,
    step: u32,
    name: &str,
    args: Value,
    error: Option<&str>,
) -> Value {
    let mut p = agy_payload(env, "conv-agy-0001", event);
    p["lastUserInput"] = json!("SENTINEL-USER-INPUT");
    p["stepIdx"] = json!(step);
    p["toolCall"] = json!({"id": "call_0000000000000000001", "name": name, "args": args});
    if let Some(e) = error {
        p["error"] = json!(e);
    }
    p
}

fn agy_stop(env: &Env) -> Value {
    let mut p = agy_payload(env, "conv-agy-0001", "Stop");
    p["continuationActive"] = json!(false);
    p["error"] = json!("");
    p["executionNum"] = json!(1);
    p["fullyIdle"] = json!(true);
    p["lastUserInput"] = json!("SENTINEL-USER-INPUT");
    p["terminationReason"] = json!("<synthetic>");
    p
}

fn shell_args(command: &str) -> Value {
    json!({
        "CommandLine": command,
        "WaitMsBeforeAsync": 5000,
        "toolSummary": format!("Run shell command {command}"),
    })
}

fn pre_shell(env: &Env, command: &str) -> Value {
    agy_tool(
        env,
        "PreToolUse",
        1,
        "run_command",
        shell_args(command),
        None,
    )
}

fn post_shell(env: &Env, command: &str) -> Value {
    agy_tool(
        env,
        "PostToolUse",
        1,
        "run_command",
        shell_args(command),
        Some(""),
    )
}

fn post_read(env: &Env, path: &Path, error: &str) -> Value {
    agy_tool(
        env,
        "PostToolUse",
        2,
        "read_file",
        json!({"Path": path}),
        Some(error),
    )
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
    // banner rides the same single systemMessage, never extra stdout.
    let msg = inner["systemMessage"].as_str().unwrap();
    assert!(!msg.is_empty() && msg.contains("[resume]"), "{out}");
    assert!(msg.contains("[resume] Initialized"), "{out}");

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

fn only_deny(stdout: &str) -> Value {
    assert!(
        stdout.ends_with('\n') && stdout.trim_end().lines().count() == 1,
        "{stdout:?}"
    );
    let out: Value = serde_json::from_str(stdout).unwrap();
    let mut keys: Vec<_> = out.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["decision", "reason"]);
    assert_eq!(out["decision"], "deny");
    out
}

#[test]
fn agy_guard_block_is_one_deny_object() {
    let env = Env::new();
    let (code, stdout, stderr) = env.run_code("guard", &pre_shell(&env, "rm -rf /"));
    assert_eq!(code, 0, "{stderr}");
    let out = only_deny(&stdout);
    let reason = out["reason"].as_str().unwrap();
    assert!(reason.contains("BLOCKED: rm -rf / blocked"), "{reason}");
    assert!(stderr.contains("BLOCKED: rm -rf / blocked"), "{stderr}");
}

/// A workspace rules file; the harness project dir must exist for it to load.
fn write_probe_rule(env: &Env, workspace: &Path) {
    let name = workspace.file_name().unwrap();
    fs::create_dir_all(env.harness().join("projects").join(name)).unwrap();
    fs::create_dir_all(workspace.join(".harness")).unwrap();
    fs::write(
        workspace.join(".harness/guard-rules.yaml"),
        "blocked:\n  - pattern: epic-guard-probe | msg: probe rule blocked\n",
    )
    .unwrap();
}

#[test]
fn agy_guard_custom_rule_block() {
    let env = Env::new();
    write_probe_rule(&env, &env.proj());
    let (code, stdout, stderr) = env.run_code("guard", &pre_shell(&env, "echo epic-guard-probe"));
    assert_eq!(code, 0, "{stderr}");
    let out = only_deny(&stdout);
    assert!(
        out["reason"]
            .as_str()
            .unwrap()
            .contains("BLOCKED: probe rule blocked"),
        "{out}"
    );
}

#[test]
fn agy_guard_allow_prints_nothing() {
    let env = Env::new();
    let existing = env.proj().join("existing.txt");
    let pre = |name: &str, args: Value| agy_tool(&env, "PreToolUse", 1, name, args, None);
    let payloads = [
        pre_shell(&env, "true"),
        pre_shell(&env, "false"),
        pre_shell(&env, "echo hi"),
        // Builtin warn rule: the warning stays on stderr.
        pre_shell(&env, "rm -rf build/"),
        pre("read_file", json!({"Path": existing})),
        pre(
            "edit_file",
            json!({"Path": existing, "Replacement": "SENTINEL-REPLACEMENT", "Target": "SENTINEL-TARGET"}),
        ),
        pre("exit_plan_mode", json!({"plan": "synthetic plan text"})),
        pre("run_command", json!({"WaitMsBeforeAsync": 5000})),
        pre("run_command", json!({"CommandLine": 7})),
    ];
    for payload in &payloads {
        let (code, stdout, stderr) = env.run_code("guard", payload);
        assert_eq!(code, 0, "{payload}: {stderr}");
        assert_eq!(stdout, "", "{payload}");
    }
}

#[test]
fn agy_guard_rules_follow_workspace() {
    let env = Env::new();
    // Rules live under the workspace only; the hook cwd (plugin root) has none.
    write_probe_rule(&env, &env.proj());
    let (_, stdout, _) = env.run_code("guard", &pre_shell(&env, "echo epic-guard-probe"));
    only_deny(&stdout);

    // Same command from another workspace without rules is allowed.
    let other = env.root.join("other_proj");
    fs::create_dir_all(env.harness().join("projects/other_proj")).unwrap();
    fs::create_dir_all(&other).unwrap();
    let mut payload = pre_shell(&env, "echo epic-guard-probe");
    payload["workspacePaths"] = json!([other]);
    payload["cwd"] = json!(other);
    let (code, stdout, _) = env.run_code("guard", &payload);
    assert_eq!((code, stdout.as_str()), (0, ""));
}

/// Runs observe once with SQLite blocked; returns (record, raw JSONL line).
fn observe_one(env: &Env, payload: &Value) -> (Value, String) {
    env.jsonl_only();
    let (code, stdout, stderr) = env.run_code("observe", payload);
    assert_eq!((code, stdout.as_str()), (0, ""), "{stderr}");
    let raw = fs::read_to_string(env.obs_file()).unwrap();
    assert_eq!(raw.lines().count(), 1, "{raw}");
    (serde_json::from_str(raw.trim_end()).unwrap(), raw)
}

#[test]
fn agy_observe_shell_success() {
    let env = Env::new();
    let (rec, _) = observe_one(&env, &post_shell(&env, "true"));
    assert_eq!(rec["tool"], "run_command");
    assert_eq!(rec["tool_category"], "bash");
    assert_eq!(rec["result"], "success");
    assert!(rec["failure_category"].is_null());
    assert_eq!(rec["score"], 1.0);
    assert_eq!(rec["action"], "true");
}

/// Pins Decision 2 (root `error` only): agy reports `error: ""` for a shell
/// command that exits non-zero, so it is recorded as a success. A later change
/// (transcript lookup) must be deliberate.
#[test]
fn agy_observe_nonzero_exit_is_invisible() {
    let env = Env::new();
    let (rec, _) = observe_one(&env, &post_shell(&env, "false"));
    assert_eq!(rec["result"], "success");
}

#[test]
fn agy_observe_missing_file_is_failure() {
    let env = Env::new();
    let missing = env.proj().join("does_not_exist.txt");
    let (rec, _) = observe_one(&env, &post_read(&env, &missing, "file does not exist"));
    assert_eq!(rec["tool_category"], "read");
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert_eq!(rec["score"], 0.35);
    assert!(
        rec["error_snippet"]
            .as_str()
            .unwrap()
            .contains("file does not exist"),
        "{rec}"
    );
    assert_eq!(rec["file_ext"], ".txt");
}

#[test]
fn agy_observe_read_success() {
    let env = Env::new();
    let (rec, _) = observe_one(&env, &post_read(&env, &env.proj().join("existing.txt"), ""));
    assert_eq!(rec["result"], "success");
    assert_eq!(rec["score"], 1.0);
}

#[test]
fn agy_observe_edit_never_records_content() {
    let env = Env::new();
    let payload = agy_tool(
        &env,
        "PostToolUse",
        3,
        "edit_file",
        json!({
            "Path": env.proj().join("existing.txt"),
            "Replacement": "SENTINEL-REPLACEMENT",
            "Target": "SENTINEL-TARGET",
        }),
        Some(""),
    );
    let (rec, raw) = observe_one(&env, &payload);
    assert_eq!(rec["tool_category"], "edit");
    assert_eq!(rec["result"], "success");
    let transcript = env.root.join("transcript.jsonl");
    for secret in [
        "SENTINEL-REPLACEMENT",
        "SENTINEL-TARGET",
        "SENTINEL-USER-INPUT",
        transcript.to_str().unwrap(),
    ] {
        assert!(!raw.contains(secret), "record leaks {secret}: {raw}");
    }
}

#[test]
fn agy_observe_unknown_tool() {
    let env = Env::new();
    let payload = agy_tool(
        &env,
        "PostToolUse",
        4,
        "exit_plan_mode",
        json!({"plan": "synthetic plan text"}),
        Some(""),
    );
    let (rec, _) = observe_one(&env, &payload);
    assert_eq!(rec["tool_category"], "other");
    assert_eq!(rec["result"], "success");
}

#[test]
fn agy_observe_files_under_workspace_and_conversation() {
    let env = Env::new();
    observe_one(&env, &post_shell(&env, "true"));
    assert!(env.obs_file().is_file());
    assert!(!env.harness().join("projects/plugin_root").exists());
}

/// SessionStart first, as agy fires it for a new conversation: it creates the
/// project dir, without which reflect returns before reading anything. Then
/// three observations (true, read success, missing read) via SQLite.
fn observe_three(env: &Env) {
    let (ok, _) = env.run("resume", &agy_payload(env, "conv-agy-0001", "SessionStart"));
    assert!(ok);
    fs::write(env.proj().join("existing.txt"), "synthetic").unwrap();
    let missing = env.proj().join("does_not_exist.txt");
    for payload in [
        post_shell(env, "true"),
        post_read(env, &env.proj().join("existing.txt"), ""),
        post_read(env, &missing, "file does not exist"),
    ] {
        let (code, stdout, stderr) = env.run_code("observe", &payload);
        assert_eq!((code, stdout.as_str()), (0, ""), "{stderr}");
    }
}

#[test]
fn agy_stop_runs_reflect_once_per_batch() {
    let env = Env::new();
    observe_three(&env);
    let cursor = env.harness().join("projects/agy_proj/reflect_cursor.json");
    assert!(!cursor.exists());

    let (code, stdout, stderr) = env.run_code("reflect", &agy_stop(&env));
    assert_eq!((code, stdout.as_str()), (0, "{}\n"), "{stderr}");
    let first = fs::read(&cursor).expect("reflect_cursor.json after the first Stop");

    let (code, stdout, stderr) = env.run_code("reflect", &agy_stop(&env));
    assert_eq!((code, stdout.as_str()), (0, "{}\n"), "{stderr}");
    assert_eq!(fs::read(&cursor).unwrap(), first, "second Stop is a no-op");
}

#[test]
fn agy_stop_with_too_few_observations() {
    let env = Env::new();
    let (ok, _) = env.run(
        "resume",
        &agy_payload(&env, "conv-agy-0001", "SessionStart"),
    );
    assert!(ok);
    for payload in [post_shell(&env, "true"), post_shell(&env, "echo hi")] {
        let (code, _, stderr) = env.run_code("observe", &payload);
        assert_eq!(code, 0, "{stderr}");
    }
    let (code, stdout, stderr) = env.run_code("reflect", &agy_stop(&env));
    assert_eq!((code, stdout.as_str()), (0, "{}\n"), "{stderr}");
    assert!(
        !env.harness()
            .join("projects/agy_proj/reflect_cursor.json")
            .exists()
    );
}
