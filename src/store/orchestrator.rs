//! orchestrator.rs — Orchestrator state SQLite I/O
//!
//! Replaces file-based orchestrator/ directory with SQLite tables.
//! See observations.rs for dead_code rationale.
#![allow(dead_code)]
//! flock(2) advisory locking is replaced by SQLite WAL transactions.

use rusqlite::Connection;
use std::io;

use super::{ImmediateTx, query_row_optional, store_err};

// ── Types ────────────────────────────────────────────

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
pub fn init_run_conn(conn: &Connection, run: &OrchRun) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orch_runs
         (id, status, agents_json, dep_graph_json, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![
            run.id,
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
         FROM orch_runs WHERE status = 'running' ORDER BY created_at DESC LIMIT 1",
        [],
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
pub fn update_run_status_conn(conn: &Connection, run_id: &str, status: &str) -> io::Result<()> {
    store_err(conn.execute(
        "UPDATE orch_runs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, crate::shared::helpers::now_iso(), run_id],
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
    action: &str,
    target: Option<&str>,
    message: Option<&str>,
    generation: i64,
) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orch_control (id, action, target, message, generation)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![action, target, message, generation],
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
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE status IN ('complete', 'aborted')",
            [],
        ))?;
    } else {
        store_err(conn.execute(
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE status IN ('complete', 'aborted') AND updated_at < ?1",
            rusqlite::params![cutoff_ts],
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
        init_run_conn(&conn, &run).unwrap();

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
        init_run_conn(&conn, &run).unwrap();

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
        init_run_conn(&conn, &run).unwrap();

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
