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
    Reassign,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Handoff,
    Question,
    Blocked,
    Result,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxMessage {
    pub id: String,
    pub from: String,
    pub timestamp: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_type: Option<MessageType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlDirective {
    pub action: ControlAction,
    pub target: Option<String>,
    pub message: Option<String>,
    pub generation: u64,
}

// ── Constants ─────────────────────────────────────────

/// Maximum number of concurrent agents per orchestration run.
/// Used in Phase 3 (R14: Dynamic Agent Spawning) to cap agent count.
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
    debug_assert!(validate_agent_id(agent_id), "invalid agent_id: {agent_id}");
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

/// Error type for `read_jsonl`.
#[derive(Debug)]
pub enum ReadJsonlError {
    FileTooLarge(u64),
    Io(std::io::Error),
}

/// Read all lines from a JSONL file as typed records.
/// Returns `Err(ReadJsonlError::FileTooLarge)` if file exceeds 10 MB.
pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, ReadJsonlError> {
    const MAX_JSONL_BYTES: u64 = 10 * 1024 * 1024;
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > MAX_JSONL_BYTES {
        return Err(ReadJsonlError::FileTooLarge(size));
    }
    Ok(fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
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
    read_jsonl(&agent_stream_file(base, agent_id)).unwrap_or_default()
}

/// Post a message to an agent's inbox.
/// If `msg.id` is empty, an ID is auto-generated from a hash of from+timestamp+message.
pub fn post_inbox_message(base: &Path, agent_id: &str, msg: &InboxMessage) -> io::Result<()> {
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;
    let msg = if msg.id.is_empty() {
        let hash_input = format!("{}{}{}", msg.from, msg.timestamp, msg.message);
        let id = format!(
            "msg-{:x}",
            hash_input
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
        );
        InboxMessage { id, ..msg.clone() }
    } else {
        msg.clone()
    };
    append_jsonl(&agent_inbox_file(base, agent_id), &msg)
}

/// Mark an inbox message as read by setting its `read_at` timestamp.
/// Rewrites the entire inbox.jsonl atomically with the updated entry.
pub fn mark_inbox_read(base: &Path, agent_id: &str, msg_id: &str) -> io::Result<()> {
    let path = agent_inbox_file(base, agent_id);
    let messages: Vec<InboxMessage> = read_jsonl(&path).unwrap_or_default();
    let now = crate::hooks::common::now_iso();
    let updated: Vec<InboxMessage> = messages
        .into_iter()
        .map(|mut m| {
            if m.id == msg_id && m.read_at.is_none() {
                m.read_at = Some(now.clone());
            }
            m
        })
        .collect();
    let tmp = path.with_extension("jsonl.tmp");
    {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        for m in &updated {
            let json = serde_json::to_string(m).map_err(io::Error::other)?;
            writeln!(f, "{json}")?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Read only unread inbox messages (`read_at` is `None`).
pub fn read_inbox_unread(base: &Path, agent_id: &str) -> Vec<InboxMessage> {
    read_inbox(base, agent_id)
        .into_iter()
        .filter(|m| m.read_at.is_none())
        .collect()
}

/// Read all inbox messages for an agent.
pub fn read_inbox(base: &Path, agent_id: &str) -> Vec<InboxMessage> {
    read_jsonl(&agent_inbox_file(base, agent_id)).unwrap_or_default()
}

/// Send a typed message from one agent to another agent's inbox.
pub fn send_message(
    base: &Path,
    from: &str,
    to: &str,
    msg_type: MessageType,
    body: &str,
) -> io::Result<()> {
    if !validate_agent_id(from) || !validate_agent_id(to) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid agent_id",
        ));
    }
    let now = crate::hooks::common::now_iso();
    let raw = format!("{from}{to}{now}{body}");
    let id = format!(
        "msg-{:x}",
        raw.bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
    );
    let msg = InboxMessage {
        id,
        from: from.to_string(),
        timestamp: now,
        message: body.to_string(),
        msg_type: Some(msg_type),
        read_at: None,
    };
    post_inbox_message(base, to, &msg)
}

/// Add a new agent to an existing orchestration run at runtime.
/// The agents list is append-only — existing agents are never removed or modified.
/// Creates the agent's directory structure and updates run.json atomically.
pub fn add_agent(base: &Path, run_id: &str, agent_def: AgentDef) -> io::Result<()> {
    if !validate_agent_id(&agent_def.id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid agent_id: {}", agent_def.id),
        ));
    }
    let mut run = read_run(base)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "run.json not found"))?;
    if run.id != run_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("run_id mismatch: expected {}, got {}", run.id, run_id),
        ));
    }
    // Check max concurrent agents
    let active = run
        .agents
        .iter()
        .filter(|a| a.status == AgentStatus::Running || a.status == AgentStatus::Pending)
        .count();
    if active >= MAX_CONCURRENT_AGENTS {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!("max concurrent agents ({MAX_CONCURRENT_AGENTS}) reached"),
        ));
    }
    // Create agent directory
    let a_dir = agent_dir(base, &agent_def.id);
    fs::create_dir_all(&a_dir)?;
    // Append agent (never modify existing)
    run.agents.push(agent_def);
    run.updated_at = crate::hooks::common::now_iso();
    write_run(base, &run)
}

/// Self-register the current agent using EPIC_AGENT_ID and EPIC_RUN_ID env vars.
/// No-op if either env var is missing or agent already exists in run.
pub fn self_register_agent(base: &Path, role: &str, task: &str) -> io::Result<()> {
    let agent_id = match std::env::var("EPIC_AGENT_ID") {
        Ok(id) if !id.is_empty() => id,
        _ => return Ok(()), // no-op
    };
    let run_id = match std::env::var("EPIC_RUN_ID") {
        Ok(id) if !id.is_empty() => id,
        _ => return Ok(()), // no-op
    };
    // Check if already registered
    if let Some(run) = read_run(base)
        && run.agents.iter().any(|a| a.id == agent_id)
    {
        return Ok(()); // already exists, no-op
    }
    let agent_def = AgentDef {
        id: agent_id,
        role: role.to_string(),
        task: task.to_string(),
        satisfies: vec![],
        status: AgentStatus::Running,
        started_at: Some(crate::hooks::common::now_iso()),
        completed_at: None,
    };
    add_agent(base, &run_id, agent_def)
}

/// Reassign a blocked/failed agent's task to another agent.
///
/// - Copies `from_agent`'s task description to `to_agent`'s inbox as a `Handoff` message
/// - Sets `from_agent` status to `AgentStatus::Failed` (task handed off)
/// - Updates run.json atomically
pub fn reassign_agent(base: &Path, from_id: &str, to_id: &str) -> io::Result<()> {
    if !validate_agent_id(from_id) || !validate_agent_id(to_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid agent_id",
        ));
    }
    let mut run = read_run(base)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "run.json not found"))?;

    // Find from agent and extract its task
    let from_task = run
        .agents
        .iter()
        .find(|a| a.id == from_id)
        .map(|a| a.task.clone())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("agent {from_id} not found"),
            )
        })?;

    // Verify to agent exists
    if !run.agents.iter().any(|a| a.id == to_id) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("agent {to_id} not found"),
        ));
    }

    // Mark from_agent as Failed (task handed off)
    for agent in &mut run.agents {
        if agent.id == from_id {
            agent.status = AgentStatus::Failed;
            agent.completed_at = Some(crate::hooks::common::now_iso());
        }
    }
    run.updated_at = crate::hooks::common::now_iso();
    write_run(base, &run)?;

    // Send task to to_agent inbox as Handoff
    send_message(
        base,
        "orchestrator",
        to_id,
        MessageType::Handoff,
        &format!("Reassigned task from {from_id}: {from_task}"),
    )
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

/// Parse the agent output to extract the terminal state.
/// Only matches status markers at line start or document start to prevent false positives
/// (e.g., `"The old ## Status: DONE format was deprecated"` must not match).
pub fn parse_agent_state(output: &str) -> Option<AgentStatus> {
    let is_status_line = |marker: &str| -> bool {
        output == marker || output.starts_with(marker) || output.contains(&format!("\n{marker}"))
    };

    if is_status_line("## Status: DONE_WITH_CONCERNS") || is_status_line("## Status: DONE") {
        Some(AgentStatus::Done)
    } else if is_status_line("## Status: BLOCKED") || is_status_line("## Status: NEEDS_CONTEXT") {
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
            id: "".to_string(), // auto-generated
            from: "agent-y".to_string(),
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            message: "R1 is complete, you can start".to_string(),
            msg_type: None,
            read_at: None,
        };
        let msg2 = InboxMessage {
            id: "".to_string(), // auto-generated
            from: "user".to_string(),
            timestamp: "2026-05-07T10:01:00Z".to_string(),
            message: "Focus on edge cases".to_string(),
            msg_type: None,
            read_at: None,
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

    #[test]
    fn post_inbox_auto_generates_id() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "inbox-auto-id",
            vec![("agent-x", AgentStatus::Pending)],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        let msg = InboxMessage {
            id: "".to_string(),
            from: "agent-y".to_string(),
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            message: "hello".to_string(),
            msg_type: None,
            read_at: None,
        };
        post_inbox_message(tmp.path(), "agent-x", &msg).unwrap();

        let msgs = read_inbox(tmp.path(), "agent-x");
        assert!(!msgs[0].id.is_empty(), "id should be auto-generated");
        assert!(msgs[0].id.starts_with("msg-"), "id should start with msg-");
    }

    #[test]
    fn mark_inbox_read_updates_read_at() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "inbox-read",
            vec![("agent-x", AgentStatus::Pending)],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        let msg = InboxMessage {
            id: "msg-001".to_string(),
            from: "agent-y".to_string(),
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            message: "start now".to_string(),
            msg_type: None,
            read_at: None,
        };
        post_inbox_message(tmp.path(), "agent-x", &msg).unwrap();

        // Before mark: read_at is None
        let msgs = read_inbox(tmp.path(), "agent-x");
        assert!(msgs[0].read_at.is_none());

        // Mark as read
        mark_inbox_read(tmp.path(), "agent-x", "msg-001").unwrap();

        // After mark: read_at is set
        let msgs = read_inbox(tmp.path(), "agent-x");
        assert!(msgs[0].read_at.is_some());
    }

    #[test]
    fn read_inbox_unread_filters_read_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "inbox-unread",
            vec![("agent-x", AgentStatus::Pending)],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        let msg1 = InboxMessage {
            id: "msg-001".to_string(),
            from: "a".to_string(),
            timestamp: "2026-05-07T10:00:00Z".to_string(),
            message: "first".to_string(),
            msg_type: None,
            read_at: Some("2026-05-07T10:01:00Z".to_string()), // already read
        };
        let msg2 = InboxMessage {
            id: "msg-002".to_string(),
            from: "b".to_string(),
            timestamp: "2026-05-07T10:01:00Z".to_string(),
            message: "second".to_string(),
            msg_type: None,
            read_at: None, // unread
        };
        post_inbox_message(tmp.path(), "agent-x", &msg1).unwrap();
        post_inbox_message(tmp.path(), "agent-x", &msg2).unwrap();

        let unread = read_inbox_unread(tmp.path(), "agent-x");
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].id, "msg-002");
    }

    // ── parse_agent_state false positive prevention (R5) ──

    #[test]
    fn parse_done_false_positive_prevented() {
        // "## Status: DONE" が行の先頭でない場合 → None
        let output = "The old ## Status: DONE format was deprecated in v2";
        assert_eq!(parse_agent_state(output), None);
    }

    #[test]
    fn parse_done_at_line_start() {
        let output = "Some preamble\n## Status: DONE\nSome postamble";
        assert_eq!(parse_agent_state(output), Some(AgentStatus::Done));
    }

    // ── read_jsonl large file error (R6) ──

    #[test]
    fn read_jsonl_returns_err_for_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.jsonl");
        // 11MB 파일 생성
        let line = "{\"seq\":1}\n";
        let content = line.repeat(11 * 1024 * 1024 / line.len() + 1);
        fs::write(&path, content).unwrap();
        let result: Result<Vec<serde_json::Value>, _> = read_jsonl(&path);
        assert!(matches!(result, Err(ReadJsonlError::FileTooLarge(_))));
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

    // ── R12: send_message ──

    #[test]
    fn send_message_delivers_to_inbox() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "msg-test",
            vec![
                ("agent-a", AgentStatus::Running),
                ("agent-b", AgentStatus::Pending),
            ],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        send_message(
            tmp.path(),
            "agent-a",
            "agent-b",
            MessageType::Handoff,
            "R1 complete",
        )
        .unwrap();

        let msgs = read_inbox(tmp.path(), "agent-b");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from, "agent-a");
        assert_eq!(msgs[0].msg_type, Some(MessageType::Handoff));
        assert!(msgs[0].message.contains("R1 complete"));
        assert!(msgs[0].read_at.is_none());
    }

    #[test]
    fn send_message_rejects_invalid_agent_id() {
        let tmp = tempfile::tempdir().unwrap();
        let result = send_message(
            tmp.path(),
            "../evil",
            "agent-b",
            MessageType::Result,
            "test",
        );
        assert!(result.is_err());
    }

    // ── R13: push notification uses Handoff type ──

    #[test]
    fn push_notification_uses_handoff_type() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "push-test",
            vec![
                ("builder", AgentStatus::Done),
                ("tester", AgentStatus::Blocked),
            ],
            vec![("tester", vec!["builder"])],
        );
        init_run(tmp.path(), &run).unwrap();

        // Simulate orchestrator sending handoff push notification
        send_message(
            tmp.path(),
            "orchestrator",
            "tester",
            MessageType::Handoff,
            "Dependency 'builder' completed. You are unblocked.",
        )
        .unwrap();

        let msgs = read_inbox_unread(tmp.path(), "tester");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from, "orchestrator");
        assert_eq!(msgs[0].msg_type, Some(MessageType::Handoff));
        assert!(msgs[0].message.contains("unblocked"));
        assert!(msgs[0].read_at.is_none(), "message should be unread");
    }

    #[test]
    fn push_notification_unblocks_agent_with_multiple_deps() {
        let tmp = tempfile::tempdir().unwrap();
        // tester depends on both builder and linter; both are done
        let run = make_run(
            "push-multi-dep",
            vec![
                ("builder", AgentStatus::Done),
                ("linter", AgentStatus::Done),
                ("tester", AgentStatus::Blocked),
            ],
            vec![("tester", vec!["builder", "linter"])],
        );
        init_run(tmp.path(), &run).unwrap();

        // evaluate_dependencies sees both builder and linter as finished,
        // so tester is unblocked when linter completes
        let unblocked = evaluate_dependencies(&run, "linter");
        assert_eq!(unblocked, vec!["tester"]);

        // Send the handoff notification
        send_message(
            tmp.path(),
            "orchestrator",
            "tester",
            MessageType::Handoff,
            "Dependency 'linter' completed. You are unblocked.",
        )
        .unwrap();

        let msgs = read_inbox_unread(tmp.path(), "tester");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].msg_type, Some(MessageType::Handoff));
    }

    #[test]
    fn message_type_serialization() {
        let types = vec![
            (MessageType::Handoff, "\"handoff\""),
            (MessageType::Question, "\"question\""),
            (MessageType::Blocked, "\"blocked\""),
            (MessageType::Result, "\"result\""),
        ];
        for (t, expected) in types {
            assert_eq!(serde_json::to_string(&t).unwrap(), expected);
        }
    }

    // ── R14: Dynamic Agent Spawning ──

    #[test]
    fn add_agent_appends_to_run() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("dyn-test", vec![("agent-a", AgentStatus::Running)], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let new_agent = AgentDef {
            id: "agent-b".to_string(),
            role: "tester".to_string(),
            task: "run tests".to_string(),
            satisfies: vec!["R2".to_string()],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        add_agent(tmp.path(), "dyn-test", new_agent).unwrap();

        let loaded = read_run(tmp.path()).unwrap();
        assert_eq!(loaded.agents.len(), 2);
        assert_eq!(loaded.agents[1].id, "agent-b");
        assert_eq!(loaded.agents[0].id, "agent-a"); // existing unchanged
    }

    #[test]
    fn add_agent_rejects_invalid_id() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("dyn-invalid", vec![], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let bad_agent = AgentDef {
            id: "../evil".to_string(),
            role: "bad".to_string(),
            task: "escape".to_string(),
            satisfies: vec![],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        assert!(add_agent(tmp.path(), "dyn-invalid", bad_agent).is_err());
    }

    #[test]
    fn add_agent_enforces_max_concurrent() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_ids = ["a0", "a1", "a2", "a3", "a4", "a5"]; // MAX_CONCURRENT_AGENTS = 6
        assert_eq!(agent_ids.len(), MAX_CONCURRENT_AGENTS);
        let agents: Vec<(&str, AgentStatus)> = agent_ids
            .iter()
            .map(|id| (*id, AgentStatus::Running))
            .collect();
        let run = make_run("max-test", agents, vec![]);
        init_run(tmp.path(), &run).unwrap();

        let extra = AgentDef {
            id: "agent-extra".to_string(),
            role: "extra".to_string(),
            task: "overflow".to_string(),
            satisfies: vec![],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        assert!(matches!(
            add_agent(tmp.path(), "max-test", extra),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn add_agent_creates_agent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("dir-test", vec![], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let new_agent = AgentDef {
            id: "agent-new".to_string(),
            role: "builder".to_string(),
            task: "build".to_string(),
            satisfies: vec![],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        add_agent(tmp.path(), "dir-test", new_agent).unwrap();

        assert!(agent_dir(tmp.path(), "agent-new").is_dir());
    }

    #[test]
    fn add_agent_rejects_run_id_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("correct-id", vec![], vec![]);
        init_run(tmp.path(), &run).unwrap();

        let new_agent = AgentDef {
            id: "agent-x".to_string(),
            role: "builder".to_string(),
            task: "build".to_string(),
            satisfies: vec![],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        let result = add_agent(tmp.path(), "wrong-id", new_agent);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn add_agent_returns_not_found_when_no_run() {
        let tmp = tempfile::tempdir().unwrap();
        // orchestrator dir exists but run.json does not
        fs::create_dir_all(orchestrator_dir(tmp.path())).unwrap();

        let new_agent = AgentDef {
            id: "agent-x".to_string(),
            role: "builder".to_string(),
            task: "build".to_string(),
            satisfies: vec![],
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
        };
        let result = add_agent(tmp.path(), "any-id", new_agent);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    // ── R15: Agent Load Balancing — reassign_agent ──

    #[test]
    fn reassign_moves_task_to_inbox() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "reassign-test",
            vec![
                ("agent-blocked", AgentStatus::Blocked),
                ("agent-free", AgentStatus::Pending),
            ],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        reassign_agent(tmp.path(), "agent-blocked", "agent-free").unwrap();

        // from_agent should be Failed
        let loaded = read_run(tmp.path()).unwrap();
        let from = loaded
            .agents
            .iter()
            .find(|a| a.id == "agent-blocked")
            .unwrap();
        assert_eq!(from.status, AgentStatus::Failed);

        // to_agent inbox should have the handoff
        let msgs = read_inbox_unread(tmp.path(), "agent-free");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].msg_type, Some(MessageType::Handoff));
        assert!(msgs[0].message.contains("agent-blocked"));
    }

    #[test]
    fn reassign_rejects_invalid_agent_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run("reassign-invalid", vec![], vec![]);
        init_run(tmp.path(), &run).unwrap();

        assert!(reassign_agent(tmp.path(), "../evil", "agent-b").is_err());
        assert!(reassign_agent(tmp.path(), "agent-a", "../../etc").is_err());
    }

    #[test]
    fn reassign_rejects_unknown_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let run = make_run(
            "reassign-unknown",
            vec![("agent-a", AgentStatus::Blocked)],
            vec![],
        );
        init_run(tmp.path(), &run).unwrap();

        // to_agent doesn't exist
        assert!(reassign_agent(tmp.path(), "agent-a", "nonexistent").is_err());
    }

    #[test]
    fn control_action_reassign_serialization() {
        let json = serde_json::to_string(&ControlAction::Reassign).unwrap();
        assert_eq!(json, "\"reassign\"");
        let action: ControlAction = serde_json::from_str("\"reassign\"").unwrap();
        assert_eq!(action, ControlAction::Reassign);
    }
}
