//! `PostToolUseFailure` (Claude Code and grok) is the only event that carries a
//! failed tool call: `PostToolUse` fires on success only. Every run uses a temp
//! HOME, HARNESS_DIR and cwd, with SQLite blocked so the JSONL fallback is
//! read back; all payloads are synthetic.
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";
const BASH_ERROR: &str = "Exit code 1\nerror[E0425]: cannot find value `x` in this scope";

struct Fixture {
    _temp: tempfile::TempDir,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::create_dir_all(harness.join("projects").join(SLUG).join("obs")).unwrap();
        fs::create_dir(harness.join("harness.db")).unwrap();
        Self {
            _temp: temp,
            cwd,
            home,
            harness,
        }
    }

    fn project_dir(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG)
    }

    fn observe(&self, payload: serde_json::Value, env: &[(&str, &str)]) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        cmd.arg("observe")
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
            .env_remove("GROK_HOOK_EVENT")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    }

    /// Every observation written so far, in file order.
    fn records(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for entry in fs::read_dir(self.project_dir().join("obs")).unwrap() {
            let path = entry.unwrap().path();
            // Skip the telemetry sampling counter that shares the directory.
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let text = fs::read_to_string(path).unwrap();
            out.extend(text.lines().map(|l| serde_json::from_str(l).unwrap()));
        }
        out
    }

    fn single_record(&self) -> serde_json::Value {
        let mut records = self.records();
        assert_eq!(records.len(), 1, "{records:?}");
        records.remove(0)
    }

    fn agent_statuses(&self) -> Vec<String> {
        let raw = fs::read_to_string(self.project_dir().join("orchestrator/run.json")).unwrap();
        let run: serde_json::Value = serde_json::from_str(&raw).unwrap();
        run["agents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["status"].as_str().unwrap().to_string())
            .collect()
    }
}

fn failure(tool: &str, input: serde_json::Value, error: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PostToolUseFailure",
        "session_id": "synthetic-session",
        "tool_name": tool,
        "tool_input": input,
        "error": error,
    })
}

fn bash_failure(error: &str) -> serde_json::Value {
    failure(
        "Bash",
        serde_json::json!({"command": "cargo test"}),
        error.into(),
    )
}

/// Default score weights are 0.5 / 0.3 / 0.2 and `tool_success` is 0, so the
/// score is `0.3 * failure_quality + 0.2`.
#[test]
fn bash_failure_is_recorded_with_classified_category() {
    let fx = Fixture::new();
    fx.observe(bash_failure(BASH_ERROR), &[]);
    let rec = fx.single_record();
    assert_eq!(rec["tool"], "Bash");
    assert_eq!(rec["result"], "error");
    // `classify_failure` has no rule for this text (None), so the category is
    // the `runtime_error` fallback.
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(
        (rec["score"].as_f64().unwrap() - 0.35).abs() < 1e-9,
        "{rec}"
    );
    assert!(
        rec["error_snippet"]
            .as_str()
            .is_some_and(|s| s.starts_with("Exit code 1")),
        "{rec}"
    );
    assert_eq!(rec["dimensions"]["tool_success"], 0.0);
}

/// The classifier verdict wins over the fallback when a rule matches.
#[test]
fn classifier_category_and_golden_scores() {
    for (error, category, score) in [
        ("SyntaxError: Unexpected token", "syntax_error", 0.29),
        ("ENOENT: no such file or directory", "not_found", 0.38),
        ("test result: FAILED. 0 passed; 1 failed", "test_fail", 0.38),
    ] {
        let fx = Fixture::new();
        fx.observe(bash_failure(error), &[]);
        let rec = fx.single_record();
        assert_eq!(rec["failure_category"], category, "{rec}");
        assert!(
            (rec["score"].as_f64().unwrap() - score).abs() < 1e-9,
            "{rec}"
        );
    }
}

#[test]
fn read_not_found_falls_back_to_runtime_error() {
    let fx = Fixture::new();
    fx.observe(
        failure(
            "Read",
            serde_json::json!({"file_path": "src/missing.rs"}),
            "File does not exist.".into(),
        ),
        &[],
    );
    let rec = fx.single_record();
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(
        (rec["score"].as_f64().unwrap() - 0.35).abs() < 1e-9,
        "{rec}"
    );
}

#[test]
fn interrupted_call_writes_no_observation() {
    let fx = Fixture::new();
    let mut payload = bash_failure(BASH_ERROR);
    payload["is_interrupt"] = serde_json::json!(true);
    fx.observe(payload, &[]);
    assert!(fx.records().is_empty());
}

#[test]
fn false_is_interrupt_is_still_a_failure() {
    let fx = Fixture::new();
    let mut payload = bash_failure(BASH_ERROR);
    payload["is_interrupt"] = serde_json::json!(false);
    fx.observe(payload, &[]);
    assert_eq!(fx.single_record()["result"], "error");
}

#[test]
fn secrets_in_the_error_text_are_masked() {
    let fx = Fixture::new();
    fx.observe(
        bash_failure("Exit code 1\ncurl -H \"Authorization: Bearer sk-abc123XYZ\" failed"),
        &[],
    );
    let snippet = fx.single_record()["error_snippet"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!snippet.contains("sk-abc123XYZ"), "{snippet}");
    assert!(snippet.contains("<REDACTED>"), "{snippet}");
}

/// A typed `error` field would fail to parse here and drop the whole payload,
/// recording the tool as "unknown".
#[test]
fn non_string_error_keeps_the_payload() {
    let fx = Fixture::new();
    fx.observe(
        failure(
            "Bash",
            serde_json::json!({"command": "false"}),
            serde_json::json!({"code": 1}),
        ),
        &[],
    );
    let rec = fx.single_record();
    assert_eq!(rec["tool"], "Bash");
    assert_eq!(rec["result"], "error");
    assert_eq!(rec["failure_category"], "runtime_error");
    assert!(rec["error_snippet"].is_null(), "{rec}");
}

#[test]
fn grok_camel_case_payload_is_recorded() {
    let fx = Fixture::new();
    fx.observe(
        serde_json::json!({
            "hookEventName": "PostToolUseFailure",
            "sessionId": "synthetic-session",
            "toolName": "Bash",
            "toolInput": {"command": "false"},
            "error": "Exit code 1",
        }),
        &[],
    );
    let rec = fx.single_record();
    assert_eq!(rec["tool"], "Bash");
    assert_eq!(rec["result"], "error");
}

/// Telemetry consent is "off" in the fixture: no sampling counter may be written.
#[test]
fn no_telemetry_counter_file_when_consent_is_off() {
    let fx = Fixture::new();
    fx.observe(bash_failure(BASH_ERROR), &[]);
    assert_eq!(fx.single_record()["result"], "error");
    let counters = fs::read_dir(fx.project_dir().join("obs"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("telemetry_error_count_")
        })
        .count();
    assert_eq!(counters, 0);
}

/// Exit code of `guard` for one payload.
fn guard_exit(fx: &Fixture, payload: serde_json::Value) -> i32 {
    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("guard")
        .current_dir(&fx.cwd)
        .env("HOME", &fx.home)
        .env("HARNESS_DIR", &fx.harness)
        .env("GIT_CEILING_DIRECTORIES", fx.cwd.parent().unwrap())
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
    child.wait().unwrap().code().unwrap()
}

/// Payload normalization is observe-only in 0.9.2: guard must see the same
/// (empty) input on a camelCase grok payload as before.
#[test]
fn grok_camel_case_payload_is_not_normalized_for_guard() {
    let fx = Fixture::new();
    let command = "DROP TABLE prod_users";
    let snake = serde_json::json!({"tool_name": "Bash", "tool_input": {"command": command}});
    assert_eq!(
        guard_exit(&fx, snake),
        2,
        "control: guard blocks this command"
    );
    let camel = serde_json::json!({"toolName": "Bash", "toolInput": {"command": command}});
    assert_eq!(guard_exit(&fx, camel), 0);
}

#[test]
fn event_name_from_env_is_recorded() {
    let fx = Fixture::new();
    fx.observe(
        serde_json::json!({
            "session_id": "synthetic-session",
            "tool_name": "Bash",
            "tool_input": {"command": "false"},
            "error": "Exit code 1",
        }),
        &[("GROK_HOOK_EVENT", "PostToolUseFailure")],
    );
    assert_eq!(fx.single_record()["result"], "error");
}

#[test]
fn post_tool_use_success_is_unchanged() {
    let fx = Fixture::new();
    fx.observe(
        serde_json::json!({
            "hook_event_name": "PostToolUse",
            "session_id": "synthetic-session",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "tool_response": {"stdout": "test result: ok. 3 passed", "stderr": ""},
        }),
        &[],
    );
    assert_eq!(fx.single_record()["result"], "success");
}

/// A failed Agent call has no output container, so it must not take the
/// PreToolUse branch and flip the agent back to Running.
#[test]
fn failed_agent_call_is_recorded_failed_never_running() {
    let fx = Fixture::new();
    for prompt in ["Build the module", "Review\nsecond line", "x"] {
        let input = serde_json::json!({"prompt": prompt, "description": "synthetic"});
        fx.observe(
            serde_json::json!({
                "session_id": "synthetic-session",
                "tool_name": "Agent",
                "tool_input": input,
            }),
            &[],
        );
        assert!(fx.agent_statuses().contains(&"running".to_string()));
        fx.observe(failure("Agent", input, "Agent crashed".into()), &[]);
        assert!(
            fx.agent_statuses().iter().all(|s| s == "failed"),
            "{:?}",
            fx.agent_statuses()
        );
        fs::remove_dir_all(fx.project_dir().join("orchestrator")).unwrap();
    }
}

#[test]
fn interrupted_agent_call_is_recorded_failed_without_observation() {
    let fx = Fixture::new();
    let input = serde_json::json!({"prompt": "Build the module"});
    let mut payload = failure("Agent", input, "interrupted".into());
    payload["is_interrupt"] = serde_json::json!(true);
    fx.observe(payload, &[]);
    assert_eq!(fx.agent_statuses(), vec!["failed".to_string()]);
    assert!(fx.records().is_empty());
}
