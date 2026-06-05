//! orbit_store.rs — Orbit pipeline SQLite I/O
//!
//! Dual-write model: the /orbit skill writes PIPELINE-*.json files (required for
//! phase recovery via Claude Code's file tools); hooks sync those files to SQLite
//! at session-end (reflect) and pre-compact (snapshot) so the REST API and dashboard
//! have a queryable, up-to-date view without changing the skill.

use std::io;

const MAX_PIPELINE_LIST: usize = 200;

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

/// Pool version of sync_orbit_files_to_db.
pub async fn sync_orbit_files_to_db_pool(
    pool: &SqlitePool,
    orbit_dir: &std::path::Path,
) -> io::Result<usize> {
    if !orbit_dir.is_dir() {
        return Ok(0);
    }

    let project = crate::shared::paths::project_slug();
    let mut synced = 0;

    let entries = std::fs::read_dir(orbit_dir)
        .map_err(io::Error::other)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.starts_with("PIPELINE-") && s.ends_with(".json")
        });

    for entry in entries {
        let path = entry.path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[store/orbit] failed to read {}: {e}", path.display());
                continue;
            }
        };

        let val: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[store/orbit] malformed JSON in {}: {e}", path.display());
                continue;
            }
        };

        let id = val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            })
            .to_string();

        let status = val
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let phase = val.get("phase").and_then(|v| v.as_str());
        let mode = val.get("mode").and_then(|v| v.as_str());

        if let Err(e) =
            upsert_pipeline_pool(pool, &id, &project, status, phase, mode, &content).await
        {
            eprintln!("[store/orbit] upsert failed for {id}: {e}");
            continue;
        }
        synced += 1;
    }

    Ok(synced)
}

pub async fn upsert_pipeline_pool(
    pool: &SqlitePool,
    id: &str,
    project: &str,
    status: &str,
    phase: Option<&str>,
    mode: Option<&str>,
    state_json: &str,
) -> io::Result<()> {
    let now = crate::shared::helpers::now_iso();
    sqlx::query(
        "INSERT OR REPLACE INTO orbit_pipelines (id, project, status, phase, mode, state_json, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6, COALESCE((SELECT created_at FROM orbit_pipelines WHERE id = ?1), ?7), ?8)"
    )
    .bind(id)
    .bind(project)
    .bind(status)
    .bind(phase)
    .bind(mode)
    .bind(state_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
pub async fn read_running_pipeline_pool(
    pool: &SqlitePool,
    project: Option<&str>,
) -> io::Result<Option<serde_json::Value>> {
    let row = if let Some(proj) = project {
        sqlx::query(
            "SELECT state_json FROM orbit_pipelines WHERE status = 'running' AND project = ?1 LIMIT 1"
        )
        .bind(proj)
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query(
            "SELECT state_json FROM orbit_pipelines WHERE status = 'running' LIMIT 1"
        )
        .fetch_optional(pool)
        .await
    }
    .map_err(crate::store::sqlx_err)?;

    match row {
        Some(r) => {
            let json_str: String = r.try_get(0).map_err(crate::store::sqlx_err)?;
            let val: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_else(|e| {
                eprintln!("[store/orbit] malformed state_json, using empty object: {e}");
                serde_json::Value::Object(Default::default())
            });
            Ok(Some(val))
        }
        None => Ok(None),
    }
}

pub async fn list_all_pipelines_pool(pool: &SqlitePool) -> io::Result<Vec<serde_json::Value>> {
    list_all_pipelines_pool_limited(pool, MAX_PIPELINE_LIST as i64).await
}

pub async fn list_all_pipelines_pool_limited(
    pool: &SqlitePool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT id, project, status, phase, mode, state_json, created_at, updated_at FROM orbit_pipelines ORDER BY created_at DESC LIMIT ?1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let pipelines: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let id: String = r.try_get(0).unwrap_or_default();
            let project: String = r.try_get(1).unwrap_or_default();
            let status: String = r.try_get(2).unwrap_or_default();
            let phase: Option<String> = r.try_get(3).unwrap_or(None);
            let mode: Option<String> = r.try_get(4).unwrap_or(None);
            let state_json: String = r.try_get(5).unwrap_or_default();
            let created_at: String = r.try_get(6).unwrap_or_default();
            let updated_at: String = r.try_get(7).unwrap_or_default();

            let mut val: serde_json::Value =
                serde_json::from_str(&state_json).unwrap_or_else(|e| {
                    eprintln!(
                        "[store/orbit] malformed state_json in listing, using empty object: {e}"
                    );
                    serde_json::Value::Object(Default::default())
                });
            if !val.is_object() {
                val = serde_json::Value::Object(Default::default());
            }
            if let Some(map) = val.as_object_mut() {
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
            }
            val
        })
        .collect();
    Ok(pipelines)
}

pub async fn dismiss_pipeline_pool(pool: &SqlitePool, pipeline_id: &str) -> io::Result<bool> {
    let result = sqlx::query("DELETE FROM orbit_pipelines WHERE id = ?1")
        .bind(pipeline_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
pub async fn update_pipeline_status_pool(
    pool: &SqlitePool,
    pipeline_id: &str,
    status: &str,
) -> io::Result<bool> {
    let now = crate::shared::helpers::now_iso();
    let result =
        sqlx::query("UPDATE orbit_pipelines SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(status)
            .bind(&now)
            .bind(pipeline_id)
            .execute(pool)
            .await
            .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> sqlx::SqlitePool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn upsert_and_read_running() {
        let pool = in_memory_pool().await;
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
    async fn list_all_pipelines() {
        let pool = in_memory_pool().await;
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
    async fn dismiss_pipeline() {
        let pool = in_memory_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj", "running", None, None, "{}")
            .await
            .unwrap();

        let dismissed = dismiss_pipeline_pool(&pool, "PIPELINE-1").await.unwrap();
        assert!(dismissed);

        let result = read_running_pipeline_pool(&pool, None).await.unwrap();
        assert!(result.is_none());
    }
}
