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
    /// Written by `/intervene cancel {agent_id}` (see the orchestrate skill).
    Cancelled,
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

impl ControlDirective {
    /// True when the directive addresses this agent (its id, `"all"`, or no target).
    pub fn targets(&self, agent_id: &str) -> bool {
        self.target.as_deref() == Some(agent_id)
            || self.target.as_deref() == Some("all")
            || self.target.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    RunCancelled,
    AgentCancelled,
    Paused,
    DirectiveCancelled,
}

/// Why an Agent spawn is refused (see `spawn_block`).
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnBlock {
    pub kind: BlockKind,
    pub run_id: String,
    /// Directive kinds only.
    pub generation: Option<u64>,
    /// Directive target as written; None means all. Directive kinds only.
    pub target: Option<String>,
    /// Directive message, trimmed; None when absent or empty.
    pub message: Option<String>,
    /// `iso_from_secs(parse(run.updated_at) + RUN_IDLE_SECS)`
    pub expires_at: String,
}

impl SpawnBlock {
    /// Deny reason lines without the "[orchestrator] " tag.
    pub fn reason_lines(&self, agent_id: &str) -> Vec<String> {
        let run_id = &self.run_id;
        let gen_target = format!(
            "gen {}, target {}",
            self.generation.unwrap_or_default(),
            self.target.as_deref().unwrap_or("all")
        );
        let (first, lift) = match self.kind {
            BlockKind::RunCancelled => (
                format!(
                    "BLOCKED: agent {agent_id} not started: orchestration run {run_id} was cancelled by the user (/intervene cancel all)."
                ),
                "all",
            ),
            BlockKind::AgentCancelled => (
                format!(
                    "BLOCKED: agent {agent_id} not started: it was cancelled by the user in run {run_id} (/intervene cancel {agent_id})."
                ),
                agent_id,
            ),
            BlockKind::Paused => (
                format!(
                    "BLOCKED: agent {agent_id} not started: paused by user directive ({gen_target})."
                ),
                self.directive_lift(agent_id),
            ),
            BlockKind::DirectiveCancelled => (
                format!(
                    "BLOCKED: agent {agent_id} not started: cancelled by user directive ({gen_target})."
                ),
                self.directive_lift(agent_id),
            ),
        };
        let mut lines = vec![first];
        if let Some(message) = &self.message {
            let message: String = message.chars().take(200).collect();
            lines.push(format!("Reason: {message}"));
        }
        lines.push(format!(
            "To lift: the user runs /intervene resume {lift}. Otherwise the block ends by itself after 2 hours without run activity (earliest {}).",
            self.expires_at
        ));
        lines.push(
            "Do not retry this spawn and do not run /intervene yourself; tell the user."
                .to_string(),
        );
        lines
    }

    /// `{x}` when the directive names this agent, else `all`.
    fn directive_lift<'a>(&'a self, agent_id: &'a str) -> &'a str {
        if self.target.as_deref() == Some(agent_id) {
            agent_id
        } else {
            "all"
        }
    }
}

// ── Constants ─────────────────────────────────────────

pub const MAX_CONCURRENT_AGENTS: usize = 6;
/// A run with no activity for this long holds no pause or cancel block.
pub const RUN_IDLE_SECS: u64 = 7200;
/// Run timestamps further than this in the future are invalid (run treated as not live).
const CLOCK_SKEW_SECS: u64 = 300;
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
    crate::shared::helpers::parse_persisted_json(&content).ok()
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
        if matches!(
            agent.status,
            AgentStatus::Done | AgentStatus::Failed | AgentStatus::Cancelled
        ) {
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

/// Check if the run is complete: all agents are Done, Failed or Cancelled.
pub fn is_run_complete(run: &OrchestrationRun) -> bool {
    run.agents.iter().all(|a| {
        matches!(
            a.status,
            AgentStatus::Done | AgentStatus::Failed | AgentStatus::Cancelled
        )
    })
}

/// Acquire an exclusive advisory file lock on `path`.
/// Returns the locked file handle (holds the lock until dropped).
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
    // Non-blocking exclusive lock — if contended, fall back to blocking
    // (hooks are sequential within a session, contention is cross-session)
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let result = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

/// `File::lock` is `LockFileEx` on Windows. The previous fallback only opened
/// the file and excluded nothing, so concurrent agent spawns lost updates.
#[cfg(not(unix))]
fn acquire_lock(lock_path: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    file.lock()?;
    Ok(file)
}

fn mtime_secs(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Last activity of the run in epoch seconds, as far as blocks are concerned;
/// 0 when unknown (malformed `updated_at`).
///
/// A timestamp beyond the clock skew allowance (a model-written local time
/// without offset, a wrong clock) is replaced by the run.json mtime, which
/// comes from the harness host clock. An aborted run with a bound control.json
/// is anchored at the earlier of that and control.json's mtime: denied spawns
/// rewrite run.json, and they must not extend a cancel. Without a bound
/// control.json there is no such cap, so a future `updated_at` on an aborted
/// run is unknown (0, not live) instead of falling back to that mtime.
fn run_activity_secs(base: &Path, run: &OrchestrationRun, now: u64) -> u64 {
    let t = epoch_secs_from_iso(&run.updated_at);
    if t == 0 {
        return 0;
    }
    let bound_control = if run.status == RunStatus::Aborted && control_is_bound(base, run) {
        mtime_secs(&control_file(base))
    } else {
        None
    };
    let t = if t > now + CLOCK_SKEW_SECS {
        if run.status == RunStatus::Aborted && bound_control.is_none() {
            return 0;
        }
        mtime_secs(&run_file(base)).unwrap_or(0)
    } else {
        t
    };
    bound_control.map_or(t, |c| t.min(c))
}

/// Earliest time the run's blocks end by themselves.
pub(crate) fn run_expiry(base: &Path, run: &OrchestrationRun, now: u64) -> String {
    crate::shared::helpers::iso_from_secs(run_activity_secs(base, run, now) + RUN_IDLE_SECS)
}

/// A run is live while its last activity is at most `RUN_IDLE_SECS` old.
/// Derived, never persisted: a malformed `updated_at` is not live. Agents do
/// not count as activity: every spawn, completion and intervention writes
/// `updated_at`, so a block set by an intervention lasts 2h from then.
pub fn is_live(base: &Path, run: &OrchestrationRun, now: u64) -> bool {
    let t = run_activity_secs(base, run, now);
    t != 0 && t <= now + CLOCK_SKEW_SECS && now.saturating_sub(t) <= RUN_IDLE_SECS
}

/// control.json counts for a run only when it was written during that run:
/// its mtime (whole seconds) is at or after `run.created_at`.
pub fn control_is_bound(base: &Path, run: &OrchestrationRun) -> bool {
    let created = epoch_secs_from_iso(&run.created_at);
    created != 0 && mtime_secs(&control_file(base)).is_some_and(|m| m >= created)
}

/// Whether an Agent spawn is blocked. Pure apart from reading run.json,
/// control.json and its mtime. Never writes.
pub fn spawn_block(base: &Path, agent_id: &str, now: u64) -> Option<SpawnBlock> {
    let run = read_run(base)?;
    if !is_live(base, &run, now) {
        return None;
    }
    let directive = if control_is_bound(base, &run) {
        read_control(base)
    } else {
        None
    };
    let block = |kind, d: Option<&ControlDirective>| SpawnBlock {
        kind,
        run_id: run.id.clone(),
        generation: d.map(|d| d.generation),
        target: d.and_then(|d| d.target.clone()),
        message: d
            .and_then(|d| d.message.as_deref())
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(String::from),
        expires_at: run_expiry(base, &run, now),
    };
    let targeted = directive.as_ref().filter(|d| d.targets(agent_id));
    let resumes = targeted.is_some_and(|d| d.action == ControlAction::Resume);

    if run.status == RunStatus::Aborted {
        let lifted = directive.as_ref().is_some_and(|d| {
            d.action == ControlAction::Resume && matches!(d.target.as_deref(), None | Some("all"))
        });
        return (!lifted).then(|| block(BlockKind::RunCancelled, None));
    }
    if run
        .agents
        .iter()
        .any(|a| a.id == agent_id && a.status == AgentStatus::Cancelled)
    {
        return (!resumes).then(|| block(BlockKind::AgentCancelled, None));
    }
    let d = targeted?;
    match d.action {
        ControlAction::Pause => Some(block(BlockKind::Paused, Some(d))),
        ControlAction::Cancel => Some(block(BlockKind::DirectiveCancelled, Some(d))),
        ControlAction::Redirect | ControlAction::Resume => None,
    }
}

/// Upsert an agent into an auto-created run.json.
///
/// If `run.json` does not exist, creates a new auto-run with id `auto-{timestamp}`.
/// A `Running` request also starts a new auto-run when the existing one is
/// complete, aborted or idle (not live): the previous run.json is replaced.
/// Otherwise it adds the agent or updates its status.
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
        // Only a `Running` request rotates (I2); anything else records into the
        // existing run. An aborted run is replaced only by rotation (I1).
        Some(r)
            if status != AgentStatus::Running
                || (!matches!(r.status, RunStatus::Complete | RunStatus::Aborted)
                    && is_live(base, &r, epoch_secs_now())) =>
        {
            r
        }
        _ => OrchestrationRun {
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

    // A user-cancelled agent stays Cancelled for every request except `Running`:
    // `run_pre` requests it only when the gate allowed the spawn (I3), i.e. the
    // user resumed the agent (I4). A late Done, Blocked or Pending must not
    // undo `/intervene cancel`.
    let status = if status != AgentStatus::Running
        && run
            .agents
            .iter()
            .any(|a| a.id == agent_id && a.status == AgentStatus::Cancelled)
    {
        AgentStatus::Cancelled
    } else {
        status
    };

    // Ensure agent directory exists
    let a_dir = agent_dir(base, agent_id);
    fs::create_dir_all(&a_dir)?;

    if let Some(existing) = run.agents.iter_mut().find(|a| a.id == agent_id) {
        let was_running = existing.status == AgentStatus::Running;
        existing.status = status.clone();
        if matches!(status, AgentStatus::Done | AgentStatus::Failed) {
            existing.completed_at = Some(now.clone());
        }
        if status == AgentStatus::Running {
            // A restart: the previous completion no longer applies, and the
            // old started_at would make stale-heartbeat cleanup end it early.
            // (Repeated Running updates keep the original start.)
            existing.completed_at = None;
            if !was_running || existing.started_at.is_none() {
                existing.started_at = Some(now.clone());
            }
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
            completed_at: if matches!(status, AgentStatus::Done | AgentStatus::Failed) {
                Some(now.clone())
            } else {
                None
            },
        });
    }

    // An aborted run keeps its status and `updated_at` (I1): recomputing or
    // bumping either would lift `cancel all` early or extend it on every denied
    // retry.
    if run.status != RunStatus::Aborted {
        if is_run_complete(&run) {
            run.status = RunStatus::Complete;
        }
        run.updated_at = now.clone();
    }
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
            phase: match status {
                AgentStatus::Done => "complete",
                AgentStatus::Running => "running",
                AgentStatus::Failed => "failed",
                AgentStatus::Cancelled => "cancelled",
                AgentStatus::Blocked => "blocked",
                AgentStatus::Pending => "pending",
            }
            .to_string(),
            progress: if matches!(status, AgentStatus::Done | AgentStatus::Failed) {
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

/// Mark `agent_id` Done or Failed and unblock its dependents, all under one
/// `run.json.lock` acquisition (never nested: `upsert_agent_to_run` and
/// `auto_cleanup_stale_runs` take the same lock). Returns the unblocked ids.
///
/// Any other `status` is a no-op. Per-agent status files are written after
/// run.json and a failure there is only logged, since run.json is the source
/// of truth. Known limitation: `Blocked` is shared with dependency blocking,
/// so a paused agent can be flipped to `Pending` when its dependency finishes.
pub fn complete_and_unblock(
    base: &Path,
    agent_id: &str,
    status: AgentStatus,
) -> io::Result<Vec<String>> {
    if !matches!(status, AgentStatus::Done | AgentStatus::Failed) {
        return Ok(vec![]);
    }
    let orch_dir = orchestrator_dir(base);
    fs::create_dir_all(&orch_dir)?;
    let _lock = acquire_lock(&orch_dir.join("run.json.lock"))?;

    let Some(mut run) = read_run(base) else {
        return Ok(vec![]);
    };
    // Aborted stays aborted (I1); the caller's upsert already recorded the agent.
    if run.status == RunStatus::Aborted {
        return Ok(vec![]);
    }
    let now = crate::shared::helpers::now_iso();

    for agent in run.agents.iter_mut().filter(|a| a.id == agent_id) {
        agent.status = status.clone();
        agent.completed_at = Some(now.clone());
    }
    let unblocked = evaluate_dependencies(&run, agent_id);
    for agent in run.agents.iter_mut().filter(|a| unblocked.contains(&a.id)) {
        agent.status = AgentStatus::Pending;
    }
    if is_run_complete(&run) {
        run.status = RunStatus::Complete;
    }
    run.updated_at = now.clone();
    write_run(base, &run)?;

    for id in &unblocked {
        let status_file = AgentStatusFile {
            agent_id: id.clone(),
            phase: "pending".to_string(),
            progress: 0.0,
            last_heartbeat: now.clone(),
            status: AgentStatus::Pending,
        };
        if let Err(e) = write_agent_status(base, id, &status_file) {
            eprintln!("[harness] orchestrate: status write failed for {id}: {e}");
        }
    }
    Ok(unblocked)
}

/// Clean up stale auto-generated runs.
///
/// - Complete runs older than 1 hour: remove agent status directories
/// - Running runs older than 2 hours: mark as complete, then clean
/// - Individual agents with neither a start nor a heartbeat in the last 30 min: mark as done
pub fn auto_cleanup_stale_runs(base: &Path) {
    let orch_dir = orchestrator_dir(base);
    let lock_path = orch_dir.join("run.json.lock");
    let _lock = acquire_lock(&lock_path);

    let Some(mut run) = read_run(base) else {
        return;
    };

    // Only auto-cleanup auto-generated runs
    if !run.id.starts_with("auto-") {
        return;
    }
    // Aborted stays aborted and keeps its `updated_at` (I1).
    if run.status == RunStatus::Aborted {
        return;
    }

    let current_epoch = epoch_secs_now();
    let now = crate::shared::helpers::now_iso();

    // Mark individual stale agents as done (no start or heartbeat in 30 min)
    let mut any_agent_cleaned = false;
    for agent in &mut run.agents {
        if agent.status != AgentStatus::Running {
            continue;
        }
        let started = agent.started_at.as_deref().map(epoch_secs_from_iso);
        let heartbeat =
            read_agent_status(base, &agent.id).map(|s| epoch_secs_from_iso(&s.last_heartbeat));
        let last_alive = started.into_iter().chain(heartbeat).max();
        let stale = last_alive.is_none_or(|t| current_epoch.saturating_sub(t) > 1800); // 30 min
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
    // Same liveness as the gate and rotation.
    let stale_running = run.status == RunStatus::Running && !is_live(base, &run, current_epoch);

    // Also mark as complete if all agents are now done
    if run.status == RunStatus::Running && is_run_complete(&run) {
        run.status = RunStatus::Complete;
        any_agent_cleaned = true;
    }

    // Cleanup is not run activity: `updated_at` stays, else a session start
    // would make an expired run live again and revive its pause or cancel.
    if any_agent_cleaned {
        let _ = write_run(base, &run);
    }

    if !stale_complete && !stale_running && !any_agent_cleaned {
        return;
    }

    // Mark as complete if running and stale
    if stale_running {
        run.status = RunStatus::Complete;
        let _ = write_run(base, &run);
    }

    // Clean up agent directories — selectively remove only non-running ones
    let agents_dir = orchestrator_dir(base).join("agents");
    if agents_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&agents_dir)
    {
        for entry in entries.flatten() {
            let status_path = entry.path().join("status.json");
            if let Ok(content) = fs::read_to_string(&status_path) {
                let status: serde_json::Value =
                    crate::shared::helpers::parse_persisted_json(&content).unwrap_or_default();
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
/// Reuses the accurate `days_since_epoch` logic from mem::store::util for the
/// `YYYY-MM-DDThh:mm:ss` part, then honors optional fractional seconds and a
/// `Z` or `+hh:mm` / `-hh:mm` offset (what `date -Iseconds` writes). No suffix
/// means UTC.
fn epoch_secs_from_iso(iso: &str) -> u64 {
    let utc_as_written = crate::mem::store::parse_iso_to_secs(iso);
    if utc_as_written == 0 {
        return 0;
    }
    let rest = iso.get(19..).unwrap_or("");
    let rest = match rest.strip_prefix('.') {
        Some(frac) => frac.trim_start_matches(|c: char| c.is_ascii_digit()),
        None => rest,
    };
    let (west, offset) = match rest.split_at_checked(1) {
        Some(("+", o)) => (false, o),
        Some(("-", o)) => (true, o),
        _ => return utc_as_written,
    };
    let digits = offset.replacen(':', "", 1);
    let number = |range| digits.get(range).and_then(|d: &str| d.parse::<u64>().ok());
    let secs = match (digits.len(), number(0..2), number(2..4)) {
        (2, Some(h), _) => h * 3600,
        (4, Some(h), Some(m)) => h * 3600 + m * 60,
        _ => return utc_as_written,
    };
    if west {
        utc_as_written + secs
    } else {
        utc_as_written.saturating_sub(secs)
    }
}

/// Current time as approximate epoch seconds.
pub(crate) fn epoch_secs_now() -> u64 {
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
    fn cancelled_agent_counts_as_finished() {
        let run = make_run(
            "c",
            vec![("a", AgentStatus::Cancelled), ("b", AgentStatus::Done)],
            vec![],
        );
        assert!(is_run_complete(&run));
    }

    #[test]
    fn agent_status_serialization() {
        let statuses = vec![
            (AgentStatus::Pending, "\"pending\""),
            (AgentStatus::Running, "\"running\""),
            (AgentStatus::Done, "\"done\""),
            (AgentStatus::Blocked, "\"blocked\""),
            (AgentStatus::Failed, "\"failed\""),
            (AgentStatus::Cancelled, "\"cancelled\""),
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

    #[test]
    fn upsert_running_agent_after_complete_starts_new_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Done).unwrap();
        let mut run = read_run(base).unwrap();
        assert_eq!(run.status, RunStatus::Complete);
        assert!(run.agents[0].completed_at.is_some());
        // Same-second ids collide, so give the finished run a distinct id.
        run.id = "auto-old".to_string();
        write_run(base, &run).unwrap();

        // A new agent starts after completion: a new run, not a reopened one.
        upsert_agent_to_run(base, "b", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_ne!(run.id, "auto-old");
        assert!(run.id.starts_with("auto-"));
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(agent_ids(&run), ["b"]);

        // Within a running run a restart still clears completed_at.
        upsert_agent_to_run(base, "c", "t", "builder", AgentStatus::Running).unwrap();
        upsert_agent_to_run(base, "b", "t", "builder", AgentStatus::Done).unwrap();
        let id = read_run(base).unwrap().id;
        upsert_agent_to_run(base, "b", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_eq!(run.id, id, "a restart inside a running run does not rotate");
        assert_eq!(agent_ids(&run), ["b", "c"]);
        let b = run.agents.iter().find(|x| x.id == "b").unwrap();
        assert!(b.completed_at.is_none(), "restart clears completed_at");
    }

    #[test]
    fn upsert_never_overwrites_a_cancelled_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        // What `/intervene cancel a` writes.
        let mut run = read_run(base).unwrap();
        run.agents[0].status = AgentStatus::Cancelled;
        write_run(base, &run).unwrap();

        for late in [
            AgentStatus::Blocked,
            AgentStatus::Pending,
            AgentStatus::Done,
            AgentStatus::Failed,
        ] {
            upsert_agent_to_run(base, "a", "t", "builder", late.clone()).unwrap();
            let run = read_run(base).unwrap();
            assert_eq!(run.agents[0].status, AgentStatus::Cancelled, "{late:?}");
            assert!(run.agents[0].completed_at.is_none(), "{late:?}");
            assert_eq!(
                read_agent_status(base, "a").unwrap().status,
                AgentStatus::Cancelled,
                "{late:?}"
            );
        }
    }

    /// I4: `Running` is requested only when the gate allowed the spawn, i.e.
    /// the user resumed the agent, so it un-cancels.
    #[test]
    fn running_upsert_uncancels() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        // A second running agent keeps the run incomplete, so it stays live.
        upsert_agent_to_run(base, "hold", "t", "builder", AgentStatus::Running).unwrap();
        let mut run = read_run(base).unwrap();
        run.agents[0].status = AgentStatus::Cancelled;
        write_run(base, &run).unwrap();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(
            read_agent_status(base, "a").unwrap().status,
            AgentStatus::Cancelled
        );

        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        let after = read_run(base).unwrap();
        assert_eq!(after.id, run.id, "same live run");
        assert_eq!(after.agents[0].status, AgentStatus::Running);
        assert!(after.agents[0].completed_at.is_none());
        assert_eq!(
            read_agent_status(base, "a").unwrap().status,
            AgentStatus::Running
        );
    }

    #[test]
    fn restarted_agent_gets_fresh_started_at_and_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Done).unwrap();
        // Age the agent: started an hour ago, heartbeat an hour ago.
        let old = "2000-01-01T00:00:00Z".to_string();
        let mut run = read_run(base).unwrap();
        run.agents[0].started_at = Some(old.clone());
        write_run(base, &run).unwrap();
        let mut st = read_agent_status(base, "a").unwrap();
        st.last_heartbeat = old.clone();
        write_agent_status(base, "a", &st).unwrap();

        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        let started = run.agents[0].started_at.clone().unwrap();
        assert_ne!(started, old, "restart resets started_at");
        assert_ne!(read_agent_status(base, "a").unwrap().last_heartbeat, old);

        // A repeated Running update (heartbeat cycle) keeps the start time.
        write_run(base, &{
            let mut r = run.clone();
            r.agents[0].started_at = Some(old.clone());
            r
        })
        .unwrap();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Running).unwrap();
        assert_eq!(read_run(base).unwrap().agents[0].started_at, Some(old));
    }

    #[test]
    fn concurrent_upserts_keep_every_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let base = base.clone();
                std::thread::spawn(move || {
                    upsert_agent_to_run(
                        &base,
                        &format!("agent-{i}"),
                        "t",
                        "builder",
                        AgentStatus::Running,
                    )
                    .unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(read_run(&base).unwrap().agents.len(), 16);
    }

    // ── complete_and_unblock ──

    #[test]
    fn complete_and_unblock_done_unblocks_dependent() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let run = make_run(
            "r",
            vec![("A", AgentStatus::Running), ("B", AgentStatus::Pending)],
            vec![("B", vec!["A"])],
        );
        init_run(base, &run).unwrap();

        let unblocked = complete_and_unblock(base, "A", AgentStatus::Done).unwrap();
        assert_eq!(unblocked, vec!["B".to_string()]);
        let run = read_run(base).unwrap();
        let a = run.agents.iter().find(|x| x.id == "A").unwrap();
        assert_eq!(a.status, AgentStatus::Done);
        assert!(a.completed_at.is_some());
        let b = run.agents.iter().find(|x| x.id == "B").unwrap();
        assert_eq!(b.status, AgentStatus::Pending);
        assert_eq!(read_agent_status(base, "B").unwrap().phase, "pending");
    }

    #[test]
    fn complete_and_unblock_failed_still_unblocks() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let run = make_run(
            "r",
            vec![("A", AgentStatus::Running), ("B", AgentStatus::Blocked)],
            vec![("B", vec!["A"])],
        );
        init_run(base, &run).unwrap();

        let unblocked = complete_and_unblock(base, "A", AgentStatus::Failed).unwrap();
        assert_eq!(unblocked, vec!["B".to_string()]);
        let run = read_run(base).unwrap();
        let a = run.agents.iter().find(|x| x.id == "A").unwrap();
        assert_eq!(a.status, AgentStatus::Failed);
        assert!(a.completed_at.is_some());
        assert_eq!(agent_status_of(base, "B"), AgentStatus::Pending);
        assert_eq!(run.status, RunStatus::Running);

        // B finishes too: every agent is Done or Failed, so the run completes.
        complete_and_unblock(base, "B", AgentStatus::Done).unwrap();
        assert_eq!(read_run(base).unwrap().status, RunStatus::Complete);
    }

    #[test]
    fn complete_and_unblock_ignores_non_terminal_status() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        init_run(
            base,
            &make_run("r", vec![("A", AgentStatus::Running)], vec![]),
        )
        .unwrap();
        let unblocked = complete_and_unblock(base, "A", AgentStatus::Running).unwrap();
        assert!(unblocked.is_empty());
        assert_eq!(agent_status_of(base, "A"), AgentStatus::Running);
    }

    #[test]
    fn complete_and_unblock_without_run_creates_only_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        assert!(
            complete_and_unblock(base, "A", AgentStatus::Done)
                .unwrap()
                .is_empty()
        );
        assert!(read_run(base).is_none());
        let entries: Vec<_> = fs::read_dir(orchestrator_dir(base))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["run.json.lock".to_string()]);
    }

    #[test]
    fn complete_and_unblock_concurrent_with_upserts_loses_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        // A live run (fresh timestamps) that `hold` keeps incomplete: an idle or
        // completed run would rotate on the next `Running` upsert and drop `seed`.
        let mut run = make_run(
            "r",
            vec![
                ("seed", AgentStatus::Running),
                ("hold", AgentStatus::Running),
            ],
            vec![],
        );
        run.created_at = crate::shared::helpers::now_iso();
        run.updated_at = run.created_at.clone();
        init_run(&base, &run).unwrap();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let base = base.clone();
                std::thread::spawn(move || {
                    if i == 0 {
                        complete_and_unblock(&base, "seed", AgentStatus::Done).unwrap();
                    } else {
                        upsert_agent_to_run(
                            &base,
                            &format!("agent-{i}"),
                            "t",
                            "builder",
                            AgentStatus::Running,
                        )
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let run = read_run(&base).unwrap();
        assert_eq!(run.agents.len(), 17);
        assert_eq!(agent_status_of(&base, "seed"), AgentStatus::Done);
    }

    #[test]
    fn upsert_failed_writes_failed_phase_and_completed_at() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Failed).unwrap();
        let st = read_agent_status(base, "a").unwrap();
        assert_eq!(st.phase, "failed");
        assert_eq!(st.progress, 1.0);
        assert!(read_run(base).unwrap().agents[0].completed_at.is_some());

        // Existing-agent branch: Running then Failed.
        upsert_agent_to_run(base, "b", "t", "builder", AgentStatus::Running).unwrap();
        upsert_agent_to_run(base, "b", "t", "builder", AgentStatus::Failed).unwrap();
        let run = read_run(base).unwrap();
        let b = run.agents.iter().find(|x| x.id == "b").unwrap();
        assert!(b.completed_at.is_some());
        assert_eq!(read_agent_status(base, "b").unwrap().phase, "failed");
    }

    #[test]
    fn upsert_blocked_writes_blocked_phase() {
        let tmp = tempfile::tempdir().unwrap();
        upsert_agent_to_run(tmp.path(), "a", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(read_agent_status(tmp.path(), "a").unwrap().phase, "blocked");
    }

    #[cfg(unix)]
    #[test]
    fn lock_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        upsert_agent_to_run(tmp.path(), "a", "t", "builder", AgentStatus::Running).unwrap();
        let mode = fs::metadata(orchestrator_dir(tmp.path()).join("run.json.lock"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    // ── Agent staleness uses the newest liveness signal ──

    use crate::shared::helpers::iso_secs_ago as ago;

    /// Run `auto-x` with Running agents started `started_secs_ago` seconds ago.
    fn cleanup_fixture(base: &Path, agents: &[(&str, u64)]) {
        let mut run = make_run("auto-x", vec![], vec![]);
        run.agents = agents
            .iter()
            .map(|(id, started)| {
                let mut a = make_run("r", vec![(id, AgentStatus::Running)], vec![]).agents;
                let mut a = a.remove(0);
                a.started_at = Some(ago(*started));
                a
            })
            .collect();
        run.updated_at = crate::shared::helpers::now_iso();
        init_run(base, &run).unwrap();
    }

    fn heartbeat(base: &Path, id: &str, secs_ago: u64) {
        let status = AgentStatusFile {
            agent_id: id.to_string(),
            phase: "executing".to_string(),
            progress: 0.0,
            last_heartbeat: ago(secs_ago),
            status: AgentStatus::Running,
        };
        write_agent_status(base, id, &status).unwrap();
    }

    fn agent_status_of(base: &Path, id: &str) -> AgentStatus {
        read_run(base)
            .unwrap()
            .agents
            .into_iter()
            .find(|a| a.id == id)
            .unwrap()
            .status
    }

    #[test]
    fn e_cleanup_keeps_agent_with_fresh_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        cleanup_fixture(tmp.path(), &[("a1", 7200)]);
        heartbeat(tmp.path(), "a1", 300);
        auto_cleanup_stale_runs(tmp.path());
        assert_eq!(agent_status_of(tmp.path(), "a1"), AgentStatus::Running);
        assert!(agent_dir(tmp.path(), "a1").is_dir());
    }

    #[test]
    fn e_cleanup_marks_done_when_both_stale() {
        let tmp = tempfile::tempdir().unwrap();
        cleanup_fixture(tmp.path(), &[("a1", 7200)]);
        heartbeat(tmp.path(), "a1", 45 * 60);
        auto_cleanup_stale_runs(tmp.path());
        let run = read_run(tmp.path()).unwrap();
        assert_eq!(run.agents[0].status, AgentStatus::Done);
        assert!(run.agents[0].completed_at.is_some());
    }

    #[test]
    fn e_cleanup_without_status_uses_started_at() {
        let tmp = tempfile::tempdir().unwrap();
        cleanup_fixture(tmp.path(), &[("fresh", 600), ("old", 7200)]);
        auto_cleanup_stale_runs(tmp.path());
        assert_eq!(agent_status_of(tmp.path(), "fresh"), AgentStatus::Running);
        assert_eq!(agent_status_of(tmp.path(), "old"), AgentStatus::Done);
    }

    #[test]
    fn e_cleanup_reads_bom_status() {
        let tmp = tempfile::tempdir().unwrap();
        cleanup_fixture(tmp.path(), &[("a1", 7200)]);
        heartbeat(tmp.path(), "a1", 300);
        let path = agent_status_file(tmp.path(), "a1");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend(fs::read(&path).unwrap());
        fs::write(&path, bytes).unwrap();
        auto_cleanup_stale_runs(tmp.path());
        assert_eq!(agent_status_of(tmp.path(), "a1"), AgentStatus::Running);
    }

    // ── Spawn gate and run lifecycle ──

    use crate::shared::helpers::iso_from_secs;

    fn agent_ids(run: &OrchestrationRun) -> Vec<&str> {
        run.agents.iter().map(|a| a.id.as_str()).collect()
    }

    /// Set a file's mtime to `secs` since the epoch (std only).
    fn set_mtime(path: &Path, secs: u64) {
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    }

    /// Write run.json (id `auto-r1`) with `created_ago` / `updated_ago`
    /// seconds before `now`.
    fn live_run(
        base: &Path,
        now: u64,
        status: RunStatus,
        created_ago: u64,
        updated_ago: u64,
        agents: &[(&str, AgentStatus)],
    ) -> OrchestrationRun {
        let mut run = make_run("auto-r1", agents.to_vec(), vec![]);
        run.status = status;
        run.created_at = iso_from_secs(now - created_ago);
        run.updated_at = iso_from_secs(now - updated_ago);
        fs::create_dir_all(orchestrator_dir(base)).unwrap();
        write_run(base, &run).unwrap();
        run
    }

    /// Write control.json (generation 1) and pin its mtime to `mtime`.
    fn put_control(
        base: &Path,
        action: ControlAction,
        target: Option<&str>,
        message: Option<&str>,
        mtime: u64,
    ) {
        let directive = ControlDirective {
            action,
            target: target.map(String::from),
            message: message.map(String::from),
            generation: 1,
        };
        write_control(base, &directive).unwrap();
        set_mtime(&control_file(base), mtime);
    }

    fn kind_of(base: &Path, id: &str, now: u64) -> Option<BlockKind> {
        spawn_block(base, id, now).map(|b| b.kind)
    }

    // Done-when 1: a stale cancel never blocks a later run.

    #[test]
    fn stale_abort_never_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Aborted,
            8000,
            7201,
            &[("a", AgentStatus::Running)],
        );
        put_control(base, ControlAction::Cancel, Some("all"), None, now);
        for id in ["a", "zzz", "other"] {
            assert_eq!(spawn_block(base, id, now), None, "{id}");
        }
        upsert_agent_to_run(base, "new", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_ne!(run.id, "auto-r1");
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(agent_ids(&run), ["new"]);
    }

    #[test]
    fn cancel_from_previous_run_is_unbound() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let run = live_run(
            base,
            now,
            RunStatus::Running,
            0,
            0,
            &[("a", AgentStatus::Running)],
        );
        for action in [ControlAction::Cancel, ControlAction::Pause] {
            put_control(base, action, Some("all"), None, now - 60);
            assert!(!control_is_bound(base, &run));
            assert_eq!(spawn_block(base, "a", now), None);
        }
        // Written in the run's own second: bound.
        put_control(base, ControlAction::Pause, Some("all"), None, now);
        assert!(control_is_bound(base, &run));
        assert_eq!(kind_of(base, "a", now), Some(BlockKind::Paused));
    }

    #[test]
    fn cancelled_id_free_in_new_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Complete,
            8000,
            7201,
            &[("x", AgentStatus::Cancelled)],
        );
        assert_eq!(spawn_block(base, "x", now), None);
        upsert_agent_to_run(base, "x", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_ne!(run.id, "auto-r1");
        assert_eq!(agent_ids(&run), ["x"]);
        assert_eq!(run.agents[0].status, AgentStatus::Running);
    }

    #[test]
    fn ttl_boundary() {
        let now = epoch_secs_now();
        for kind in [
            BlockKind::RunCancelled,
            BlockKind::AgentCancelled,
            BlockKind::Paused,
            BlockKind::DirectiveCancelled,
        ] {
            for (age, blocked) in [(7200u64, true), (7201, false)] {
                let tmp = tempfile::tempdir().unwrap();
                let base = tmp.path();
                let (created, updated) = (age + 100, age);
                match kind {
                    BlockKind::RunCancelled => {
                        live_run(base, now, RunStatus::Aborted, created, updated, &[]);
                    }
                    BlockKind::AgentCancelled => {
                        live_run(
                            base,
                            now,
                            RunStatus::Running,
                            created,
                            updated,
                            &[("x", AgentStatus::Cancelled)],
                        );
                    }
                    BlockKind::Paused | BlockKind::DirectiveCancelled => {
                        live_run(
                            base,
                            now,
                            RunStatus::Running,
                            created,
                            updated,
                            &[("x", AgentStatus::Running)],
                        );
                        let action = if kind == BlockKind::Paused {
                            ControlAction::Pause
                        } else {
                            ControlAction::Cancel
                        };
                        put_control(base, action, Some("all"), None, now);
                    }
                }
                assert_eq!(
                    kind_of(base, "x", now),
                    blocked.then(|| kind.clone()),
                    "{kind:?} at age {age}"
                );
            }
        }
    }

    #[test]
    fn future_or_malformed_timestamps_fail_open() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let mut run = live_run(
            base,
            now,
            RunStatus::Aborted,
            100,
            0,
            &[("x", AgentStatus::Running)],
        );
        // Malformed: not live whatever the files say.
        run.updated_at = "garbage".to_string();
        write_run(base, &run).unwrap();
        assert!(!is_live(base, &run, now));
        assert_eq!(spawn_block(base, "x", now), None);

        // Future beyond the skew allowance: an aborted run without a bound
        // control.json is not live, and for other runs the run.json mtime
        // stands in, so an old mtime means expired (a future time cannot pin a
        // block forever).
        let just_too_far = iso_from_secs(now + 301);
        for future in ["2099-01-01T00:00:00Z", just_too_far.as_str()] {
            run.updated_at = future.to_string();
            write_run(base, &run).unwrap();
            set_mtime(&run_file(base), now - 3 * 3600);
            assert!(!is_live(base, &run, now), "{future}");
            assert_eq!(spawn_block(base, "x", now), None, "{future}");
        }
        // Within the clock skew allowance the timestamp is trusted.
        run.updated_at = iso_from_secs(now + 300);
        assert!(is_live(base, &run, now));

        // Malformed created_at: control.json is unbound.
        let mut run = live_run(
            base,
            now,
            RunStatus::Running,
            100,
            0,
            &[("x", AgentStatus::Running)],
        );
        put_control(base, ControlAction::Pause, Some("all"), None, now);
        run.created_at = "garbage".to_string();
        write_run(base, &run).unwrap();
        assert!(!control_is_bound(base, &run));
        assert_eq!(spawn_block(base, "x", now), None);
    }

    // Done-when 2: the documented flows block while meant to.

    fn pause_fixture(base: &Path, now: u64, target: Option<&str>) {
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Running), ("y", AgentStatus::Running)],
        );
        put_control(base, ControlAction::Pause, target, None, now);
    }

    #[test]
    fn pause_agent_blocks_only_that_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let now = epoch_secs_now();
        pause_fixture(tmp.path(), now, Some("x"));
        assert_eq!(kind_of(tmp.path(), "x", now), Some(BlockKind::Paused));
        assert_eq!(kind_of(tmp.path(), "y", now), None);
    }

    #[test]
    fn pause_all_blocks_every_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let now = epoch_secs_now();
        pause_fixture(tmp.path(), now, Some("all"));
        for id in ["x", "y", "unknown"] {
            assert_eq!(
                kind_of(tmp.path(), id, now),
                Some(BlockKind::Paused),
                "{id}"
            );
        }
    }

    #[test]
    fn pause_with_no_target_blocks_every_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let now = epoch_secs_now();
        pause_fixture(tmp.path(), now, None);
        for id in ["x", "y", "unknown"] {
            assert_eq!(
                kind_of(tmp.path(), id, now),
                Some(BlockKind::Paused),
                "{id}"
            );
        }
    }

    #[test]
    fn cancel_agent_blocks_respawn_even_after_other_directive() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Cancelled), ("y", AgentStatus::Running)],
        );
        put_control(base, ControlAction::Cancel, Some("x"), None, now);
        assert_eq!(kind_of(base, "x", now), Some(BlockKind::AgentCancelled));
        // The single control slot now holds an unrelated directive.
        put_control(base, ControlAction::Pause, Some("y"), None, now);
        assert_eq!(kind_of(base, "x", now), Some(BlockKind::AgentCancelled));
        assert_eq!(kind_of(base, "y", now), Some(BlockKind::Paused));
    }

    #[test]
    fn cancel_directive_without_run_mark_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Running)],
        );
        put_control(
            base,
            ControlAction::Cancel,
            Some("x"),
            Some("  stop  "),
            now,
        );
        let block = spawn_block(base, "x", now).unwrap();
        assert_eq!(block.kind, BlockKind::DirectiveCancelled);
        assert_eq!(block.generation, Some(1));
        assert_eq!(block.target.as_deref(), Some("x"));
        assert_eq!(block.message.as_deref(), Some("stop"));
    }

    #[test]
    fn cancel_all_blocks_every_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Aborted,
            500,
            100,
            &[("a", AgentStatus::Running)],
        );
        for id in ["a", "new"] {
            assert_eq!(
                kind_of(base, id, now),
                Some(BlockKind::RunCancelled),
                "{id}"
            );
        }
    }

    /// I1: three code paths used to flip an aborted run to complete or bump
    /// its `updated_at`.
    #[test]
    fn aborted_run_is_sticky() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        // `a` has neither start nor heartbeat, so cleanup would mark it done.
        live_run(
            base,
            now,
            RunStatus::Aborted,
            500,
            100,
            &[("a", AgentStatus::Running)],
        );
        let before = read_run(base).unwrap();
        let bytes = fs::read(run_file(base)).unwrap();
        let sticky = || {
            let run = read_run(base).unwrap();
            assert_eq!(run.status, RunStatus::Aborted);
            assert_eq!(run.updated_at, before.updated_at);
        };

        auto_cleanup_stale_runs(base);
        assert_eq!(
            fs::read(run_file(base)).unwrap(),
            bytes,
            "cleanup leaves run.json alone"
        );

        upsert_agent_to_run(base, "a", "t", "builder", AgentStatus::Done).unwrap();
        sticky();
        assert_eq!(
            complete_and_unblock(base, "a", AgentStatus::Done).unwrap(),
            Vec::<String>::new()
        );
        sticky();
        upsert_agent_to_run(base, "fresh", "t", "builder", AgentStatus::Blocked).unwrap();
        sticky();
        let run = read_run(base).unwrap();
        let fresh = run.agents.iter().find(|a| a.id == "fresh").unwrap();
        assert_eq!(
            fresh.status,
            AgentStatus::Blocked,
            "the denied spawn is recorded"
        );
    }

    #[test]
    fn sole_cancelled_agent_stays_blocked_after_run_completes() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        upsert_agent_to_run(base, "x", "t", "builder", AgentStatus::Running).unwrap();
        let mut run = read_run(base).unwrap();
        run.agents[0].status = AgentStatus::Cancelled;
        write_run(base, &run).unwrap();
        upsert_agent_to_run(base, "x", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(read_run(base).unwrap().status, RunStatus::Complete);
        assert_eq!(
            kind_of(base, "x", epoch_secs_now()),
            Some(BlockKind::AgentCancelled)
        );
    }

    #[test]
    fn redirect_and_resume_never_block() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Running)],
        );
        for action in [ControlAction::Redirect, ControlAction::Resume] {
            for target in [Some("all"), Some("x"), None] {
                put_control(base, action.clone(), target, Some("go"), now);
                assert_eq!(spawn_block(base, "x", now), None, "{action:?} {target:?}");
            }
        }
    }

    // Done-when 3: a blocked user always has a documented way out.

    #[test]
    fn resume_all_lifts_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Aborted,
            500,
            100,
            &[("a", AgentStatus::Running)],
        );
        // A resume for one agent does not lift a cancelled run.
        put_control(base, ControlAction::Resume, Some("a"), None, now);
        assert_eq!(kind_of(base, "a", now), Some(BlockKind::RunCancelled));
        for target in [Some("all"), None] {
            put_control(base, ControlAction::Resume, target, None, now);
            assert_eq!(spawn_block(base, "a", now), None, "{target:?}");
            assert_eq!(spawn_block(base, "new", now), None, "{target:?}");
        }
        upsert_agent_to_run(base, "new", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_ne!(run.id, "auto-r1");
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(agent_ids(&run), ["new"]);
    }

    #[test]
    fn resume_agent_lifts_agent_cancel() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Cancelled), ("y", AgentStatus::Running)],
        );
        // A denied respawn is pinned to Cancelled, including its status file.
        upsert_agent_to_run(base, "x", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(
            read_agent_status(base, "x").unwrap().status,
            AgentStatus::Cancelled
        );
        assert_eq!(
            kind_of(base, "x", epoch_secs_now()),
            Some(BlockKind::AgentCancelled)
        );

        put_control(
            base,
            ControlAction::Resume,
            Some("x"),
            None,
            epoch_secs_now(),
        );
        assert_eq!(spawn_block(base, "x", epoch_secs_now()), None);
        upsert_agent_to_run(base, "x", "t", "builder", AgentStatus::Running).unwrap();
        let run = read_run(base).unwrap();
        assert_eq!(run.id, "auto-r1", "same live run");
        let x = run.agents.iter().find(|a| a.id == "x").unwrap();
        assert_eq!(x.status, AgentStatus::Running);
        assert!(x.completed_at.is_none());
        assert_eq!(
            read_agent_status(base, "x").unwrap().status,
            AgentStatus::Running
        );
    }

    #[test]
    fn resume_all_lifts_agent_cancel_and_pause() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        live_run(
            base,
            now,
            RunStatus::Running,
            100,
            10,
            &[("x", AgentStatus::Cancelled), ("y", AgentStatus::Running)],
        );
        put_control(base, ControlAction::Pause, Some("all"), None, now);
        assert_eq!(kind_of(base, "x", now), Some(BlockKind::AgentCancelled));
        assert_eq!(kind_of(base, "y", now), Some(BlockKind::Paused));
        put_control(base, ControlAction::Resume, Some("all"), None, now);
        assert_eq!(spawn_block(base, "x", now), None);
        assert_eq!(spawn_block(base, "y", now), None);
    }

    /// Exhaustive over the finite domain (no property-test crate).
    #[test]
    fn spawn_gate_properties() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let statuses = [
            RunStatus::Running,
            RunStatus::Paused,
            RunStatus::Complete,
            RunStatus::Aborted,
        ];
        let x_states = [
            None,
            Some(AgentStatus::Running),
            Some(AgentStatus::Cancelled),
        ];
        let actions = [
            None,
            Some(ControlAction::Pause),
            Some(ControlAction::Cancel),
            Some(ControlAction::Redirect),
            Some(ControlAction::Resume),
        ];
        let targets = [None, Some("all"), Some("x"), Some("y")];
        for status in &statuses {
            for x_state in &x_states {
                for age in [0u64, 7200, 7201] {
                    let agents = match x_state {
                        Some(s) => vec![("x", s.clone())],
                        None => vec![("other", AgentStatus::Running)],
                    };
                    let created_ago = age + 100;
                    live_run(base, now, status.clone(), created_ago, age, &agents);
                    let _ = fs::remove_file(control_file(base));
                    let baseline = spawn_block(base, "x", now);
                    if age == 7201 {
                        assert_eq!(baseline, None);
                    }
                    for action in actions.iter().flatten() {
                        for target in targets {
                            for bound in [true, false] {
                                let mtime = if bound { now } else { now - created_ago - 60 };
                                put_control(base, action.clone(), target, None, mtime);
                                let got = spawn_block(base, "x", now);
                                let case = format!(
                                    "{status:?} {x_state:?} age {age} {action:?} {target:?} bound {bound}"
                                );
                                if age == 7201 {
                                    assert_eq!(got, None, "P1 {case}");
                                }
                                if !bound {
                                    assert_eq!(got, baseline, "P2 {case}");
                                }
                                if bound
                                    && *action == ControlAction::Resume
                                    && target == Some("all")
                                {
                                    assert_eq!(got, None, "P3 {case}");
                                }
                                if let Some(b) = &got {
                                    assert!(
                                        b.reason_lines("x").iter().any(|l| {
                                            l.contains("/intervene resume")
                                                && l.contains(&b.expires_at)
                                        }),
                                        "P4 {case}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn reason_lines_golden() {
        let block = |kind, target: Option<&str>, message: Option<&str>| SpawnBlock {
            kind,
            run_id: "auto-1".to_string(),
            generation: Some(7),
            target: target.map(String::from),
            message: message.map(String::from),
            expires_at: "2026-01-01T02:00:00Z".to_string(),
        };
        let lift = |who: &str| {
            format!(
                "To lift: the user runs /intervene resume {who}. Otherwise the block ends by itself after 2 hours without run activity (earliest 2026-01-01T02:00:00Z)."
            )
        };
        let tail = "Do not retry this spawn and do not run /intervene yourself; tell the user.";

        assert_eq!(
            block(BlockKind::RunCancelled, None, None).reason_lines("x"),
            [
                "BLOCKED: agent x not started: orchestration run auto-1 was cancelled by the user (/intervene cancel all).".to_string(),
                lift("all"),
                tail.to_string(),
            ]
        );
        assert_eq!(
            block(BlockKind::AgentCancelled, None, None).reason_lines("x"),
            [
                "BLOCKED: agent x not started: it was cancelled by the user in run auto-1 (/intervene cancel x).".to_string(),
                lift("x"),
                tail.to_string(),
            ]
        );
        assert_eq!(
            block(BlockKind::Paused, Some("x"), Some("hold on")).reason_lines("x"),
            [
                "BLOCKED: agent x not started: paused by user directive (gen 7, target x)."
                    .to_string(),
                "Reason: hold on".to_string(),
                lift("x"),
                tail.to_string(),
            ]
        );
        assert_eq!(
            block(BlockKind::Paused, None, None).reason_lines("x"),
            [
                "BLOCKED: agent x not started: paused by user directive (gen 7, target all)."
                    .to_string(),
                lift("all"),
                tail.to_string(),
            ]
        );
        assert_eq!(
            block(BlockKind::DirectiveCancelled, Some("y"), None).reason_lines("x"),
            [
                "BLOCKED: agent x not started: cancelled by user directive (gen 7, target y)."
                    .to_string(),
                lift("all"),
                tail.to_string(),
            ]
        );
        // Truncation counts chars, so a multibyte message cuts on a boundary.
        let long = "\u{ac00}".repeat(250);
        let lines =
            block(BlockKind::DirectiveCancelled, Some("all"), Some(&long)).reason_lines("x");
        assert_eq!(lines[1], format!("Reason: {}", "\u{ac00}".repeat(200)));
    }

    // Upsert regressions.

    #[test]
    fn concurrent_starts_rotate_once() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        upsert_agent_to_run(&base, "a", "t", "builder", AgentStatus::Running).unwrap();
        upsert_agent_to_run(&base, "a", "t", "builder", AgentStatus::Done).unwrap();
        let mut run = read_run(&base).unwrap();
        assert_eq!(run.status, RunStatus::Complete);
        run.id = "auto-old".to_string();
        write_run(&base, &run).unwrap();

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let base = base.clone();
                std::thread::spawn(move || {
                    upsert_agent_to_run(
                        &base,
                        &format!("agent-{i}"),
                        "t",
                        "builder",
                        AgentStatus::Running,
                    )
                    .unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let run = read_run(&base).unwrap();
        assert_ne!(run.id, "auto-old");
        let mut ids = agent_ids(&run);
        ids.sort_unstable();
        let expected: Vec<String> = (0..8).map(|i| format!("agent-{i}")).collect();
        assert_eq!(ids, expected);
    }

    // Review round 2.

    /// Run `auto-r1` created 4h and updated 3h ago (expired) with a bound
    /// `pause all` written 3h + 1 min ago.
    fn expired_pause(base: &Path, now: u64, agents: &[(&str, AgentStatus)]) {
        live_run(base, now, RunStatus::Running, 4 * 3600, 3 * 3600, agents);
        put_control(
            base,
            ControlAction::Pause,
            Some("all"),
            None,
            now - 3 * 3600 - 60,
        );
    }

    /// Cleanup is not run activity: it must not make an expired run live again.
    #[test]
    fn cleanup_does_not_revive_expired_pause_stale_running_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        expired_pause(
            base,
            now,
            &[("a", AgentStatus::Done), ("b", AgentStatus::Blocked)],
        );
        let before = read_run(base).unwrap();
        assert_eq!(spawn_block(base, "new", now), None);
        auto_cleanup_stale_runs(base);
        let after = read_run(base).unwrap();
        assert_eq!(after.status, RunStatus::Complete, "stale run is closed");
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(spawn_block(base, "new", epoch_secs_now()), None);
    }

    #[test]
    fn cleanup_does_not_revive_expired_pause_stale_agent_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        // No start and no heartbeat: cleanup marks `a` done.
        expired_pause(base, now, &[("a", AgentStatus::Running)]);
        let before = read_run(base).unwrap();
        auto_cleanup_stale_runs(base);
        let after = read_run(base).unwrap();
        assert_eq!(after.agents[0].status, AgentStatus::Done, "agent cleaned");
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(spawn_block(base, "new", epoch_secs_now()), None);
    }

    #[test]
    fn epoch_parsing_honors_offsets() {
        let utc = epoch_secs_from_iso("2026-11-26T10:00:00Z");
        assert_ne!(utc, 0);
        assert_eq!(
            epoch_secs_from_iso("2026-11-26T10:00:00"),
            utc,
            "no suffix is UTC"
        );
        assert_eq!(epoch_secs_from_iso("2026-11-26T19:00:00+09:00"), utc);
        assert_eq!(epoch_secs_from_iso("2026-11-26T19:00:00+0900"), utc);
        assert_eq!(epoch_secs_from_iso("2026-11-26T19:00:00+09"), utc);
        assert_eq!(epoch_secs_from_iso("2026-11-26T05:00:00-05:00"), utc);
        assert_eq!(epoch_secs_from_iso("2026-11-26T10:00:00.250Z"), utc);
        assert_eq!(epoch_secs_from_iso("2026-11-26T19:00:00.5+09:00"), utc);
        // An unreadable suffix keeps the old UTC reading.
        assert_eq!(epoch_secs_from_iso("2026-11-26T10:00:00+zz"), utc);
        assert_eq!(epoch_secs_from_iso("garbage"), 0);
    }

    /// The reviewer's scenario: the skill text lets the model write local time
    /// with an offset (`date -Iseconds`); that must not look like the future.
    #[test]
    fn offset_timestamp_blocks_cancel_all() {
        let now = epoch_secs_now();
        for (offset, shift) in [("+09:00", 9 * 3600i64), ("-05:00", -5 * 3600)] {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path();
            let mut run = live_run(
                base,
                now,
                RunStatus::Aborted,
                600,
                0,
                &[("a", AgentStatus::Running)],
            );
            let local = iso_from_secs((now as i64 + shift) as u64);
            run.updated_at = format!("{}{offset}", &local[..19]);
            write_run(base, &run).unwrap();
            put_control(base, ControlAction::Cancel, Some("all"), None, now);
            assert_eq!(
                kind_of(base, "new", now),
                Some(BlockKind::RunCancelled),
                "cancel all ignored: {}",
                run.updated_at
            );
            // The same abort written 2h + 1s ago has expired.
            let old = iso_from_secs((now as i64 + shift) as u64 - 7201);
            run.updated_at = format!("{}{offset}", &old[..19]);
            write_run(base, &run).unwrap();
            put_control(base, ControlAction::Cancel, Some("all"), None, now - 7201);
            assert_eq!(kind_of(base, "new", now), None, "{offset} expired");
        }
    }

    #[test]
    fn future_updated_at_on_aborted_run_blocks_until_mtime_plus_2h() {
        let now = epoch_secs_now();
        for (mtime_ago, blocked) in [(7000u64, true), (7300, false)] {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path();
            let mut run = live_run(
                base,
                now,
                RunStatus::Aborted,
                20000,
                0,
                &[("a", AgentStatus::Running)],
            );
            run.updated_at = "2099-01-01T00:00:00Z".to_string();
            write_run(base, &run).unwrap();
            set_mtime(&run_file(base), now - mtime_ago);
            put_control(base, ControlAction::Cancel, Some("all"), None, now);
            let block = spawn_block(base, "new", now);
            assert_eq!(block.is_some(), blocked, "mtime {mtime_ago}s ago");
            if let Some(b) = block {
                assert_eq!(
                    b.expires_at,
                    iso_from_secs(now - mtime_ago + RUN_IDLE_SECS),
                    "expiry anchored at the run.json mtime"
                );
            }
        }
    }

    /// Denied spawns rewrite run.json, so a bound control.json (written when
    /// the abort was) is the earlier anchor and caps the abort at 2h.
    #[test]
    fn abort_expiry_is_anchored_at_the_earlier_of_control_and_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let mut run = live_run(base, now, RunStatus::Aborted, 20000, 0, &[]);
        run.updated_at = "2099-01-01T00:00:00Z".to_string();
        write_run(base, &run).unwrap(); // run.json mtime: now (a denied retry)
        put_control(base, ControlAction::Cancel, Some("all"), None, now - 7201);
        assert_eq!(kind_of(base, "new", now), None);
        put_control(base, ControlAction::Cancel, Some("all"), None, now - 7000);
        assert_eq!(kind_of(base, "new", now), Some(BlockKind::RunCancelled));
    }

    /// The skill's `pause` writes control.json and sets run.json `updated_at`
    /// to now, so an agent that has run for 2.5h does not make it a no-op.
    #[test]
    fn pause_after_a_long_running_agent_blocks_once_the_skill_bumps_updated_at() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let mut run = live_run(
            base,
            now,
            RunStatus::Running,
            9000,
            9000,
            &[("a", AgentStatus::Running)],
        );
        run.agents[0].started_at = Some(iso_from_secs(now - 9000));
        write_run(base, &run).unwrap();
        // control.json alone: the run has been idle for 2.5h.
        put_control(base, ControlAction::Pause, Some("all"), None, now);
        assert_eq!(spawn_block(base, "new", now), None);
        // The skill also writes updated_at (UTC, with Z).
        run.updated_at = iso_from_secs(now);
        write_run(base, &run).unwrap();
        assert_eq!(kind_of(base, "new", now), Some(BlockKind::Paused));
        // A spawn now joins the live run instead of rotating it.
        upsert_agent_to_run(base, "new", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(agent_ids(&read_run(base).unwrap()), ["a", "new"]);
    }

    /// Without a bound control.json nothing caps a future `updated_at`, and
    /// denied retries rewrite run.json, so it must not fall back to the mtime.
    #[test]
    fn aborted_future_updated_at_without_bound_control_is_not_live() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let now = epoch_secs_now();
        let mut run = live_run(base, now, RunStatus::Aborted, 20000, 0, &[]);
        run.updated_at = iso_from_secs(now + 9 * 3600)[..19].to_string();
        write_run(base, &run).unwrap();
        set_mtime(&run_file(base), now - 7000);
        assert!(!control_is_bound(base, &run), "no control.json");
        assert_eq!(spawn_block(base, "new", now), None);
        // A control.json from before this run is unbound too.
        put_control(base, ControlAction::Cancel, Some("all"), None, now - 30000);
        assert!(!control_is_bound(base, &run));
        assert_eq!(spawn_block(base, "new", now), None);
        // A denied retry cannot pin it either.
        upsert_agent_to_run(base, "new", "t", "builder", AgentStatus::Blocked).unwrap();
        assert_eq!(spawn_block(base, "new", epoch_secs_now()), None);
    }
}
