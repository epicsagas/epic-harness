//! sessions.rs — Session snapshot SQLite I/O

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

use crate::shared::types::SessionSnapshot;

/// Insert a session snapshot.
pub async fn insert_snapshot_pool(
    pool: &AnyPool,
    snap: &SessionSnapshot,
    created_at_millis: i64,
) -> io::Result<i64> {
    let pending_json = serde_json::to_string(&snap.pending_tasks).unwrap_or_else(|_| "[]".into());
    let pipeline_json = snap
        .pipeline_state
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));

    sqlx::query(
        "INSERT INTO sessions
         (timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state, created_at_millis)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&snap.timestamp)
    .bind(&snap.snap_type)
    .bind(&snap.summary)
    .bind(&pending_json)
    .bind(snap.context_usage)
    .bind(&pipeline_json)
    .bind(created_at_millis)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(id)
}

/// Get the most recent session snapshot.
pub async fn get_latest_snapshot_pool(pool: &AnyPool) -> io::Result<Option<SessionSnapshot>> {
    let row = sqlx::query(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state
         FROM sessions ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => {
            let pending_json: String = r.try_get(3).map_err(super::sqlx_err)?;
            let pending_tasks: Vec<String> =
                serde_json::from_str(&pending_json).unwrap_or_default();
            let pipeline_json: Option<String> = r.try_get(5).map_err(super::sqlx_err)?;
            let pipeline_state: Option<serde_json::Value> = pipeline_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            Ok(Some(SessionSnapshot {
                timestamp: r.try_get(0).map_err(super::sqlx_err)?,
                snap_type: r.try_get(1).map_err(super::sqlx_err)?,
                summary: r.try_get(2).map_err(super::sqlx_err)?,
                pending_tasks,
                context_usage: r.try_get(4).map_err(super::sqlx_err)?,
                pipeline_state,
            }))
        }
        None => Ok(None),
    }
}

/// List the N most recent session snapshots.
pub async fn list_recent_snapshots_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<SessionSnapshot>> {
    let rows = sqlx::query(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state
         FROM sessions ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut snaps = Vec::with_capacity(rows.len());
    for r in rows {
        let pending_json: String = r.try_get(3).map_err(super::sqlx_err)?;
        let pending_tasks: Vec<String> =
            serde_json::from_str(&pending_json).unwrap_or_default();
        let pipeline_json: Option<String> = r.try_get(5).map_err(super::sqlx_err)?;
        let pipeline_state: Option<serde_json::Value> = pipeline_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        snaps.push(SessionSnapshot {
            timestamp: r.try_get(0).map_err(super::sqlx_err)?,
            snap_type: r.try_get(1).map_err(super::sqlx_err)?,
            summary: r.try_get(2).map_err(super::sqlx_err)?,
            pending_tasks,
            context_usage: r.try_get(4).map_err(super::sqlx_err)?,
            pipeline_state,
        });
    }
    Ok(snaps)
}

/// Alias: list recent snapshots without project filter (all projects).
#[allow(dead_code)]
pub async fn list_recent_snapshots_all_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<SessionSnapshot>> {
    list_recent_snapshots_pool(pool, limit).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_get_latest() {
        let pool = test_pool().await;
        let snap = SessionSnapshot {
            timestamp: "2026-06-02T10:00:00Z".into(),
            snap_type: "pre-compact".into(),
            summary: "Test summary".into(),
            pending_tasks: vec!["task1".into(), "task2".into()],
            context_usage: Some(0.75),
            pipeline_state: None,
        };

        insert_snapshot_pool(&pool, &snap, 1000).await.unwrap();

        let latest = get_latest_snapshot_pool(&pool).await.unwrap();
        assert!(latest.is_some());
        let s = latest.unwrap();
        assert_eq!(s.summary, "Test summary");
        assert_eq!(s.pending_tasks.len(), 2);
    }

    #[tokio::test]
    async fn get_latest_when_empty() {
        let pool = test_pool().await;
        let result = get_latest_snapshot_pool(&pool).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_recent_snapshots() {
        let pool = test_pool().await;

        for i in 0..5 {
            let snap = SessionSnapshot {
                timestamp: format!("2026-06-02T10:0{}:00Z", i),
                snap_type: "pre-compact".into(),
                summary: format!("Session {}", i),
                pending_tasks: vec![],
                context_usage: None,
                pipeline_state: None,
            };
            insert_snapshot_pool(&pool, &snap, 1000 + i as i64).await.unwrap();
        }

        let recent = list_recent_snapshots_pool(&pool, 3).await.unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].summary, "Session 4");
    }
}
