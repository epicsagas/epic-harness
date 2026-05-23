//! state.rs -- Orchestration state types and file I/O helpers

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// ── Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Paused,
    Complete,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Pending,
    Running,
    Done,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ControlAction {
    Pause,
    Cancel,
    Redirect,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationRun {
    pub id: String,
    pub status: RunStatus,
    pub agents: Vec<AgentDef>,
    #[serde(default)]
    pub dependency_graph: HashMap<String, Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub id: String,
    pub role: String,
    pub task: String,
    #[serde(default)]
    pub satisfies: Vec<String>,
    pub status: AgentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub timestamp: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusFile {
    pub agent_id: String,
    pub phase: String,
    pub progress: f64,
    pub last_heartbeat: String,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxMessage {
    pub from: String,
    pub timestamp: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlDirective {
    pub action: ControlAction,
    pub target: Option<String>,
    pub message: Option<String>,
    pub generation: u64,
}

// ── Constants ─────────────────────────────────────────

pub const MAX_CONCURRENT_AGENTS: usize = 6;
const RUN_FILE: &str = "run.json";
const CONTROL_FILE: &str = "control.json";

// ── Path helpers ──────────────────────────────────────

/// Validate that an agent_id contains only safe characters.
/// Prevents path traversal (e.g., `../../etc/passwd`).
pub fn validate_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Returns the orchestrator state directory: `$HARNESS_DIR/orchestrator/`
pub fn orchestrator_dir(base: &Path) -> PathBuf {
    base.join("orchestrator")
}

pub fn run_file(base: &Path) -> PathBuf {
    orchestrator_dir(base).join(RUN_FILE)
}

pub fn control_file(base: &Path) -> PathBuf {
    orchestrator_dir(base).join(CONTROL_FILE)
}

pub fn agent_dir(base: &Path, agent_id: &str) -> PathBuf {
    if !validate_agent_id(agent_id) {
        return orchestrator_dir(base).join("agents").join("_invalid");
    }
    orchestrator_dir(base).join("agents").join(agent_id)
}

pub fn agent_status_file(base: &Path, agent_id: &str) -> PathBuf {
    agent_dir(base, agent_id).join("status.json")
}

pub fn agent_stream_file(base: &Path, agent_id: &str) -> PathBuf {
    agent_dir(base, agent_id).join("stream.jsonl")
}

pub fn agent_inbox_file(base: &Path, agent_id: &str) -> PathBuf {
    agent_dir(base, agent_id).join("inbox.jsonl")
}

// ── File I/O ──────────────────────────────────────────

/// Atomic write: write to `.tmp` then rename. Sets 0o600 permissions on Unix.
pub fn atomic_write_json(path: &Path, data: &impl Serialize) -> io::Result<()> {
    let content = serde_json::to_string(data).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Read a JSON file, returning None if missing or unparseable.
pub fn read_json_opt<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Append a JSON line to a JSONL file.
pub fn append_jsonl(path: &Path, record: &impl Serialize) -> io::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(record).map_err(io::Error::other)?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{json}")?;
    Ok(())
}

/// Read all lines from a JSONL file as typed records.
/// Skips files larger than 10 MB to prevent memory exhaustion.
pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    const MAX_JSONL_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
    if fs::metadata(path).map(|m| m.len()).unwrap_or(0) > MAX_JSONL_BYTES {
        return Vec::new();
    }
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// ── Orchestration operations ──────────────────────────

/// Create the full directory structure for an orchestration run.
/// Creates: orchestrator/, orchestrator/agents/{id}/ for each agent.
pub fn init_run(base: &Path, run: &OrchestrationRun) -> io::Result<()> {
    let orch_dir = orchestrator_dir(base);
    fs::create_dir_all(&orch_dir)?;

    for agent in &run.agents {
        let a_dir = agent_dir(base, &agent.id);
        fs::create_dir_all(&a_dir)?;
    }

    atomic_write_json(&run_file(base), run)?;
    Ok(())
}

/// Write/update an agent's status file.
pub fn write_agent_status(base: &Path, agent_id: &str, status: &AgentStatusFile) -> io::Result<()> {
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;
    atomic_write_json(&agent_status_file(base, agent_id), status)
}

/// Read an agent's status file.
pub fn read_agent_status(base: &Path, agent_id: &str) -> Option<AgentStatusFile> {
    read_json_opt(&agent_status_file(base, agent_id))
}

/// Append an event to an agent's stream.jsonl.
pub fn append_event(base: &Path, agent_id: &str, event: &AgentEvent) -> io::Result<()> {
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;
    append_jsonl(&agent_stream_file(base, agent_id), event)
}

/// Read all events from an agent's stream.jsonl.
pub fn read_events(base: &Path, agent_id: &str) -> Vec<AgentEvent> {
    read_jsonl(&agent_stream_file(base, agent_id))
}

/// Post a message to an agent's inbox.
pub fn post_inbox_message(base: &Path, agent_id: &str, msg: &InboxMessage) -> io::Result<()> {
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;
    append_jsonl(&agent_inbox_file(base, agent_id), msg)
}

/// Read all inbox messages for an agent.
pub fn read_inbox(base: &Path, agent_id: &str) -> Vec<InboxMessage> {
    read_jsonl(&agent_inbox_file(base, agent_id))
}

/// Write a control directive.
pub fn write_control(base: &Path, directive: &ControlDirective) -> io::Result<()> {
    let orch_dir = orchestrator_dir(base);
    fs::create_dir_all(&orch_dir)?;
    atomic_write_json(&control_file(base), directive)
}

/// Read the current control directive.
pub fn read_control(base: &Path) -> Option<ControlDirective> {
    read_json_opt(&control_file(base))
}

/// Read the run file.
pub fn read_run(base: &Path) -> Option<OrchestrationRun> {
    read_json_opt(&run_file(base))
}

/// Update the run file atomically.
pub fn write_run(base: &Path, run: &OrchestrationRun) -> io::Result<()> {
    atomic_write_json(&run_file(base), run)
}

/// Evaluate the dependency graph: given an agent that just completed,
/// return the IDs of agents that are now unblocked (all their deps are done/failed).
pub fn evaluate_dependencies(run: &OrchestrationRun, completed_agent_id: &str) -> Vec<String> {
    let mut unblocked = Vec::new();

    // Build a set of completed/failed agent IDs
    let mut finished = std::collections::HashSet::new();
    for agent in &run.agents {
        if agent.status == AgentStatus::Done || agent.status == AgentStatus::Failed {
            finished.insert(agent.id.clone());
        }
    }
    // The just-completed agent is also finished
    finished.insert(completed_agent_id.to_string());

    // Check each agent that is still pending/blocked
    for agent in &run.agents {
        if agent.status != AgentStatus::Pending && agent.status != AgentStatus::Blocked {
            continue;
        }
        let deps = run.dependency_graph.get(&agent.id);
        match deps {
            Some(deps) if !deps.is_empty() && deps.iter().all(|d| finished.contains(d)) => {
                unblocked.push(agent.id.clone());
            }
            _ => {
                // No dependencies -> already unblocked (not our concern here,
                // but if they are blocked, they have deps. Skip no-dep agents.)
            }
        }
    }

    unblocked
}

/// Check if the run is complete: all agents are Done or Failed.
pub fn is_run_complete(run: &OrchestrationRun) -> bool {
    run.agents
        .iter()
        .all(|a| a.status == AgentStatus::Done || a.status == AgentStatus::Failed)
}

/// Acquire an exclusive advisory file lock on `path`.
/// Returns the locked file handle (holds the lock until dropped).
/// Uses non-blocking flock with retries to avoid blocking hooks.
///
/// **Tradeoff**: Binds flock(2) directly via `extern "C"` instead of depending on
/// the `libc` crate. This avoids a transitive dependency but requires that the
/// platform's C library exports `flock` with the standard signature — true for
/// macOS, Linux, and all BSDs. If this ever causes link issues, switch to
/// `fs4` or `file-lock` crate (one line change in Cargo.toml + here).
#[cfg(unix)]
fn acquire_lock(lock_path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .mode(0o600)
        .read(true)
        .write(true)
        .open(lock_path)?;
    // Advisory exclusive lock via flock(2) — non-blocking with retries.
    // Uses raw syscall binding to avoid the `libc` crate dependency.
    // SAFETY: fd is a valid open file descriptor.
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    const MAX_RETRIES: u32 = 10;
    const RETRY_DELAY_MS: u64 = 50;
    for _ in 0..MAX_RETRIES {
        let result = unsafe { flock(fd, LOCK_EX | LOCK_NB) };
        if result == 0 {
            return Ok(file);
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::WouldBlock {
            return Err(err);
        }
        std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "failed to acquire lock after retries",
    ))
}

#[cfg(unix)]
const LOCK_EX: i32 = 2; // LOCK_EX from sys/file.h
#[cfg(unix)]
const LOCK_NB: i32 = 4; // LOCK_NB from sys/file.h

/// Raw flock(2) binding — avoids pulling in the `libc` crate.
/// Only used on Unix for advisory file locking.
#[cfg(unix)]
unsafe fn flock(fd: i32, operation: i32) -> i32 {
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    unsafe { flock(fd, operation) }
}

/// No-op lock for non-Unix platforms. Advisory file locking (flock) is Unix-only.
/// On Windows, concurrent agent spawns may produce rare write races on run.json.
/// Impact is limited: auto-tracked runs are ephemeral and self-heal on next session.
#[cfg(not(unix))]
fn acquire_lock(lock_path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
}

/// Upsert an agent into an auto-created run.json.
///
/// If `run.json` does not exist, creates a new auto-run with id `auto-{timestamp}`.
/// If it exists, adds the agent or updates its status.
/// Uses advisory file locking to prevent TOCTOU races during concurrent agent spawns.
/// Returns the agent ID used.
pub fn upsert_agent_to_run(
    base: &Path,
    agent_id: &str,
    description: &str,
    subagent_type: &str,
    status: AgentStatus,
) -> io::Result<String> {
    let orch_dir = orchestrator_dir(base);
    fs::create_dir_all(&orch_dir)?;

    // Acquire advisory lock to prevent concurrent write races
    let lock_path = orch_dir.join("run.json.lock");
    let _lock = acquire_lock(&lock_path)?;

    let now = crate::shared::helpers::now_iso();

    let mut run = match read_run(base) {
        Some(r) => r,
        None => OrchestrationRun {
            id: {
                let compact = now.replace(['-', ':'], "").replace('T', "-");
                let truncated: String = compact.chars().take(19).collect();
                format!("auto-{truncated}")
            },
            status: RunStatus::Running,
            agents: Vec::new(),
            dependency_graph: HashMap::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        },
    };

    // Ensure agent directory exists
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;

    let mut changed = false;
    if let Some(existing) = run.agents.iter_mut().find(|a| a.id == agent_id) {
        if existing.status != status {
            existing.status = status.clone();
            if status == AgentStatus::Done {
                existing.completed_at = Some(now.clone());
            }
            if status == AgentStatus::Running && existing.started_at.is_none() {
                existing.started_at = Some(now.clone());
            }
            changed = true;
        }
    } else {
        run.agents.push(AgentDef {
            id: agent_id.to_string(),
            role: subagent_type.to_string(),
            task: description.to_string(),
            satisfies: Vec::new(),
            status: status.clone(),
            started_at: if status == AgentStatus::Running {
                Some(now.clone())
            } else {
                None
            },
            completed_at: if status == AgentStatus::Done {
                Some(now.clone())
            } else {
                None
            },
        });
        changed = true;
    }

    if !changed {
        return Ok(agent_id.to_string());
    }

    // Check if all agents are done
    if is_run_complete(&run) {
        run.status = RunStatus::Complete;
    }
    run.updated_at = now.clone();
    write_run(base, &run)?;

    // Write agent status file for dashboard only when status meaningfully changes
    // (skip on repeated "running" updates to reduce I/O during heartbeat cycles)
    let should_write_status = match read_agent_status(base, agent_id) {
        Some(existing) => existing.status != status,
        None => true, // first time — always write
    };
    if should_write_status {
        let status_file = AgentStatusFile {
            agent_id: agent_id.to_string(),
            phase: if status == AgentStatus::Done {
                "complete".to_string()
            } else if status == AgentStatus::Running {
                "running".to_string()
            } else {
                "pending".to_string()
            },
            progress: if status == AgentStatus::Done {
                1.0
            } else {
                0.0
            },
            last_heartbeat: now,
            status,
        };
        write_agent_status(base, agent_id, &status_file)?;
    }

    Ok(agent_id.to_string())
}

/// Clean up stale auto-generated runs.
///
/// - Complete runs older than 1 hour: remove agent status directories
/// - Running runs older than 2 hours: mark as complete, then clean
/// - Individual agents with stale heartbeat (>30 min): mark as done
pub fn auto_cleanup_stale_runs(base: &Path) {
    let orch_dir = orchestrator_dir(base);
    let lock_path = orch_dir.join("run.json.lock");
    let _lock = match acquire_lock(&lock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[harness] cleanup: failed to acquire lock: {e}");
            return;
        }
    };

    let Some(mut run) = read_run(base) else {
        return;
    };

    // Only auto-cleanup auto-generated runs
    if !run.id.starts_with("auto-") {
        return;
    }

    let current_epoch = epoch_secs_now();
    let now = crate::shared::helpers::now_iso();

    // Mark individual stale agents as done (heartbeat > 30 min ago)
    let mut any_agent_cleaned = false;
    for agent in &mut run.agents {
        if agent.status != AgentStatus::Running {
            continue;
        }
        let stale = agent.started_at.as_ref().is_none_or(|started| {
            current_epoch.saturating_sub(epoch_secs_from_iso(started)) > 1800 // 30 min
        });
        if stale {
            agent.status = AgentStatus::Done;
            agent.completed_at = Some(now.clone());
            any_agent_cleaned = true;
        }
    }

    // Check run-level staleness
    let run_epoch = epoch_secs_from_iso(&run.updated_at);
    let stale_complete =
        run.status == RunStatus::Complete && current_epoch.saturating_sub(run_epoch) > 3600;
    let stale_running =
        run.status == RunStatus::Running && current_epoch.saturating_sub(run_epoch) > 7200;

    // Also mark as complete if all agents are now done
    if run.status == RunStatus::Running && is_run_complete(&run) {
        run.status = RunStatus::Complete;
        any_agent_cleaned = true;
    }

    if any_agent_cleaned {
        run.updated_at = now;
        let _ = write_run(base, &run);
    }

    if !stale_complete && !stale_running && !any_agent_cleaned {
        return;
    }

    // Mark as complete if running and stale
    if stale_running {
        run.status = RunStatus::Complete;
        run.updated_at = crate::shared::helpers::now_iso();
        let _ = write_run(base, &run);
    }

    // Clean up agent directories — selectively remove only non-running ones
    let agents_dir = orchestrator_dir(base).join("agents");
    if agents_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&agents_dir)
    {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name == "_invalid" {
                continue;
            }
            let status_path = entry.path().join("status.json");
            if let Ok(content) = fs::read_to_string(&status_path) {
                let status: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
                let is_running = status["status"].as_str() == Some("running");
                if is_running {
                    // Check heartbeat staleness
                    let heartbeat = status["last_heartbeat"].as_str().unwrap_or("");
                    let heartbeat_age =
                        current_epoch.saturating_sub(epoch_secs_from_iso(heartbeat));
                    if heartbeat_age < 1800 {
                        continue; // still active
                    }
                }
            }
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Parse an ISO-8601 timestamp to epoch seconds.
/// Reuses the accurate `days_since_epoch` logic from mem::store::util.
fn epoch_secs_from_iso(iso: &str) -> u64 {
    crate::mem::store::parse_iso_to_secs(iso)
}

/// Current time as approximate epoch seconds.
fn epoch_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Parse the agent output to extract the terminal state.
/// Looks for known status patterns in the output text.
pub fn parse_agent_state(output: &str) -> Option<AgentStatus> {
    // Match the standard output format from agents
    if (output.contains("## Status: DONE") && !output.contains("DONE_WITH_CONCERNS"))
        || output.contains("## Status: DONE_WITH_CONCERNS")
    {
        Some(AgentStatus::Done)
    } else if output.contains("## Status: BLOCKED") || output.contains("## Status: NEEDS_CONTEXT") {
        Some(AgentStatus::Blocked)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test OrchestrationRun with given agents and deps.
    fn make_run(
        id: &str,
        agents: Vec<(&str, AgentStatus)>,
        deps: Vec<(&str, Vec<&str>)>,
    ) -> OrchestrationRun {
        OrchestrationRun {
            id: id.to_string(),
            status: RunStatus::Running,
            agents: agents
                .into_iter()
                .map(|(aid, status)| AgentDef {
                    id: aid.to_string(),
                    role: "builder".to_string(),
                    task: format!("task for {aid}"),
                    satisfies: vec![format!("R-{aid}")],
                    status,
                    started_at: None,
                    completed_at: None,
                })
                .collect(),
            dependency_graph: deps
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.into_iter().map(|s| s.to_string()).collect(),
                    )
                })
                .collect(),
            created_at: "2026-05-07T00:00:00Z".to_string(),
            updated_at: "2026-05-07T00:00:00Z".to_string(),
        }
    }

    // ── Test 1: Creating orchestration run directory with correct structure ──

    #[test]
    fn init_run_creates_directory_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "run-1",
            vec![
                ("agent-a", AgentStatus::Pending),
                ("agent-b", AgentStatus::Pending),
            ],
            vec![],
        );

        init_run(tmp.path(), &run).unwrap();

        // Check directories
        assert!(orchestrator_dir(tmp.path()).is_dir());
        assert!(agent_dir(tmp.path(), "agent-a").is_dir());
        assert!(agent_dir(tmp.path(), "agent-b").is_dir());

        // Check run.json exists and can be parsed
        let loaded = read_run(tmp.path()).unwrap();
        assert_eq!(loaded.id, "run-1");
        assert_eq!(loaded.agents.len(), 2);
        assert_eq!(loaded.status, RunStatus::Running);
    }

    #[test]
    fn init_run_creates_no_stale_tmp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("run-2", vec![("a1", AgentStatus::Pending)], vec![]);

        init_run(tmp.path(), &run).unwrap();

        let run_path = run_file(tmp.path());
        let tmp_path = run_path.with_extension("json.tmp");
        assert!(
            !tmp_path.exists(),
            "no .tmp file should remain after atomic write"
        );
    }

    // ── Test 2: Writing and reading agent status ──

    #[test]
    fn write_and_read_agent_status_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("run-3", vec![("builder-1", AgentStatus::Pending)], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let status = AgentStatusFile {
            agent_id: "builder-1".to_string(),
            phase: "executing".to_string(),
            progress: 0.5,
            last_heartbeat: "2026-05-07T10:00:00Z".to_string(),
            status: AgentStatus::Running,
        };

        write_agent_status(tmp.path(), "builder-1", &status).unwrap();

        let loaded = read_agent_status(tmp.path(), "builder-1").unwrap();
        assert_eq!(loaded.agent_id, "builder-1");
        assert_eq!(loaded.phase, "executing");
        assert_eq!(loaded.progress, 0.5);
        assert_eq!(loaded.status, AgentStatus::Running);
    }

    #[test]
    fn read_agent_status_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_agent_status(tmp.path(), "nonexistent").is_none());
    }

    #[test]
    fn write_agent_status_no_tmp_remains() {
        let tmp = tempfile::tempdir().unwrap();
        let status = AgentStatusFile {
            agent_id: "a".to_string(),
            phase: "init".to_string(),
            progress: 0.0,
            last_heartbeat: "2026-05-07T00:00:00Z".to_string(),
            status: AgentStatus::Pending,
        };
        write_agent_status(tmp.path(), "a", &status).unwrap();

        let path = agent_status_file(tmp.path(), "a");
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }

    // ── Test 3: Appending events to stream.jsonl ──

    #[test]
    fn append_and_read_events() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("run-4", vec![("a1", AgentStatus::Running)], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let event1 = AgentEvent {
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            event_type: "tool_call".to_string(),
            data: serde_json::json!({"tool": "Bash", "command": "cargo test"}),
        };
        let event2 = AgentEvent {
            timestamp: "2026-05-07T10:00:05Z".to_string(),
            event_type: "test_result".to_string(),
            data: serde_json::json!({"passed": 42, "failed": 0}),
        };

        append_event(tmp.path(), "a1", &event1).unwrap();
        append_event(tmp.path(), "a1", &event2).unwrap();

        let events = read_events(tmp.path(), "a1");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "tool_call");
        assert_eq!(events[1].event_type, "test_result");
        assert_eq!(events[1].data["passed"], 42);
    }

    #[test]
    fn read_events_missing_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let events = read_events(tmp.path(), "nonexistent");
        assert!(events.is_empty());
    }

    // ── Test 4: Evaluating dependency graph when agent completes ──

    #[test]
    fn evaluate_deps_unblocks_downstream() {
        let run = make_run(
            "run-deps",
            vec![
                ("builder", AgentStatus::Done),
                ("tester", AgentStatus::Blocked),
                ("reviewer", AgentStatus::Pending),
            ],
            vec![("tester", vec!["builder"]), ("reviewer", vec!["builder"])],
        );

        let mut unblocked = evaluate_dependencies(&run, "builder");
        unblocked.sort();
        assert_eq!(unblocked, vec!["reviewer", "tester"]);
    }

    #[test]
    fn evaluate_deps_not_all_deps_complete() {
        let run = make_run(
            "run-partial",
            vec![
                ("builder", AgentStatus::Running),
                ("tester", AgentStatus::Blocked),
            ],
            vec![("tester", vec!["builder"])],
        );

        // builder is not done yet (only the just-completed agent is considered)
        let unblocked = evaluate_dependencies(&run, "some-other-agent");
        assert!(unblocked.is_empty());
    }

    #[test]
    fn evaluate_deps_multiple_deps() {
        let run = make_run(
            "run-multi",
            vec![
                ("a", AgentStatus::Done),
                ("b", AgentStatus::Done),
                ("c", AgentStatus::Blocked),
            ],
            vec![("c", vec!["a", "b"])],
        );

        // "b" just completed, "a" was already done -> c is unblocked
        let unblocked = evaluate_dependencies(&run, "b");
        assert_eq!(unblocked, vec!["c"]);
    }

    #[test]
    fn evaluate_deps_no_deps_never_unblocked_by_this() {
        let run = make_run(
            "run-nodeps",
            vec![
                ("independent", AgentStatus::Pending),
                ("other", AgentStatus::Done),
            ],
            vec![],
        );

        let unblocked = evaluate_dependencies(&run, "other");
        assert!(
            unblocked.is_empty(),
            "agents with no deps should not appear"
        );
    }

    #[test]
    fn evaluate_deps_failed_counts_as_resolved() {
        let run = make_run(
            "run-failed",
            vec![
                ("flaky", AgentStatus::Failed),
                ("downstream", AgentStatus::Blocked),
            ],
            vec![("downstream", vec!["flaky"])],
        );

        let unblocked = evaluate_dependencies(&run, "flaky");
        assert_eq!(unblocked, vec!["downstream"]);
    }

    // ── Test 5: Control directive parsing ──

    #[test]
    fn write_and_read_control_directive() {
        let tmp = tempfile::tempdir().unwrap();
        let orch_dir = orchestrator_dir(tmp.path());
        fs::create_dir_all(&orch_dir).unwrap();

        let directive = ControlDirective {
            action: ControlAction::Pause,
            target: Some("agent-1".to_string()),
            message: Some("Waiting for approval".to_string()),
            generation: 1,
        };

        write_control(tmp.path(), &directive).unwrap();

        let loaded = read_control(tmp.path()).unwrap();
        assert_eq!(loaded.action, ControlAction::Pause);
        assert_eq!(loaded.target, Some("agent-1".to_string()));
        assert_eq!(loaded.generation, 1);
    }

    #[test]
    fn read_control_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_control(tmp.path()).is_none());
    }

    #[test]
    fn control_cancel_action() {
        let tmp = tempfile::tempdir().unwrap();
        let orch_dir = orchestrator_dir(tmp.path());
        fs::create_dir_all(&orch_dir).unwrap();

        let directive = ControlDirective {
            action: ControlAction::Cancel,
            target: None, // cancel all
            message: None,
            generation: 2,
        };
        write_control(tmp.path(), &directive).unwrap();

        let loaded = read_control(tmp.path()).unwrap();
        assert_eq!(loaded.action, ControlAction::Cancel);
        assert_eq!(loaded.target, None);
    }

    #[test]
    fn control_no_tmp_remains() {
        let tmp = tempfile::tempdir().unwrap();
        let orch_dir = orchestrator_dir(tmp.path());
        fs::create_dir_all(&orch_dir).unwrap();

        let directive = ControlDirective {
            action: ControlAction::Pause,
            target: None,
            message: None,
            generation: 1,
        };
        write_control(tmp.path(), &directive).unwrap();

        let path = control_file(tmp.path());
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }

    // ── Test 6: Generation-based invalidation ──

    #[test]
    fn generation_increments_on_write() {
        let tmp = tempfile::tempdir().unwrap();
        let orch_dir = orchestrator_dir(tmp.path());
        fs::create_dir_all(&orch_dir).unwrap();

        let d1 = ControlDirective {
            action: ControlAction::Pause,
            target: Some("a".to_string()),
            message: None,
            generation: 1,
        };
        write_control(tmp.path(), &d1).unwrap();

        let d2 = ControlDirective {
            action: ControlAction::Redirect,
            target: Some("b".to_string()),
            message: Some("new target".to_string()),
            generation: 2,
        };
        write_control(tmp.path(), &d2).unwrap();

        let loaded = read_control(tmp.path()).unwrap();
        assert_eq!(loaded.generation, 2);
        assert_eq!(loaded.action, ControlAction::Redirect);
    }

    #[test]
    fn old_generation_overwritten_by_newer() {
        let tmp = tempfile::tempdir().unwrap();
        let orch_dir = orchestrator_dir(tmp.path());
        fs::create_dir_all(&orch_dir).unwrap();

        // Write generation 3
        let d3 = ControlDirective {
            action: ControlAction::Cancel,
            target: None,
            message: None,
            generation: 3,
        };
        write_control(tmp.path(), &d3).unwrap();

        // Overwrite with generation 5
        let d5 = ControlDirective {
            action: ControlAction::Pause,
            target: Some("x".to_string()),
            message: None,
            generation: 5,
        };
        write_control(tmp.path(), &d5).unwrap();

        let loaded = read_control(tmp.path()).unwrap();
        assert_eq!(loaded.generation, 5);
        assert_eq!(loaded.action, ControlAction::Pause);
    }

    // ── Run completion checks ──

    #[test]
    fn is_run_complete_all_done() {
        let run = make_run(
            "complete",
            vec![("a", AgentStatus::Done), ("b", AgentStatus::Done)],
            vec![],
        );
        assert!(is_run_complete(&run));
    }

    #[test]
    fn is_run_complete_mixed_done_failed() {
        let run = make_run(
            "mixed",
            vec![("a", AgentStatus::Done), ("b", AgentStatus::Failed)],
            vec![],
        );
        assert!(is_run_complete(&run));
    }

    #[test]
    fn is_run_not_complete_with_running() {
        let run = make_run(
            "not-done",
            vec![("a", AgentStatus::Done), ("b", AgentStatus::Running)],
            vec![],
        );
        assert!(!is_run_complete(&run));
    }

    #[test]
    fn is_run_not_complete_with_blocked() {
        let run = make_run(
            "blocked",
            vec![("a", AgentStatus::Done), ("b", AgentStatus::Blocked)],
            vec![],
        );
        assert!(!is_run_complete(&run));
    }

    // ── Parse agent state ──

    #[test]
    fn parse_done_state() {
        let output = "## Status: DONE\n## Summary: All good";
        assert_eq!(parse_agent_state(output), Some(AgentStatus::Done));
    }

    #[test]
    fn parse_done_with_concerns_state() {
        let output = "## Status: DONE_WITH_CONCERNS\n## Summary: Mostly good";
        assert_eq!(parse_agent_state(output), Some(AgentStatus::Done));
    }

    #[test]
    fn parse_blocked_state() {
        let output = "## Status: BLOCKED\n## Summary: Need info";
        assert_eq!(parse_agent_state(output), Some(AgentStatus::Blocked));
    }

    #[test]
    fn parse_needs_context_state() {
        let output = "## Status: NEEDS_CONTEXT\n## Summary: Need more";
        assert_eq!(parse_agent_state(output), Some(AgentStatus::Blocked));
    }

    #[test]
    fn parse_unknown_output_none() {
        let output = "some random output without status marker";
        assert_eq!(parse_agent_state(output), None);
    }

    // ── Inbox messages ──

    #[test]
    fn post_and_read_inbox_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("inbox", vec![("agent-x", AgentStatus::Pending)], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let msg1 = InboxMessage {
            from: "agent-y".to_string(),
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            message: "R1 is complete, you can start".to_string(),
        };
        let msg2 = InboxMessage {
            from: "user".to_string(),
            timestamp: "2026-05-07T10:01:00Z".to_string(),
            message: "Focus on edge cases".to_string(),
        };

        post_inbox_message(tmp.path(), "agent-x", &msg1).unwrap();
        post_inbox_message(tmp.path(), "agent-x", &msg2).unwrap();

        let msgs = read_inbox(tmp.path(), "agent-x");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].from, "agent-y");
        assert_eq!(msgs[1].from, "user");
    }

    #[test]
    fn read_inbox_missing_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let msgs = read_inbox(tmp.path(), "nonexistent");
        assert!(msgs.is_empty());
    }

    // ── Max concurrent agents ──

    #[test]
    fn max_concurrent_agents_is_six() {
        assert_eq!(MAX_CONCURRENT_AGENTS, 6);
    }

    // ── Serialization roundtrip ──

    #[test]
    fn run_serialization_roundtrip() {
        let run = make_run(
            "ser-test",
            vec![("a1", AgentStatus::Running)],
            vec![("a1", vec!["b1"])],
        );
        let json = serde_json::to_string(&run).unwrap();
        let loaded: OrchestrationRun = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "ser-test");
        assert_eq!(loaded.dependency_graph["a1"], vec!["b1"]);
        assert_eq!(loaded.agents[0].status, AgentStatus::Running);
    }

    #[test]
    fn control_action_serialization() {
        let actions = vec![
            (ControlAction::Pause, "\"pause\""),
            (ControlAction::Cancel, "\"cancel\""),
            (ControlAction::Redirect, "\"redirect\""),
        ];
        for (action, expected) in actions {
            let json = serde_json::to_string(&action).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn agent_status_serialization() {
        let statuses = vec![
            (AgentStatus::Pending, "\"pending\""),
            (AgentStatus::Running, "\"running\""),
            (AgentStatus::Done, "\"done\""),
            (AgentStatus::Blocked, "\"blocked\""),
            (AgentStatus::Failed, "\"failed\""),
        ];
        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);
        }
    }

    // ── Atomic write permissions ──

    #[cfg(unix)]
    #[test]
    fn atomic_write_sets_600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.json");

        let data = serde_json::json!({"key": "value"});
        atomic_write_json(&path, &data).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "file must be owner-read/write only");
    }
}
