//! orchestrator.rs — Orchestrator state SQLite I/O
//!
//! Replaces file-based orchestrator/ directory with SQLite tables.
//! flock(2) advisory locking is replaced by SQLite WAL transactions.
//!
//! All public functions are unused until the orchestrate hooks integration lands.
//! TODO: remove `#![allow(dead_code)]` once hooks wire these up.
#![allow(dead_code)]

use rusqlite::Connection;
use std::io;

use super::{ImmediateTx, query_row_optional, store_err};

// ── Types ────────────────────────────────────────────

/// Type-safe status for an orchestration run.
/// `OrchRun.status` remains `String` for JSON/DB round-trip; use `RunStatus` for function params.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Complete,
    Aborted,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Complete => "complete",
            RunStatus::Aborted => "aborted",
        }
    }
}

/// Type-safe control action for the single-row `orch_control` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Start,
    Stop,
    Pause,
    Resume,
}

impl ControlAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ControlAction::Start => "start",
            ControlAction::Stop => "stop",
            ControlAction::Pause => "pause",
            ControlAction::Resume => "resume",
        }
    }
}

/// Subset of orchestrate::state::OrchestrationRun fields stored in DB.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrchRun {
    pub id: String,
    pub status: String,
    pub agents_json: String,
    pub dep_graph_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrchAgent {
    pub id: String,
    pub run_id: String,
    pub role: String,
    pub task: String,
    pub satisfies_json: String,
    pub status: String,
    pub phase: String,
    pub progress: f64,
    pub last_heartbeat: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

// ── Run operations ───────────────────────────────────

/// Initialize a new orchestration run.
pub fn init_run_conn(conn: &Connection, project: &str, run: &OrchRun) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orch_runs
         (id, project, status, agents_json, dep_graph_json, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            run.id,
            project,
            run.status,
            run.agents_json,
            run.dep_graph_json,
            run.created_at,
            run.updated_at,
        ],
    ))?;
    Ok(())
}

/// Read the current orchestration run.
pub fn read_run_conn(conn: &Connection) -> io::Result<Option<OrchRun>> {
    query_row_optional(conn.query_row(
        "SELECT id, status, agents_json, dep_graph_json, created_at, updated_at
         FROM orch_runs WHERE status = ?1 ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![RunStatus::Running.as_str()],
        |row| {
            Ok(OrchRun {
                id: row.get(0)?,
                status: row.get(1)?,
                agents_json: row.get(2)?,
                dep_graph_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    ))
}

/// Update run status.
pub fn update_run_status_conn(
    conn: &Connection,
    run_id: &str,
    status: RunStatus,
) -> io::Result<()> {
    store_err(conn.execute(
        "UPDATE orch_runs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status.as_str(), crate::shared::helpers::now_iso(), run_id],
    ))?;
    Ok(())
}

// ── Agent operations ─────────────────────────────────

/// Insert or update an agent.
pub fn upsert_agent_conn(conn: &Connection, agent: &OrchAgent) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orch_agents
         (id, run_id, role, task, satisfies_json, status, phase, progress,
          last_heartbeat, started_at, completed_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![
            agent.id,
            agent.run_id,
            agent.role,
            agent.task,
            agent.satisfies_json,
            agent.status,
            agent.phase,
            agent.progress,
            agent.last_heartbeat,
            agent.started_at,
            agent.completed_at,
        ],
    ))?;
    Ok(())
}

/// Read agent status by ID.
pub fn read_agent_conn(conn: &Connection, agent_id: &str) -> io::Result<Option<OrchAgent>> {
    query_row_optional(conn.query_row(
        "SELECT id, run_id, role, task, satisfies_json, status, phase, progress,
                last_heartbeat, started_at, completed_at
         FROM orch_agents WHERE id = ?1",
        rusqlite::params![agent_id],
        |row| {
            Ok(OrchAgent {
                id: row.get(0)?,
                run_id: row.get(1)?,
                role: row.get(2)?,
                task: row.get(3)?,
                satisfies_json: row.get(4)?,
                status: row.get(5)?,
                phase: row.get(6)?,
                progress: row.get(7)?,
                last_heartbeat: row.get(8)?,
                started_at: row.get(9)?,
                completed_at: row.get(10)?,
            })
        },
    ))
}

/// Dismiss an agent: remove from agents table and update run JSON.
///
/// Uses `ImmediateTx` guard for safe `BEGIN IMMEDIATE` with auto-rollback on error.
pub fn dismiss_agent_conn(conn: &Connection, agent_id: &str) -> io::Result<bool> {
    let tx = ImmediateTx::begin(conn)?;

    // Check agent exists inside the write lock
    let exists: bool = store_err(conn.query_row(
        "SELECT COUNT(*) FROM orch_agents WHERE id = ?1",
        rusqlite::params![agent_id],
        |row| row.get::<_, i64>(0),
    ))? > 0;

    if !exists {
        // tx drops → auto-ROLLBACK
        return Ok(false);
    }

    // Delete agent and related records within the held write lock
    store_err(conn.execute(
        "DELETE FROM orch_agents WHERE id = ?1",
        rusqlite::params![agent_id],
    ))?;
    store_err(conn.execute(
        "DELETE FROM orch_agent_events WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    ))?;
    store_err(conn.execute(
        "DELETE FROM orch_agent_inbox WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    ))?;

    tx.commit()?;
    Ok(true)
}

// ── Agent events ─────────────────────────────────────

/// Append an agent event.
pub fn append_event_conn(
    conn: &Connection,
    agent_id: &str,
    timestamp: &str,
    event_type: &str,
    data_json: &str,
) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT INTO orch_agent_events (agent_id, timestamp, event_type, data_json)
         VALUES (?1,?2,?3,?4)",
        rusqlite::params![agent_id, timestamp, event_type, data_json],
    ))?;
    Ok(())
}

// ── Agent inbox ──────────────────────────────────────

/// Post a message to an agent's inbox.
pub fn post_inbox_conn(
    conn: &Connection,
    agent_id: &str,
    from_agent: &str,
    timestamp: &str,
    message: &str,
) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT INTO orch_agent_inbox (agent_id, from_agent, timestamp, message)
         VALUES (?1,?2,?3,?4)",
        rusqlite::params![agent_id, from_agent, timestamp, message],
    ))?;
    Ok(())
}

// ── Control ──────────────────────────────────────────

/// Write a control directive (single-row table).
pub fn write_control_conn(
    conn: &Connection,
    action: ControlAction,
    target: Option<&str>,
    message: Option<&str>,
    generation: i64,
) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orch_control (id, action, target, message, generation)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![action.as_str(), target, message, generation],
    ))?;
    Ok(())
}

// ── Cleanup ──────────────────────────────────────────

/// Clean up completed/aborted runs and orphaned agents.
///
/// `cutoff_ts` is an ISO-8601 timestamp; only runs updated before this time
/// are removed. Pass an empty string to remove all completed/aborted runs.
///
/// All deletions are wrapped in a single IMMEDIATE transaction so a partial failure
/// does not leave orphaned agent events or inbox messages without their agents.
pub fn cleanup_stale_conn(conn: &Connection, cutoff_ts: &str) -> io::Result<u64> {
    let tx = ImmediateTx::begin(conn)?;

    let mut count = 0u64;

    // Deletion order respects FK constraints (foreign_keys=ON):
    //   child rows (events, inbox, agents) must be removed before parent (runs).
    //
    // Step 1: materialise stale run IDs into a temp table so the subquery
    //         is evaluated once and parameter binding is used throughout
    //         (no string interpolation / SQL injection surface).
    store_err(conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS _stale_run_ids (id TEXT PRIMARY KEY);
             DELETE FROM _stale_run_ids;",
    ))?;

    if cutoff_ts.is_empty() {
        store_err(conn.execute(
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE status = ?1 OR status = ?2",
            rusqlite::params![RunStatus::Complete.as_str(), RunStatus::Aborted.as_str()],
        ))?;
    } else {
        store_err(conn.execute(
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE (status = ?1 OR status = ?2) AND updated_at < ?3",
            rusqlite::params![RunStatus::Complete.as_str(), RunStatus::Aborted.as_str(), cutoff_ts],
        ))?;
    }

    // Step 2: delete child rows for agents belonging to stale runs
    store_err(conn.execute(
        "DELETE FROM orch_agent_events WHERE agent_id IN (
                 SELECT id FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)
             )",
        [],
    ))?;
    store_err(conn.execute(
        "DELETE FROM orch_agent_inbox WHERE agent_id IN (
                 SELECT id FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)
             )",
        [],
    ))?;

    // Step 3: delete agents for stale runs
    let deleted_agents = store_err(conn.execute(
        "DELETE FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)",
        [],
    ))?;
    count += deleted_agents as u64;

    // Step 4: delete the runs themselves
    let deleted_runs = store_err(conn.execute(
        "DELETE FROM orch_runs WHERE id IN (SELECT id FROM _stale_run_ids)",
        [],
    ))?;
    count += deleted_runs as u64;

    // Cleanup temp table
    store_err(conn.execute_batch("DELETE FROM _stale_run_ids"))?;

    tx.commit()?;
    Ok(count)
}

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

#[allow(dead_code)]
pub async fn init_run_pool(pool: &SqlitePool, project: &str, run: &OrchRun) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_runs (id, project, status, agents_json, dep_graph_json, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)"
    )
    .bind(&run.id)
    .bind(project)
    .bind(&run.status)
    .bind(&run.agents_json)
    .bind(&run.dep_graph_json)
    .bind(&run.created_at)
    .bind(&run.updated_at)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn read_run_pool(pool: &SqlitePool) -> io::Result<Option<OrchRun>> {
    let row = sqlx::query(
        "SELECT id, status, agents_json, dep_graph_json, created_at, updated_at FROM orch_runs WHERE status = ?1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(RunStatus::Running.as_str())
    .fetch_optional(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;

    match row {
        Some(r) => Ok(Some(OrchRun {
            id: r
                .try_get::<String, _>(0)
                .map_err(|e| io::Error::other(e.to_string()))?,
            status: r
                .try_get::<String, _>(1)
                .map_err(|e| io::Error::other(e.to_string()))?,
            agents_json: r
                .try_get::<String, _>(2)
                .map_err(|e| io::Error::other(e.to_string()))?,
            dep_graph_json: r
                .try_get::<String, _>(3)
                .map_err(|e| io::Error::other(e.to_string()))?,
            created_at: r
                .try_get::<String, _>(4)
                .map_err(|e| io::Error::other(e.to_string()))?,
            updated_at: r
                .try_get::<String, _>(5)
                .map_err(|e| io::Error::other(e.to_string()))?,
        })),
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub async fn update_run_status_pool(
    pool: &SqlitePool,
    run_id: &str,
    status: RunStatus,
) -> io::Result<()> {
    sqlx::query("UPDATE orch_runs SET status = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(status.as_str())
        .bind(crate::shared::helpers::now_iso())
        .bind(run_id)
        .execute(pool)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn upsert_agent_pool(pool: &SqlitePool, agent: &OrchAgent) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_agents (id, run_id, role, task, satisfies_json, status, phase, progress, last_heartbeat, started_at, completed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"
    )
    .bind(&agent.id)
    .bind(&agent.run_id)
    .bind(&agent.role)
    .bind(&agent.task)
    .bind(&agent.satisfies_json)
    .bind(&agent.status)
    .bind(&agent.phase)
    .bind(agent.progress)
    .bind(&agent.last_heartbeat)
    .bind(&agent.started_at)
    .bind(&agent.completed_at)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn read_agent_pool(pool: &SqlitePool, agent_id: &str) -> io::Result<Option<OrchAgent>> {
    let row = sqlx::query(
        "SELECT id, run_id, role, task, satisfies_json, status, phase, progress, last_heartbeat, started_at, completed_at FROM orch_agents WHERE id = ?1"
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;

    match row {
        Some(r) => Ok(Some(OrchAgent {
            id: r
                .try_get::<String, _>(0)
                .map_err(|e| io::Error::other(e.to_string()))?,
            run_id: r
                .try_get::<String, _>(1)
                .map_err(|e| io::Error::other(e.to_string()))?,
            role: r
                .try_get::<String, _>(2)
                .map_err(|e| io::Error::other(e.to_string()))?,
            task: r
                .try_get::<String, _>(3)
                .map_err(|e| io::Error::other(e.to_string()))?,
            satisfies_json: r
                .try_get::<String, _>(4)
                .map_err(|e| io::Error::other(e.to_string()))?,
            status: r
                .try_get::<String, _>(5)
                .map_err(|e| io::Error::other(e.to_string()))?,
            phase: r
                .try_get::<String, _>(6)
                .map_err(|e| io::Error::other(e.to_string()))?,
            progress: r
                .try_get::<f64, _>(7)
                .map_err(|e| io::Error::other(e.to_string()))?,
            last_heartbeat: r
                .try_get::<String, _>(8)
                .map_err(|e| io::Error::other(e.to_string()))?,
            started_at: r
                .try_get::<Option<String>, _>(9)
                .map_err(|e| io::Error::other(e.to_string()))?,
            completed_at: r
                .try_get::<Option<String>, _>(10)
                .map_err(|e| io::Error::other(e.to_string()))?,
        })),
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub async fn dismiss_agent_pool(pool: &SqlitePool, agent_id: &str) -> io::Result<bool> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;

    let count_row = sqlx::query("SELECT COUNT(*) FROM orch_agents WHERE id = ?1")
        .bind(agent_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    let count: i64 = count_row
        .try_get::<i64, _>(0)
        .map_err(|e| io::Error::other(e.to_string()))?;
    if count == 0 {
        return Ok(false);
    }

    sqlx::query("DELETE FROM orch_agents WHERE id = ?1")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    sqlx::query("DELETE FROM orch_agent_events WHERE agent_id = ?1")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    sqlx::query("DELETE FROM orch_agent_inbox WHERE agent_id = ?1")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(true)
}

#[allow(dead_code)]
pub async fn append_event_pool(
    pool: &SqlitePool,
    agent_id: &str,
    timestamp: &str,
    event_type: &str,
    data_json: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_events (agent_id, timestamp, event_type, data_json) VALUES (?1,?2,?3,?4)"
    )
    .bind(agent_id)
    .bind(timestamp)
    .bind(event_type)
    .bind(data_json)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn post_inbox_pool(
    pool: &SqlitePool,
    agent_id: &str,
    from_agent: &str,
    timestamp: &str,
    message: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_inbox (agent_id, from_agent, timestamp, message) VALUES (?1,?2,?3,?4)"
    )
    .bind(agent_id)
    .bind(from_agent)
    .bind(timestamp)
    .bind(message)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn write_control_pool(
    pool: &SqlitePool,
    action: ControlAction,
    target: Option<&str>,
    message: Option<&str>,
    generation: i64,
) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_control (id, action, target, message, generation) VALUES (1, ?1, ?2, ?3, ?4)"
    )
    .bind(action.as_str())
    .bind(target)
    .bind(message)
    .bind(generation)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
    }

    #[test]
    fn init_and_read_run() {
        let conn = in_memory_db();
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_conn(&conn, "test-project", &run).unwrap();

        let loaded = read_run_conn(&conn).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "auto-123");
    }

    #[test]
    fn upsert_and_read_agent() {
        let conn = in_memory_db();
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_conn(&conn, "test-project", &run).unwrap();

        let agent = OrchAgent {
            id: "agent-1".into(),
            run_id: "auto-123".into(),
            role: "coder".into(),
            task: "implement feature".into(),
            satisfies_json: "[]".into(),
            status: "running".into(),
            phase: "executing".into(),
            progress: 0.5,
            last_heartbeat: "2026-06-02T10:01:00Z".into(),
            started_at: Some("2026-06-02T10:00:00Z".into()),
            completed_at: None,
        };
        upsert_agent_conn(&conn, &agent).unwrap();

        let loaded = read_agent_conn(&conn, "agent-1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().role, "coder");
    }

    #[test]
    fn dismiss_agent() {
        let conn = in_memory_db();
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_conn(&conn, "test-project", &run).unwrap();

        let agent = OrchAgent {
            id: "agent-1".into(),
            run_id: "auto-123".into(),
            role: "coder".into(),
            task: "fix bug".into(),
            satisfies_json: "[]".into(),
            status: "running".into(),
            phase: "executing".into(),
            progress: 0.5,
            last_heartbeat: "2026-06-02T10:01:00Z".into(),
            started_at: None,
            completed_at: None,
        };
        upsert_agent_conn(&conn, &agent).unwrap();

        let dismissed = dismiss_agent_conn(&conn, "agent-1").unwrap();
        assert!(dismissed);

        let loaded = read_agent_conn(&conn, "agent-1").unwrap();
        assert!(loaded.is_none());
    }
}
