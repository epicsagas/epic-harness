//! orchestrator.rs — Orchestrator state SQLite I/O (async pool)
//!
//! Replaces file-based orchestrator/ directory with SQLite tables.
//! flock(2) advisory locking is replaced by SQLite WAL transactions.

use std::io;

// ── Types ────────────────────────────────────────────

/// Type-safe status for an orchestration run.
/// `OrchRun.status` remains `String` for JSON/DB round-trip; use `RunStatus` for function params.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
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
// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Start,
    Stop,
    Pause,
    Resume,
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
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

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, AnyPool};

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn init_run_pool(pool: &AnyPool, project: &str, run: &OrchRun) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_runs (id, project, status, agents_json, dep_graph_json, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (id) DO UPDATE SET project=excluded.project, status=excluded.status, agents_json=excluded.agents_json, dep_graph_json=excluded.dep_graph_json, updated_at=excluded.updated_at"
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
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

pub async fn read_run_pool(pool: &AnyPool, project: &str) -> io::Result<Option<OrchRun>> {
    let row = sqlx::query(
        "SELECT id, status, agents_json, dep_graph_json, created_at, updated_at FROM orch_runs WHERE project = $1 AND status = $2 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(project)
    .bind(RunStatus::Running.as_str())
    .fetch_optional(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(OrchRun {
            id: r.try_get::<String, _>(0).map_err(crate::store::sqlx_err)?,
            status: r.try_get::<String, _>(1).map_err(crate::store::sqlx_err)?,
            agents_json: r.try_get::<String, _>(2).map_err(crate::store::sqlx_err)?,
            dep_graph_json: r.try_get::<String, _>(3).map_err(crate::store::sqlx_err)?,
            created_at: r.try_get::<String, _>(4).map_err(crate::store::sqlx_err)?,
            updated_at: r.try_get::<String, _>(5).map_err(crate::store::sqlx_err)?,
        })),
        None => Ok(None),
    }
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn update_run_status_pool(
    pool: &AnyPool,
    run_id: &str,
    status: RunStatus,
) -> io::Result<()> {
    sqlx::query("UPDATE orch_runs SET status = $1, updated_at = $2 WHERE id = $3")
        .bind(status.as_str())
        .bind(crate::shared::helpers::now_iso())
        .bind(run_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn upsert_agent_pool(pool: &AnyPool, agent: &OrchAgent) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agents (id, run_id, role, task, satisfies_json, status, phase, progress, last_heartbeat, started_at, completed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (id) DO UPDATE SET run_id=excluded.run_id, role=excluded.role, task=excluded.task, satisfies_json=excluded.satisfies_json, status=excluded.status, phase=excluded.phase, progress=excluded.progress, last_heartbeat=excluded.last_heartbeat, started_at=excluded.started_at, completed_at=excluded.completed_at"
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
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

pub async fn read_agent_pool(
    pool: &AnyPool,
    project: &str,
    agent_id: &str,
) -> io::Result<Option<OrchAgent>> {
    let row = sqlx::query(
        "SELECT a.id, a.run_id, a.role, a.task, a.satisfies_json, a.status, a.phase, a.progress, a.last_heartbeat, a.started_at, a.completed_at FROM orch_agents a JOIN orch_runs r ON a.run_id = r.id WHERE a.id = $1 AND r.project = $2"
    )
    .bind(agent_id)
    .bind(project)
    .fetch_optional(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(OrchAgent {
            id: r.try_get::<String, _>(0).map_err(crate::store::sqlx_err)?,
            run_id: r.try_get::<String, _>(1).map_err(crate::store::sqlx_err)?,
            role: r.try_get::<String, _>(2).map_err(crate::store::sqlx_err)?,
            task: r.try_get::<String, _>(3).map_err(crate::store::sqlx_err)?,
            satisfies_json: r.try_get::<String, _>(4).map_err(crate::store::sqlx_err)?,
            status: r.try_get::<String, _>(5).map_err(crate::store::sqlx_err)?,
            phase: r.try_get::<String, _>(6).map_err(crate::store::sqlx_err)?,
            progress: r.try_get::<f64, _>(7).map_err(crate::store::sqlx_err)?,
            last_heartbeat: r.try_get::<String, _>(8).map_err(crate::store::sqlx_err)?,
            started_at: r
                .try_get::<Option<String>, _>(9)
                .map_err(crate::store::sqlx_err)?,
            completed_at: r
                .try_get::<Option<String>, _>(10)
                .map_err(crate::store::sqlx_err)?,
        })),
        None => Ok(None),
    }
}

pub async fn dismiss_agent_pool(
    pool: &AnyPool,
    project: &str,
    agent_id: &str,
) -> io::Result<bool> {
    let mut tx = pool.begin().await.map_err(crate::store::sqlx_err)?;

    let result = sqlx::query(
        "DELETE FROM orch_agents WHERE id = $1 AND run_id IN (SELECT id FROM orch_runs WHERE project = $2)"
    )
        .bind(agent_id)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    if result.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query("DELETE FROM orch_agent_events WHERE agent_id = $1")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    sqlx::query("DELETE FROM orch_agent_inbox WHERE agent_id = $1")
        .bind(agent_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;

    tx.commit().await.map_err(crate::store::sqlx_err)?;
    Ok(true)
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn append_event_pool(
    pool: &AnyPool,
    agent_id: &str,
    timestamp: &str,
    event_type: &str,
    data_json: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_events (agent_id, timestamp, event_type, data_json) VALUES ($1,$2,$3,$4)"
    )
    .bind(agent_id)
    .bind(timestamp)
    .bind(event_type)
    .bind(data_json)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn post_inbox_pool(
    pool: &AnyPool,
    agent_id: &str,
    from_agent: &str,
    timestamp: &str,
    message: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_agent_inbox (agent_id, from_agent, timestamp, message) VALUES ($1,$2,$3,$4)"
    )
    .bind(agent_id)
    .bind(from_agent)
    .bind(timestamp)
    .bind(message)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn write_control_pool(
    pool: &AnyPool,
    action: ControlAction,
    target: Option<&str>,
    message: Option<&str>,
    generation: i64,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO orch_control (id, action, target, message, generation) VALUES (1, $1, $2, $3, $4) ON CONFLICT (id) DO UPDATE SET action=excluded.action, target=excluded.target, message=excluded.message, generation=excluded.generation"
    )
    .bind(action.as_str())
    .bind(target)
    .bind(message)
    .bind(generation)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

/// Clean up completed/aborted runs and orphaned agents.
// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn cleanup_stale_pool(
    pool: &AnyPool,
    project: &str,
    cutoff_ts: &str,
) -> io::Result<u64> {
    let mut tx = pool.begin().await.map_err(crate::store::sqlx_err)?;

    sqlx::query("CREATE TEMP TABLE IF NOT EXISTS _stale_run_ids (id TEXT PRIMARY KEY); DELETE FROM _stale_run_ids")
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;

    if cutoff_ts.is_empty() {
        sqlx::query(
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE project = $1 AND (status = $2 OR status = $3)",
        )
        .bind(project)
        .bind(RunStatus::Complete.as_str())
        .bind(RunStatus::Aborted.as_str())
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    } else {
        sqlx::query(
            "INSERT INTO _stale_run_ids SELECT id FROM orch_runs WHERE project = $1 AND (status = $2 OR status = $3) AND updated_at < $4",
        )
        .bind(project)
        .bind(RunStatus::Complete.as_str())
        .bind(RunStatus::Aborted.as_str())
        .bind(cutoff_ts)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    }

    sqlx::query(
        "DELETE FROM orch_agent_events WHERE agent_id IN (
             SELECT id FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)
         )",
    )
    .execute(&mut *tx)
    .await
    .map_err(crate::store::sqlx_err)?;

    sqlx::query(
        "DELETE FROM orch_agent_inbox WHERE agent_id IN (
             SELECT id FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)
         )",
    )
    .execute(&mut *tx)
    .await
    .map_err(crate::store::sqlx_err)?;

    let result =
        sqlx::query("DELETE FROM orch_agents WHERE run_id IN (SELECT id FROM _stale_run_ids)")
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
    let mut count = result.rows_affected();

    let result = sqlx::query("DELETE FROM orch_runs WHERE id IN (SELECT id FROM _stale_run_ids)")
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    count += result.rows_affected();

    sqlx::query("DELETE FROM _stale_run_ids")
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;

    tx.commit().await.map_err(crate::store::sqlx_err)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> AnyPool {
        let p = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&p).await.unwrap();
        p
    }

    #[tokio::test]
    async fn init_and_read_run() {
        let pool = in_memory_pool().await;
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_pool(&pool, "test-project", &run).await.unwrap();

        let loaded = read_run_pool(&pool, "test-project").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "auto-123");
    }

    #[tokio::test]
    async fn upsert_and_read_agent() {
        let pool = in_memory_pool().await;
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_pool(&pool, "test-project", &run).await.unwrap();

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

        let loaded = read_agent_pool(&pool, "test-project", "agent-1")
            .await
            .unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().role, "coder");
    }

    #[tokio::test]
    async fn dismiss_agent() {
        let pool = in_memory_pool().await;
        let run = OrchRun {
            id: "auto-123".into(),
            status: "running".into(),
            agents_json: "[]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-06-02T10:00:00Z".into(),
            updated_at: "2026-06-02T10:00:00Z".into(),
        };
        init_run_pool(&pool, "test-project", &run).await.unwrap();

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

        let dismissed = dismiss_agent_pool(&pool, "test-project", "agent-1")
            .await
            .unwrap();
        assert!(dismissed);

        let loaded = read_agent_pool(&pool, "test-project", "agent-1")
            .await
            .unwrap();
        assert!(loaded.is_none());
    }
}
