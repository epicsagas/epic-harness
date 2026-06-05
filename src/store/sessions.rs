//! sessions.rs — Session snapshot SQLite I/O

use std::io;

use crate::shared::types::SessionSnapshot;

// ── Async pool functions ─────────────────────────────

use sqlx::{AnyPool, Row};

/// Map a sqlx row to a [`SessionSnapshot`].
fn row_to_snapshot_pool(r: &sqlx::any::AnyRow) -> io::Result<SessionSnapshot> {
    let g =
        |col: &str| -> Result<String, io::Error> { r.try_get(col).map_err(crate::store::sqlx_err) };
    let pending_json: String = g("pending_tasks")?;
    let pending_tasks: Vec<String> = serde_json::from_str(&pending_json).unwrap_or_default();
    let pipeline_json: Option<String> = r.try_get("pipeline_state").unwrap_or(None);
    let pipeline_state: Option<serde_json::Value> = pipeline_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    Ok(SessionSnapshot {
        timestamp: g("timestamp")?,
        snap_type: g("snap_type")?,
        summary: g("summary")?,
        pending_tasks,
        context_usage: r.try_get("context_usage").unwrap_or(None),
        pipeline_state,
    })
}

pub async fn insert_snapshot_pool(
    pool: &AnyPool,
    project: &str,
    snap: &SessionSnapshot,
    created_at_millis: i64,
) -> io::Result<i64> {
    let pending_json = serde_json::to_string(&snap.pending_tasks).unwrap_or_else(|e| {
        eprintln!("[store/sessions] pending_tasks serialization failed: {e}");
        "[]".into()
    });
    let pipeline_json = snap.pipeline_state.as_ref().map(|v| {
        serde_json::to_string(v).unwrap_or_else(|e| {
            eprintln!("[store/sessions] pipeline_state serialization failed: {e}");
            "{}".into()
        })
    });

    let result = sqlx::query("INSERT INTO sessions (timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state, created_at_millis, project) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(&snap.timestamp)
        .bind(&snap.snap_type)
        .bind(&snap.summary)
        .bind(&pending_json)
        .bind(snap.context_usage)
        .bind(pipeline_json)
        .bind(created_at_millis)
        .bind(project)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    // AnyPool::last_insert_id() returns None for SQLite via sqlx any-driver.
    // No caller depends on a non-zero return — insert success is verified by the
    // absence of an error from .execute().
    Ok(result.last_insert_id().unwrap_or(0))
}

pub async fn get_latest_snapshot_pool(
    pool: &AnyPool,
    project: &str,
) -> io::Result<Option<SessionSnapshot>> {
    let row = sqlx::query(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state FROM sessions WHERE project = $1 ORDER BY id DESC LIMIT 1"
    )
    .bind(project)
    .fetch_optional(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(row_to_snapshot_pool(&r)?)),
        None => Ok(None),
    }
}

pub async fn list_recent_snapshots_pool(
    pool: &AnyPool,
    project: &str,
    limit: i64,
) -> io::Result<Vec<SessionSnapshot>> {
    let rows = sqlx::query(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state FROM sessions WHERE project = $1 ORDER BY id DESC LIMIT $2"
    )
    .bind(project)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    rows.iter().map(row_to_snapshot_pool).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> sqlx::AnyPool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_get_latest() {
        let pool = in_memory_pool().await;
        let snap = SessionSnapshot {
            timestamp: "2026-06-02T10:00:00Z".into(),
            snap_type: "pre-compact".into(),
            summary: "Test summary".into(),
            pending_tasks: vec!["task1".into(), "task2".into()],
            context_usage: Some(0.75),
            pipeline_state: None,
        };

        insert_snapshot_pool(&pool, "test-project", &snap, 1000)
            .await
            .unwrap();

        let latest = get_latest_snapshot_pool(&pool, "test-project")
            .await
            .unwrap();
        assert!(latest.is_some());
        let s = latest.unwrap();
        assert_eq!(s.summary, "Test summary");
        assert_eq!(s.pending_tasks.len(), 2);
    }

    #[tokio::test]
    async fn get_latest_when_empty() {
        let pool = in_memory_pool().await;
        let result = get_latest_snapshot_pool(&pool, "test-project")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_recent_snapshots() {
        let pool = in_memory_pool().await;

        for i in 0..5 {
            let snap = SessionSnapshot {
                timestamp: format!("2026-06-02T10:0{}:00Z", i),
                snap_type: "pre-compact".into(),
                summary: format!("Session {}", i),
                pending_tasks: vec![],
                context_usage: None,
                pipeline_state: None,
            };
            insert_snapshot_pool(&pool, "test-project", &snap, 1000 + i as i64)
                .await
                .unwrap();
        }

        let recent = list_recent_snapshots_pool(&pool, "test-project", 3)
            .await
            .unwrap();
        assert_eq!(recent.len(), 3);
        // Most recent first (id DESC)
        assert_eq!(recent[0].summary, "Session 4");
    }
}
