//! orchestrator.rs — Orchestrator state SQLite I/O
#![allow(dead_code)]
//!
//! Replaces file-based orchestrator/ directory with SQLite tables.
//! flock(2) advisory locking is replaced by SQLite WAL transactions.

use sqlx::AnyPool;
use sqlx::Row;
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
pub async fn init_run_pool(pool: &AnyPool, run: &OrchRun) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_runs
         (id, status, agents_json, dep_graph_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&run.id)
    .bind(&run.status)
    .bind(&run.agents_json)
    .bind(&run.dep_graph_json)
    .bind(&run.created_at)
    .bind(&run.updated_at)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Read the current orchestration run.
pub async fn read_run_pool(pool: &AnyPool) -> io::Result<Option<OrchRun>> {
    let row = sqlx::query(
        "SELECT id, status, agents_json, dep_graph_json, created_at, updated_at
         FROM orch_runs WHERE status = 'running' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(OrchRun {
            id: r.try_get(0).map_err(super::sqlx_err)?,
            status: r.try_get(1).map_err(super::sqlx_err)?,
            agents_json: r.try_get(2).map_err(super::sqlx_err)?,
            dep_graph_json: r.try_get(3).map_err(super::sqlx_err)?,
            created_at: r.try_get(4).map_err(super::sqlx_err)?,
            updated_at: r.try_get(5).map_err(super::sqlx_err)?,
        })),
        None => Ok(None),
    }
}

/// Update run status.
pub async fn update_run_status_pool(pool: &AnyPool, run_id: &str, status: &str) -> io::Result<()> {
    sqlx::query("UPDATE orch_runs SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(crate::shared::helpers::now_iso())
        .bind(run_id)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(())
}

// ── Agent operations ─────────────────────────────────

/// Insert or update an agent.
pub async fn upsert_agent_pool(pool: &AnyPool, agent: &OrchAgent) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_agents
         (id, run_id, role, task, satisfies_json, status, phase, progress,
          last_heartbeat, started_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Read agent status by ID.
pub async fn read_agent_pool(pool: &AnyPool, agent_id: &str) -> io::Result<Option<OrchAgent>> {
    let row = sqlx::query(
        "SELECT id, run_id, role, task, satisfies_json, status, phase, progress,
                last_heartbeat, started_at, completed_at
         FROM orch_agents WHERE id = ?",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(OrchAgent {
            id: r.try_get(0).map_err(super::sqlx_err)?,
            run_id: r.try_get(1).map_err(super::sqlx_err)?,
            role: r.try_get(2).map_err(super::sqlx_err)?,
            task: r.try_get(3).map_err(super::sqlx_err)?,
            satisfies_json: r.try_get(4).map_err(super::sqlx_err)?,
            status: r.try_get(5).map_err(super::sqlx_err)?,
            phase: r.try_get(6).map_err(super::sqlx_err)?,
            progress: r.try_get(7).map_err(super::sqlx_err)?,
            last_heartbeat: r.try_get(8).map_err(super::sqlx_err)?,
            started_at: r.try_get(9).map_err(super::sqlx_err)?,
            completed_at: r.try_get(10).map_err(super::sqlx_err)?,
        })),
        None => Ok(None),
    }
}

/// Dismiss an agent: remove from agents table and cascade events/inbox.
pub async fn dismiss_agent_pool(pool: &AnyPool, agent_id: &str) -> io::Result<bool> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    // Check agent exists inside the transaction
    let row = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orch_agents WHERE id = ?")
        .bind(agent_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    if row == 0 {
        tx.rollback().await.map_err(super::sqlx_err)?;
        return Ok(false);
    }

    // Delete FK children first, then the agent
    sqlx::query("DELETE FROM orch_agent_events WHERE agent_id = ?")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("DELETE FROM orch_agent_inbox WHERE agent_id = ?")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("DELETE FROM orch_agents WHERE id = ?")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(true)
}

// ── Agent events ─────────────────────────────────────

/// Append an agent event.
pub async fn append_event_pool(
    pool: &AnyPool,
    agent_id: &str,
    timestamp: &str,
    event_type: &str,
    data_json: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_events (agent_id, timestamp, event_type, data_json)
         VALUES (?, ?, ?, ?)",
    )
    .bind(agent_id)
    .bind(timestamp)
    .bind(event_type)
    .bind(data_json)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

// ── Agent inbox ──────────────────────────────────────

/// Post a message to an agent's inbox.
pub async fn post_inbox_pool(
    pool: &AnyPool,
    agent_id: &str,
    from_agent: &str,
    timestamp: &str,
    message: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_inbox (agent_id, from_agent, timestamp, message)
         VALUES (?, ?, ?, ?)",
    )
    .bind(agent_id)
    .bind(from_agent)
    .bind(timestamp)
    .bind(message)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

// ── Control ──────────────────────────────────────────

/// Write a control directive (single-row table).
pub async fn write_control_pool(
    pool: &AnyPool,
    action: &str,
    target: Option<&str>,
    message: Option<&str>,
    generation: i64,
) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO orch_control (id, action, target, message, generation)
         VALUES (1, ?, ?, ?, ?)",
    )
    .bind(action)
    .bind(target)
    .bind(message)
    .bind(generation)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

// ── Cleanup ──────────────────────────────────────────

/// Clean up completed/aborted runs and orphaned agents.
pub async fn cleanup_stale_pool(pool: &AnyPool, cutoff_ts: &str) -> io::Result<u64> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;
    let mut count = 0u64;

    // Identify stale runs
    let stale_ids: Vec<String> = if cutoff_ts.is_empty() {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM orch_runs WHERE status IN ('complete', 'aborted')",
        )
        .fetch_all(&mut *tx)
        .await
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM orch_runs WHERE status IN ('complete', 'aborted') AND updated_at < ?",
        )
        .bind(cutoff_ts)
        .fetch_all(&mut *tx)
        .await
    }
    .map_err(super::sqlx_err)?;

    if stale_ids.is_empty() {
        tx.rollback().await.map_err(super::sqlx_err)?;
        return Ok(0);
    }

    // Delete children then parents
    for run_id in &stale_ids {
        let agent_ids: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM orch_agents WHERE run_id = ?")
                .bind(run_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(super::sqlx_err)?;

        for aid in &agent_ids {
            sqlx::query("DELETE FROM orch_agent_events WHERE agent_id = ?")
                .bind(aid)
                .execute(&mut *tx)
                .await
                .map_err(super::sqlx_err)?;
            sqlx::query("DELETE FROM orch_agent_inbox WHERE agent_id = ?")
                .bind(aid)
                .execute(&mut *tx)
                .await
                .map_err(super::sqlx_err)?;
        }

        let del = sqlx::query("DELETE FROM orch_agents WHERE run_id = ?")
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(super::sqlx_err)?;
        count += del.rows_affected();
    }

    for run_id in &stale_ids {
        let del = sqlx::query("DELETE FROM orch_runs WHERE id = ?")
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(super::sqlx_err)?;
        count += del.rows_affected();
    }

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    fn sample_run() -> OrchRun {
        OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn init_and_read_run() {
        let pool = test_pool().await;
        init_run_pool(&pool, &sample_run()).await.unwrap();

        let loaded = read_run_pool(&pool).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "auto-123");
    }

    #[tokio::test]
    async fn upsert_and_read_agent() {
        let pool = test_pool().await;
        init_run_pool(&pool, &sample_run()).await.unwrap();

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
        upsert_agent_pool(&pool, &agent).await.unwrap();

        let loaded = read_agent_pool(&pool, "agent-1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().role, "coder");
    }

    #[tokio::test]
    async fn dismiss_agent() {
        let pool = test_pool().await;
        init_run_pool(&pool, &sample_run()).await.unwrap();

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
        upsert_agent_pool(&pool, &agent).await.unwrap();

        let dismissed = dismiss_agent_pool(&pool, "agent-1").await.unwrap();
        assert!(dismissed);

        let loaded = read_agent_pool(&pool, "agent-1").await.unwrap();
        assert!(loaded.is_none());
    }
}
