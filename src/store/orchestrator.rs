//! orchestrator.rs — Orchestrator state SQLite I/O
#![allow(dead_code)]
//!
//! Replaces file-based orchestrator/ directory with SQLite tables.
//! flock(2) advisory locking is replaced by SQLite WAL transactions.

use rusqlite::Connection;
use std::io;

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
    conn.execute(
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
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Read the current orchestration run.
pub fn read_run_conn(conn: &Connection) -> io::Result<Option<OrchRun>> {
    let result = conn.query_row(
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
    );

    match result {
        Ok(run) => Ok(Some(run)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

/// Update run status.
pub fn update_run_status_conn(conn: &Connection, run_id: &str, status: &str) -> io::Result<()> {
    conn.execute(
        "UPDATE orch_runs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, crate::shared::helpers::now_iso(), run_id],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

// ── Agent operations ─────────────────────────────────

/// Insert or update an agent.
pub fn upsert_agent_conn(conn: &Connection, agent: &OrchAgent) -> io::Result<()> {
    conn.execute(
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
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Read agent status by ID.
pub fn read_agent_conn(conn: &Connection, agent_id: &str) -> io::Result<Option<OrchAgent>> {
    let result = conn.query_row(
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
    );

    match result {
        Ok(agent) => Ok(Some(agent)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

/// Dismiss an agent: remove from agents table and update run JSON.
pub fn dismiss_agent_conn(conn: &Connection, agent_id: &str) -> io::Result<bool> {
    let tx = conn.unchecked_transaction().map_err(io::Error::other)?;

    // Check agent exists
    let exists: bool = tx
        .query_row(
            "SELECT COUNT(*) FROM orch_agents WHERE id = ?1",
            rusqlite::params![agent_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(io::Error::other)?
        > 0;

    if !exists {
        tx.rollback().map_err(io::Error::other)?;
        return Ok(false);
    }

    // Delete agent
    tx.execute(
        "DELETE FROM orch_agents WHERE id = ?1",
        rusqlite::params![agent_id],
    )
    .map_err(io::Error::other)?;

    // Delete agent events and inbox
    tx.execute(
        "DELETE FROM orch_agent_events WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    )
    .map_err(io::Error::other)?;
    tx.execute(
        "DELETE FROM orch_agent_inbox WHERE agent_id = ?1",
        rusqlite::params![agent_id],
    )
    .map_err(io::Error::other)?;

    tx.commit().map_err(io::Error::other)?;
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
    conn.execute(
        "INSERT INTO orch_agent_events (agent_id, timestamp, event_type, data_json)
         VALUES (?1,?2,?3,?4)",
        rusqlite::params![agent_id, timestamp, event_type, data_json],
    )
    .map_err(io::Error::other)?;
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
    conn.execute(
        "INSERT INTO orch_agent_inbox (agent_id, from_agent, timestamp, message)
         VALUES (?1,?2,?3,?4)",
        rusqlite::params![agent_id, from_agent, timestamp, message],
    )
    .map_err(io::Error::other)?;
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
    conn.execute(
        "INSERT OR REPLACE INTO orch_control (id, action, target, message, generation)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![action, target, message, generation],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

// ── Cleanup ──────────────────────────────────────────

/// Clean up completed/aborted runs and orphaned agents.
///
/// `cutoff_ts` is an ISO-8601 timestamp; only runs updated before this time
/// are removed. Pass an empty string to remove all completed/aborted runs.
///
/// TODO: extend to clean up runs by timestamp once callers can supply a
/// meaningful cutoff (e.g. "older than 7 days").
pub fn cleanup_stale_conn(conn: &Connection, cutoff_ts: &str) -> io::Result<u64> {
    let mut count = 0u64;

    let deleted_runs = if cutoff_ts.is_empty() {
        conn.execute(
            "DELETE FROM orch_runs WHERE status IN ('complete', 'aborted')",
            [],
        )
    } else {
        conn.execute(
            "DELETE FROM orch_runs WHERE status IN ('complete', 'aborted') AND updated_at < ?1",
            rusqlite::params![cutoff_ts],
        )
    }
    .map_err(io::Error::other)?;
    count += deleted_runs as u64;

    // Delete events and inbox messages for orphaned agents (no matching run)
    conn.execute(
        "DELETE FROM orch_agent_events WHERE agent_id IN (
             SELECT id FROM orch_agents WHERE run_id NOT IN (SELECT id FROM orch_runs)
         )",
        [],
    )
    .map_err(io::Error::other)?;
    conn.execute(
        "DELETE FROM orch_agent_inbox WHERE agent_id IN (
             SELECT id FROM orch_agents WHERE run_id NOT IN (SELECT id FROM orch_runs)
         )",
        [],
    )
    .map_err(io::Error::other)?;

    let deleted_agents = conn
        .execute(
            "DELETE FROM orch_agents WHERE run_id NOT IN (SELECT id FROM orch_runs)",
            [],
        )
        .map_err(io::Error::other)?;
    count += deleted_agents as u64;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();
        conn
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
