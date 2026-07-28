//! Process-level Codex hook contracts. These tests exercise the compiled CLI
//! with isolated HOME and project directories rather than internal helpers.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_epic-harness");

fn project_path(root: &Path) -> PathBuf {
    root.join("project-under-test")
}

fn harness_path(home: &Path, project: &Path) -> PathBuf {
    let output = Command::new(BINARY)
        .arg("path")
        .current_dir(project)
        .env("HOME", home)
        .env_remove("USERPROFILE")
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .stdin(Stdio::null())
        .output()
        .expect("run path");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("path output is UTF-8")
            .trim(),
    )
}

fn establish_host_session_state(home: &Path, project: &Path, session_id: &str) {
    let harness = harness_path(home, project);
    fs::create_dir_all(&harness).expect("harness directory");
    fs::write(
        harness.join(format!("session_start.{session_id}.json")),
        r#"{"date":"20260728","written_at":"2026-07-28T00:00:00Z"}"#,
    )
    .expect("session start state");
}

fn run_hook(home: &Path, project: &Path, command: &str, input: &str) -> Output {
    run_hook_with_env(home, project, command, input, &[])
}

fn run_hook_with_env(
    home: &Path,
    project: &Path,
    command: &str,
    input: &str,
    extra_env: &[(&str, &std::ffi::OsStr)],
) -> Output {
    let mut process = Command::new(BINARY);
    process
        .arg(command)
        .current_dir(project)
        .env("HOME", home)
        .env_remove("USERPROFILE")
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .env("EPIC_HOOK_PROFILE", "strict")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        process.env(key, value);
    }
    process
        .output_with_stdin(input.as_bytes())
        .unwrap_or_else(|error| panic!("run {command}: {error}"))
}

trait OutputWithStdin {
    fn output_with_stdin(self, input: &[u8]) -> std::io::Result<Output>;
}

impl OutputWithStdin for &mut Command {
    fn output_with_stdin(self, input: &[u8]) -> std::io::Result<Output> {
        let mut child = self.spawn()?;
        child.stdin.take().expect("piped stdin").write_all(input)?;
        child.wait_with_output()
    }
}

fn wait_for_completed_jobs(queue: &Path, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let completed = fs::read_dir(queue)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".completed"))
            .count();
        if completed == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "expected {expected} completed reflection jobs, found {completed}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_lines(path: &Path, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let count = fs::read_to_string(path).unwrap_or_default().lines().count();
        if count == expected {
            return;
        }
        assert!(
            count < expected && Instant::now() < deadline,
            "expected {expected} lines in {}, found {count}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn session_start_context(output: &Output) -> String {
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("SessionStart JSON");
    value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("SessionStart additionalContext")
        .to_string()
}

#[cfg(unix)]
struct FakeDashboard {
    port: u16,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl FakeDashboard {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("fake dashboard bind");
        listener
            .set_nonblocking(true)
            .expect("fake dashboard nonblocking");
        let port = listener
            .local_addr()
            .expect("fake dashboard address")
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let body = format!(
                "<html><head><meta name=\"harness-version\" content=\"{}\"></head></html>",
                env!("CARGO_PKG_VERSION")
            );
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                        let mut request = [0_u8; 1024];
                        let _ = stream.read(&mut request);
                        let response = format!(
                            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("fake dashboard accept: {error}"),
                }
            }
        });
        Self {
            port,
            stop,
            worker: Some(worker),
        }
    }
}

#[cfg(unix)]
impl Drop for FakeDashboard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("fake dashboard worker");
        }
    }
}

fn record_failed_observations(home: &Path, project: &Path, session_id: &str, error: &str) {
    establish_host_session_state(home, project, session_id);
    for index in 0..3 {
        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "turn_id": format!("{session_id}-turn"),
            "tool_use_id": format!("{session_id}-tool-{index}"),
            "tool_name": "Bash",
            "tool_input": {"command": "false"},
            "tool_response": {"exit_code": 1, "stderr": error},
        })
        .to_string();
        let output = run_hook(home, project, "observe", &input);
        assert!(
            output.status.success(),
            "{session_id} observation {index}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn invalid_codex_guard_inputs_fail_closed_with_exit_two() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    fs::create_dir_all(&project).expect("project");

    for (label, input) in [
        ("empty input", ""),
        ("empty object", "{}"),
        (
            "missing tool name",
            r#"{"hook_event_name":"PreToolUse","tool_input":{"command":"rm -rf /"}}"#,
        ),
        (
            "missing Bash command",
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{}}"#,
        ),
        (
            "empty Bash command",
            r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"  "}}"#,
        ),
    ] {
        let output = run_hook(root.path(), &project, "guard", input);

        assert_eq!(output.status.code(), Some(2), "{label}");
        let stdout: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("one deny JSON object");
        assert_eq!(
            stdout["hookSpecificOutput"]["permissionDecision"], "deny",
            "{label}"
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .expect("deny output is UTF-8")
                .lines()
                .count(),
            1,
            "{label}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("invalid hook input"),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn non_start_hooks_reject_missing_host_session_state() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    fs::create_dir_all(&project).expect("project");

    let observe = run_hook(
        root.path(),
        &project,
        "observe",
        r#"{"hook_event_name":"PostToolUse","session_id":"missing-state","tool_name":"Bash","tool_input":{"command":"pwd"},"tool_response":{"exit_code":0}}"#,
    );
    assert_eq!(observe.status.code(), Some(1));
    assert!(observe.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&observe.stderr).contains("host session identity is unavailable")
    );

    let guard = run_hook(
        root.path(),
        &project,
        "guard",
        r#"{"hook_event_name":"PreToolUse","session_id":"missing-state","tool_name":"Bash","tool_input":{"command":"pwd"}}"#,
    );
    assert_eq!(guard.status.code(), Some(2));
    let deny: serde_json::Value = serde_json::from_slice(&guard.stdout).expect("guard deny JSON");
    assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");

    let stop = run_hook(
        root.path(),
        &project,
        "observe",
        r#"{"hook_event_name":"SubagentStop","session_id":"missing-state","agent_id":"agent-1","agent_type":"worker"}"#,
    );
    assert_eq!(stop.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stop.stdout).expect("SubagentStop JSON"),
        serde_json::json!({})
    );
}

#[cfg(unix)]
#[test]
fn startup_compact_resume_restores_context_and_routes_dashboard_opening() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    let fake_bin = root.path().join("fake-bin");
    let browser_log = root.path().join("browser.log");
    fs::create_dir_all(&project).expect("project");
    fs::create_dir_all(&fake_bin).expect("fake bin");

    for command in ["open", "xdg-open"] {
        let path = fake_bin.join(command);
        fs::write(
            &path,
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$EPIC_TEST_BROWSER_LOG\"\n",
        )
        .expect("browser probe");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("browser probe permissions");
    }

    let dashboard = FakeDashboard::start();
    let global_harness = root.path().join(".harness");
    fs::create_dir_all(&global_harness).expect("global harness");
    fs::write(
        global_harness.join("config.toml"),
        format!(
            "[dashboard]\nport = {}\nauto_open = true\n\n[evolution]\nattribution_holdout_modulus = 0\n",
            dashboard.port
        ),
    )
    .expect("test config");

    let mut search_paths = vec![fake_bin];
    search_paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let test_path = std::env::join_paths(search_paths).expect("test PATH");
    let extra_env = [
        ("PATH", test_path.as_os_str()),
        ("EPIC_TEST_BROWSER_LOG", browser_log.as_os_str()),
    ];
    let session_id = "stable-host-session";

    let startup = run_hook_with_env(
        root.path(),
        &project,
        "resume",
        &serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": session_id,
            "source": "startup",
        })
        .to_string(),
        &extra_env,
    );
    assert!(
        startup.status.success(),
        "{}",
        String::from_utf8_lossy(&startup.stderr)
    );
    wait_for_lines(&browser_log, 1);

    let harness = harness_path(root.path(), &project);
    let skill_dir = harness.join("evolved/durable-skill");
    fs::create_dir_all(&skill_dir).expect("evolved skill directory");
    fs::write(
        skill_dir.join("SKILL.md"),
        "# Durable skill\nDURABLE_SKILL_CONTEXT_MARKER\n",
    )
    .expect("evolved skill");

    let snapshot = run_hook_with_env(
        root.path(),
        &project,
        "snapshot",
        &serde_json::json!({
            "hook_event_name": "PreCompact",
            "session_id": session_id,
            "conversation_summary": "DURABLE_CONTEXT_MARKER",
            "pending_tasks": ["DURABLE_PENDING_MARKER"],
            "context_usage": 0.82,
        })
        .to_string(),
        &extra_env,
    );
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let snapshots: Vec<PathBuf> = fs::read_dir(harness.join("sessions"))
        .expect("session snapshots")
        .map(|entry| entry.expect("snapshot entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();
    assert_eq!(snapshots.len(), 1);
    let persisted = fs::read_to_string(&snapshots[0]).expect("persisted snapshot");
    assert!(persisted.contains("DURABLE_CONTEXT_MARKER"));
    assert!(persisted.contains("DURABLE_PENDING_MARKER"));

    let compact = run_hook_with_env(
        root.path(),
        &project,
        "resume",
        &serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": session_id,
            "source": "compact",
        })
        .to_string(),
        &extra_env,
    );
    assert!(
        compact.status.success(),
        "{}",
        String::from_utf8_lossy(&compact.stderr)
    );
    let compact_context = session_start_context(&compact);
    assert!(compact_context.contains("DURABLE_CONTEXT_MARKER"));
    assert!(compact_context.contains("DURABLE_PENDING_MARKER"));
    assert!(compact_context.contains("DURABLE_SKILL_CONTEXT_MARKER"));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        fs::read_to_string(&browser_log)
            .expect("startup browser call")
            .lines()
            .count(),
        1,
        "compact must not open a dashboard tab"
    );

    let resume = run_hook_with_env(
        root.path(),
        &project,
        "resume",
        &serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": session_id,
            "source": "resume",
        })
        .to_string(),
        &extra_env,
    );
    assert!(
        resume.status.success(),
        "{}",
        String::from_utf8_lossy(&resume.stderr)
    );
    let resume_context = session_start_context(&resume);
    assert!(resume_context.contains("DURABLE_CONTEXT_MARKER"));
    assert!(resume_context.contains("DURABLE_PENDING_MARKER"));
    assert!(resume_context.contains("DURABLE_SKILL_CONTEXT_MARKER"));
    wait_for_lines(&browser_log, 2);
    let expected_url = format!("http://localhost:{}", dashboard.port);
    assert_eq!(
        fs::read_to_string(&browser_log)
            .expect("browser decisions")
            .lines()
            .collect::<Vec<_>>(),
        [expected_url.as_str(), expected_url.as_str()]
    );
}

#[test]
fn fresh_session_start_initializes_all_project_runtime_directories() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    fs::create_dir_all(&project).expect("project");

    let output = run_hook(
        root.path(),
        &project,
        "resume",
        r#"{"hook_event_name":"SessionStart","session_id":"fresh-session","source":"startup"}"#,
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let harness = harness_path(root.path(), &project);
    for path in ["obs", "sessions", "memory", "evolved"] {
        assert!(
            harness.join(path).is_dir(),
            "fresh SessionStart must create {}",
            harness.join(path).display()
        );
    }
}

#[test]
fn fresh_session_start_migrates_legacy_project_local_harness() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    let legacy = project.join(".harness");
    fs::create_dir_all(&legacy).expect("legacy harness");
    fs::write(legacy.join("legacy-state.txt"), "preserve me").expect("legacy state");

    let output = run_hook(
        root.path(),
        &project,
        "resume",
        r#"{"hook_event_name":"SessionStart","session_id":"migration-session","source":"startup"}"#,
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let harness = harness_path(root.path(), &project);
    assert_eq!(
        fs::read_to_string(harness.join("legacy-state.txt")).expect("migrated legacy state"),
        "preserve me"
    );
    assert!(
        !legacy.exists(),
        "legacy project-local harness must be removed after migration"
    );
}

#[test]
fn session_start_from_home_does_not_migrate_the_global_harness_root() {
    let home = tempfile::tempdir().expect("temp home");
    let global_harness = home.path().join(".harness");
    fs::create_dir_all(&global_harness).expect("global harness");
    fs::write(global_harness.join("global-state.txt"), "keep global").expect("global state");

    let output = run_hook(
        home.path(),
        home.path(),
        "resume",
        r#"{"hook_event_name":"SessionStart","session_id":"home-session","source":"startup"}"#,
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let context = session_start_context(&output);
    assert!(
        !context.contains("Migration failed"),
        "the global harness root is not project-local legacy state: {context}"
    );
    assert_eq!(
        fs::read_to_string(global_harness.join("global-state.txt")).expect("global state remains"),
        "keep global"
    );
    let project_harness = harness_path(home.path(), home.path());
    assert!(project_harness.is_dir());
    assert!(
        !project_harness.join("global-state.txt").exists(),
        "global state must not be copied into the home-directory project"
    );
}

#[test]
fn native_codex_subagent_lifecycle_persists_running_then_done() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    fs::create_dir_all(&project).expect("project");
    let harness = harness_path(root.path(), &project);
    fs::create_dir_all(harness.join("obs")).expect("harness obs directory");
    establish_host_session_state(root.path(), &project, "session-a");

    let start = run_hook(
        root.path(),
        &project,
        "observe",
        r#"{"hook_event_name":"SubagentStart","session_id":"session-a","agent_id":"agent-1","agent_type":"reviewer"}"#,
    );
    assert!(
        start.status.success(),
        "{}",
        String::from_utf8_lossy(&start.stderr)
    );

    let running: serde_json::Value = serde_json::from_slice(
        &fs::read(harness.join("orchestrator/run.json")).expect("running state"),
    )
    .expect("valid run state");
    assert_eq!(running["agents"][0]["id"], "agent-1");
    assert_eq!(running["agents"][0]["role"], "reviewer");
    assert_eq!(running["agents"][0]["status"], "running");

    let stop = run_hook(
        root.path(),
        &project,
        "observe",
        r#"{"hook_event_name":"SubagentStop","session_id":"session-a","agent_id":"agent-1","agent_type":"reviewer"}"#,
    );
    assert!(
        stop.status.success(),
        "{}",
        String::from_utf8_lossy(&stop.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stop.stdout).expect("Codex stop JSON")["continue"],
        true,
    );

    let done: serde_json::Value = serde_json::from_slice(
        &fs::read(harness.join("orchestrator/run.json")).expect("completed state"),
    )
    .expect("valid completed state");
    assert_eq!(done["agents"][0]["status"], "done");
    assert!(done["agents"][0]["completed_at"].is_string());
}

#[test]
fn session_end_jobs_are_session_scoped_and_exactly_once() {
    let root = tempfile::tempdir().expect("temp root");
    let project = project_path(root.path());
    fs::create_dir_all(&project).expect("project");
    let harness = harness_path(root.path(), &project);
    fs::create_dir_all(harness.join("obs")).expect("harness obs directory");

    for (index, (session_id, error)) in [
        ("session-a", "TypeError: alpha-only failure"),
        ("session-b", "permission denied: beta-only failure"),
    ]
    .into_iter()
    .enumerate()
    {
        record_failed_observations(root.path(), &project, session_id, error);
        let input = format!(r#"{{"hook_event_name":"SessionEnd","session_id":"{session_id}"}}"#);
        let output = run_hook(root.path(), &project, "reflect", &input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("SessionEnd JSON")["continue"],
            true,
        );
        wait_for_completed_jobs(&harness.join("reflect-queue"), index + 1);
    }

    let queue = harness.join("reflect-queue");
    wait_for_completed_jobs(&queue, 2);
    let evolution_path = harness.join("evolution.jsonl");
    let evolution_before_replay = fs::read(&evolution_path).expect("evolution records");
    let records: Vec<serde_json::Value> = String::from_utf8(evolution_before_replay.clone())
        .expect("evolution UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("evolution JSON"))
        .collect();
    assert_eq!(records.len(), 2);

    let session_a = records
        .iter()
        .find(|record| {
            record["session_id"]
                .as_str()
                .is_some_and(|session_id| session_id.ends_with("_session-a"))
        })
        .expect("session-a analysis");
    let session_b = records
        .iter()
        .find(|record| {
            record["session_id"]
                .as_str()
                .is_some_and(|session_id| session_id.ends_with("_session-b"))
        })
        .expect("session-b analysis");
    assert_eq!(session_a["observations"], 3);
    assert_eq!(session_b["observations"], 3);
    assert_eq!(
        session_a["error_patterns"]
            .as_object()
            .expect("session-a errors")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["type_error"]
    );
    assert_eq!(
        session_b["error_patterns"]
            .as_object()
            .expect("session-b errors")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["permission_denied"]
    );
    assert!(
        session_a["analysis_summary"]
            .as_str()
            .expect("session-a summary")
            .contains("type_error:3")
    );
    assert!(
        !session_a["analysis_summary"]
            .as_str()
            .expect("session-a summary")
            .contains("permission_denied")
    );
    assert!(
        session_b["analysis_summary"]
            .as_str()
            .expect("session-b summary")
            .contains("permission_denied:3")
    );
    assert!(
        !session_b["analysis_summary"]
            .as_str()
            .expect("session-b summary")
            .contains("type_error")
    );

    let replay = run_hook(
        root.path(),
        &project,
        "reflect",
        r#"{"hook_event_name":"SessionEnd","session_id":"session-a"}"#,
    );
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    wait_for_completed_jobs(&queue, 2);
    assert_eq!(
        fs::read(&evolution_path).expect("evolution records after replay"),
        evolution_before_replay,
        "replaying SessionEnd must not duplicate or rewrite persisted analysis"
    );
}
