//! orbit_store.rs — Orbit pipeline SQLite I/O
#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

/// Upsert a pipeline state. If a pipeline with the same id exists, it's replaced.
pub async fn upsert_pipeline_pool(
    pool: &AnyPool,
    id: &str,
    project: &str,
    status: &str,
    phase: Option<&str>,
    mode: Option<&str>,
    state_json: &str,
) -> io::Result<()> {
    let now = crate::shared::helpers::now_iso();
    sqlx::query(
        "INSERT OR REPLACE INTO orbit_pipelines
         (id, project, status, phase, mode, state_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?,
                 COALESCE((SELECT created_at FROM orbit_pipelines WHERE id = ?), ?),
                 ?)",
    )
    .bind(id)
    .bind(project)
    .bind(status)
    .bind(phase)
    .bind(mode)
    .bind(state_json)
    .bind(id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Find a running pipeline, optionally filtered by project.
pub async fn read_running_pipeline_pool(
    pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<Option<serde_json::Value>> {
    let row = match project {
        Some(p) => {
            sqlx::query(
                "SELECT state_json FROM orbit_pipelines WHERE status = 'running' AND project = ? LIMIT 1",
            )
            .bind(p)
            .fetch_optional(pool)
            .await
        }
        None => {
            sqlx::query(
                "SELECT state_json FROM orbit_pipelines WHERE status = 'running' LIMIT 1",
            )
            .fetch_optional(pool)
            .await
        }
    }
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => {
            let json_str: String = r.try_get(0).map_err(super::sqlx_err)?;
            let val: serde_json::Value = crate::shared::helpers::parse_persisted_json(&json_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Ok(Some(val))
        }
        None => Ok(None),
    }
}

/// List all pipelines across all projects (for dashboard).
pub async fn list_all_pipelines_pool(pool: &AnyPool) -> io::Result<Vec<serde_json::Value>> {
    list_pipelines_scoped_pool(pool, None, None).await
}

/// List pipelines for one project, or across all projects when `project` is
/// `None` (most recent first), at most `limit` rows (`None` = no limit).
pub async fn list_pipelines_scoped_pool(
    pool: &AnyPool,
    project: Option<&str>,
    limit: Option<i64>,
) -> io::Result<Vec<serde_json::Value>> {
    // SQLite: a negative LIMIT means no limit.
    let limit = limit.unwrap_or(-1);
    let rows = match project {
        Some(p) => {
            sqlx::query(
                "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
                 FROM orbit_pipelines WHERE project = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(p)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query(
                "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
                 FROM orbit_pipelines ORDER BY created_at DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(super::sqlx_err)?;

    let mut pipelines = Vec::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get(0).map_err(super::sqlx_err)?;
        let project: String = r.try_get(1).map_err(super::sqlx_err)?;
        let status: String = r.try_get(2).map_err(super::sqlx_err)?;
        let phase: Option<String> = r.try_get(3).map_err(super::sqlx_err)?;
        let mode: Option<String> = r.try_get(4).map_err(super::sqlx_err)?;
        let state_json: String = r.try_get(5).map_err(super::sqlx_err)?;
        let created_at: String = r.try_get(6).map_err(super::sqlx_err)?;
        let updated_at: String = r.try_get(7).map_err(super::sqlx_err)?;

        // A corrupt or non-object state_json still lists the row (empty map).
        let mut map =
            match crate::shared::helpers::parse_persisted_json::<serde_json::Value>(&state_json) {
                Ok(serde_json::Value::Object(m)) => m,
                _ => Default::default(),
            };
        map.insert("id".into(), serde_json::Value::String(id));
        map.insert("project".into(), serde_json::Value::String(project));
        map.insert("status".into(), serde_json::Value::String(status));
        if let Some(p) = phase {
            map.insert("phase".into(), serde_json::Value::String(p));
        }
        if let Some(m) = mode {
            map.insert("mode".into(), serde_json::Value::String(m));
        }
        map.insert("started_at".into(), serde_json::Value::String(created_at));
        map.insert("updated_at".into(), serde_json::Value::String(updated_at));
        pipelines.push(serde_json::Value::Object(map));
    }
    Ok(pipelines)
}

/// List pipelines with a row limit (most recent first).
pub async fn list_all_pipelines_pool_limited(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let mut all = list_all_pipelines_pool(pool).await?;
    all.truncate(limit as usize);
    Ok(all)
}

/// Dismiss (delete) a pipeline by ID.
pub async fn dismiss_pipeline_pool(pool: &AnyPool, pipeline_id: &str) -> io::Result<bool> {
    let result = sqlx::query("DELETE FROM orbit_pipelines WHERE id = ?")
        .bind(pipeline_id)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

/// Update pipeline status only.
pub async fn update_pipeline_status_pool(
    pool: &AnyPool,
    pipeline_id: &str,
    status: &str,
) -> io::Result<bool> {
    let now = crate::shared::helpers::now_iso();
    let result = sqlx::query("UPDATE orbit_pipelines SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(&now)
        .bind(pipeline_id)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
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
    async fn upsert_and_read_running() {
        let pool = test_pool().await;
        upsert_pipeline_pool(
            &pool,
            "PIPELINE-20260602-abc",
            "my-project",
            "running",
            Some("go"),
            Some("direct"),
            r#"{"requirement":"fix bug"}"#,
        )
        .await
        .unwrap();

        let result = read_running_pipeline_pool(&pool, Some("my-project"))
            .await
            .unwrap();
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["requirement"], "fix bug");
    }

    #[tokio::test]
    async fn e_orbit_store_parses_bom_state() {
        let pool = test_pool().await;
        upsert_pipeline_pool(
            &pool,
            "PIPELINE-bom",
            "my-project",
            "running",
            None,
            None,
            "\u{feff}{\"requirement\":\"fix bug\"}",
        )
        .await
        .unwrap();

        let val = read_running_pipeline_pool(&pool, Some("my-project"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(val["requirement"], "fix bug");
    }

    #[tokio::test]
    async fn list_all_pipelines() {
        let pool = test_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj-a", "running", None, None, "{}")
            .await
            .unwrap();
        upsert_pipeline_pool(&pool, "PIPELINE-2", "proj-b", "complete", None, None, "{}")
            .await
            .unwrap();

        let pipelines = list_all_pipelines_pool(&pool).await.unwrap();
        assert_eq!(pipelines.len(), 2);
    }

    #[tokio::test]
    async fn list_pipelines_scoped_filters_by_project() {
        let pool = test_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj-a", "running", None, None, "{}")
            .await
            .unwrap();
        upsert_pipeline_pool(&pool, "PIPELINE-2", "proj-b", "complete", None, None, "{}")
            .await
            .unwrap();

        let b = list_pipelines_scoped_pool(&pool, Some("proj-b"), None)
            .await
            .unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0]["id"], "PIPELINE-2");
        assert!(
            list_pipelines_scoped_pool(&pool, Some("none"), None)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            list_pipelines_scoped_pool(&pool, None, None)
                .await
                .unwrap()
                .len(),
            2
        );
        // LIMIT applies in SQL, with and without a project.
        let one = list_pipelines_scoped_pool(&pool, None, Some(1))
            .await
            .unwrap();
        assert_eq!(one.len(), 1);
        let none = list_pipelines_scoped_pool(&pool, Some("proj-a"), Some(0))
            .await
            .unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn list_all_pipelines_tolerates_non_object_state_json() {
        let pool = test_pool().await;
        for (id, state) in [
            ("PIPELINE-1", "[]"),
            ("PIPELINE-2", "42"),
            ("PIPELINE-3", "x"),
        ] {
            upsert_pipeline_pool(&pool, id, "proj", "running", Some("go"), None, state)
                .await
                .unwrap();
        }

        let pipelines = list_all_pipelines_pool(&pool).await.unwrap();
        assert_eq!(pipelines.len(), 3, "rows with bad state_json stay listed");
        for p in &pipelines {
            assert_eq!(p["project"], "proj");
            assert_eq!(p["phase"], "go");
            assert!(p["id"].as_str().unwrap().starts_with("PIPELINE-"));
        }
    }

    #[tokio::test]
    async fn dismiss_pipeline() {
        let pool = test_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj", "running", None, None, "{}")
            .await
            .unwrap();

        let dismissed = dismiss_pipeline_pool(&pool, "PIPELINE-1").await.unwrap();
        assert!(dismissed);

        let result = read_running_pipeline_pool(&pool, None).await.unwrap();
        assert!(result.is_none());
    }
}
