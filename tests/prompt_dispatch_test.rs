//! Exercise the actual native prompt hook with isolated user and project data.
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

struct Fixture {
    temp: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let data = temp.path().join("data");
        let project = temp.path().join("prompt fixture");
        for dir in [&home, &data, &project, &home.join(".config/epic-harness")] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        Self {
            temp,
            home,
            data,
            project,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        cmd.args(args)
            .current_dir(&self.project)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("HARNESS_DIR", &self.data)
            .env("HARNESS_ROOT", self.temp.path())
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("APPDATA", self.home.join("AppData/Roaming"))
            .env("LOCALAPPDATA", self.home.join("AppData/Local"))
            .env_remove("HARNESS_MEM_URL")
            .env_remove("HARNESS_DB_URL")
            .env_remove("DATABASE_URL")
            .stdin(Stdio::null());
        cmd
    }

    fn dispatch(&self, payload: &str) -> Output {
        let mut child = self
            .command(&["dispatch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn memory(&self, title: &str, project: &str, tags: &str, body: &str, importance: &str) {
        let out = self
            .command(&[
                "mem",
                "add",
                "--title",
                title,
                "--project",
                project,
                "--tags",
                tags,
                "--body",
                body,
                "--importance",
                importance,
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn context(&self, prompt: &str) -> String {
        let output = self.dispatch(
            &serde_json::json!({
                "hook_event_name": "UserPromptSubmit", "prompt": prompt, "session_id": "synthetic"
            })
            .to_string(),
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            json["hookSpecificOutput"]["hookEventName"],
            "UserPromptSubmit"
        );
        json["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

#[test]
fn prompt_dispatch_delivers_static_guidance_and_task_scoped_memory() {
    let f = Fixture::new();
    for n in 0..12 {
        f.memory(
            &format!("Startup decision {n}"),
            "prompt_fixture",
            "",
            "Generic startup policy",
            "1",
        );
    }
    // Deliberately below the generic startup recall cap: only the task hint finds it.
    f.memory(
        "quartzgate policy",
        "prompt_fixture",
        "",
        "QUARTZ_TASK_NONCE use an explicit retry ceiling",
        "0.1",
    );
    f.memory(
        "quartzgate foreign",
        "other_prompt_fixture",
        "",
        "FOREIGN_NONCE",
        "1",
    );
    f.memory(
        "quartzgate retired",
        "prompt_fixture",
        "stale",
        "STALE_NONCE",
        "1",
    );
    let context = f.context("Fix the quartzgate crash and add regression tests.");
    assert!(context.contains("QUARTZ_TASK_NONCE"), "{context}");
    assert!(context.contains("Skill: debug"), "{context}");
    assert!(context.contains("Skill: tdd"), "{context}");
    assert!(context.contains("Skill: verify"), "{context}");
    assert!(!context.contains("FOREIGN_NONCE"));
    assert!(!context.contains("STALE_NONCE"));
    assert!(context.contains("advisory"));
    assert!(context.chars().count() <= 10_000);
}

#[test]
fn dispatch_metadata_has_no_prompt_or_memory_content_and_is_not_deduped_per_second() {
    let f = Fixture::new();
    f.memory(
        "login policy",
        "prompt_fixture",
        "",
        "MEMORY_PRIVATE_SENTINEL",
        "1",
    );
    for _ in 0..2 {
        let context = f.context(
            "Implement login authentication. PROMPT_PRIVATE_SENTINEL token=synthetic_secret",
        );
        assert!(context.contains("Skill: secure"));
        assert!(!context.contains("PROMPT_PRIVATE_SENTINEL"));
        assert!(!context.contains("synthetic_secret"));
    }
    let files: Vec<_> = fs::read_dir(f.data.join("projects/prompt_fixture/dispatch"))
        .unwrap()
        .collect();
    assert_eq!(files.len(), 1);
    let log = fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
    let entries: Vec<serde_json::Value> = log
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "distinct prompt submissions must each deliver"
    );
    assert_eq!(entries[0]["event"], "UserPromptSubmit");
    assert_eq!(entries[0]["memory_status"], "delivered");
    assert!(!log.contains("PRIVATE_SENTINEL"));
    assert!(!log.contains("synthetic_secret"));
    assert!(!log.contains("login policy"));
    assert!(!log.contains("synthetic")); // no session IDs either
}

#[test]
fn recalled_previews_mask_full_quoted_credentials_before_truncation() {
    let f = Fixture::new();
    let secret = "SECRET_PRIVATE_VALUE".repeat(50);
    f.memory(
        "quartzgate credentials",
        "prompt_fixture",
        "",
        &format!(r#"{{"token":"{secret}","policy":"KEEP_SAFE_POLICY"}}"#),
        "1",
    );
    let output = f.dispatch(r#"{"hook_event_name":"UserPromptSubmit","prompt":"Fix quartzgate"}"#);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("KEEP_SAFE_POLICY"));
    assert!(context.contains("REDACTED"));
    assert!(!context.contains("SECRET_PRIVATE_VALUE"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET_PRIVATE_VALUE"));
}

#[test]
fn irrelevant_prompts_without_memory_are_noops_and_explicit_nouns_select_perf() {
    let f = Fixture::new();
    let output = f.dispatch(r#"{"hook_event_name":"UserPromptSubmit","prompt":"Hello"}"#);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let context = f.context("Optimize slow quartzgate performance");
    assert!(context.contains("Skill: perf"));
    assert!(context.contains("Skill: verify"));
}

#[test]
fn total_budget_admits_whole_memory_records_and_logs_only_delivered_records() {
    let f = Fixture::new();
    for n in 0..3 {
        f.memory(
            &format!("quartzgate policy {n}"),
            "prompt_fixture",
            "",
            &"\u{0001}".repeat(450),
            "1",
        );
    }
    let context = f.context("Fix quartzgate authentication crash and add tests");
    let records: Vec<_> = context.lines().filter(|s| s.starts_with('{')).collect();
    assert!(!records.is_empty());
    assert!(
        records.len() < 3,
        "escape expansion must consume the remaining budget"
    );
    for record in &records {
        let _: serde_json::Value = serde_json::from_str(record).expect("complete memory record");
    }
    let entry = fs::read_dir(f.data.join("projects/prompt_fixture/dispatch"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let log: serde_json::Value =
        serde_json::from_str(fs::read_to_string(entry.path()).unwrap().trim()).unwrap();
    assert_eq!(
        log["delivered_memories"].as_u64().unwrap(),
        records.len() as u64
    );
    assert!(context.chars().count() <= 10_000);
}

#[test]
fn malformed_unsupported_and_oversized_payloads_are_nonblocking_noops() {
    let f = Fixture::new();
    let huge =
        serde_json::json!({"hook_event_name":"UserPromptSubmit", "prompt":"x".repeat(270_000)})
            .to_string();
    for payload in [
        "",
        "{",
        "null",
        "[]",
        r#"{"prompt":"fix a crash"}"#,
        r#"{"hook_event_name":"PostToolUse","prompt":"fix a crash"}"#,
        r#"{"hook_event_name":"UserPromptSubmit","prompt":42}"#,
        r#"{"hook_event_name":"UserPromptSubmit","prompt":"  "}"#,
        huge.as_str(),
    ] {
        let out = f.dispatch(payload);
        assert!(
            out.status.success(),
            "payload {}: {}",
            payload.len(),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stdout.is_empty(),
            "unexpected stdout {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
}

#[test]
fn dispatch_handles_unicode_budgets_and_unavailable_memory_without_executing_prompt() {
    let f = Fixture::new();
    // A directory in place of the memory DB forces a real unavailable-store result.
    fs::create_dir_all(f.temp.path().join(".harness/memory.db")).unwrap();
    let marker = f.project.join("payload-must-not-run");
    let prompt = format!(
        "Implement a quartzgate feature and fix a crash. {} ; echo danger > payload-must-not-run",
        "🦀".repeat(20_000)
    );
    let context = f.context(&prompt);
    assert!(context.contains("Skill: tdd"));
    assert!(context.contains("Memory recall unavailable"));
    assert!(context.chars().count() <= 10_000);
    assert!(!marker.exists());
}

#[test]
fn supported_manifests_register_prompt_dispatch_exactly_once() {
    for manifest in [
        include_str!("../.claude-plugin/hooks.json"),
        include_str!("../.codex-plugin/hooks.json"),
    ] {
        let json: serde_json::Value = serde_json::from_str(manifest).unwrap();
        let hooks = json["hooks"]["UserPromptSubmit"]
            .as_array()
            .expect("native prompt hook");
        assert_eq!(hooks.len(), 1);
        let commands = hooks[0]["hooks"].as_array().unwrap();
        assert_eq!(commands.len(), 1);
        assert!(
            commands[0]["command"]
                .as_str()
                .unwrap()
                .ends_with(" dispatch")
        );
        for event in ["SessionStart", "PreToolUse", "PostToolUse", "PreCompact"] {
            assert!(json["hooks"][event].is_array(), "preserve {event}");
        }
    }
}
