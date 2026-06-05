//! orbit_store.rs — Orbit pipeline SQLite I/O
//!
//! Dual-write model: the /orbit skill writes PIPELINE-*.json files (required for
//! phase recovery via Claude Code's file tools); hooks sync those files to SQLite
//! at session-end (reflect) and pre-compact (snapshot) so the REST API and dashboard
//! have a queryable, up-to-date view without changing the skill.

use rusqlite::Connection;
use std::io;

use super::{query_row_optional, store_err};

const MAX_PIPELINE_LIST: usize = 200;

/// Upsert a pipeline state. If a pipeline with the same id exists, it's replaced.
#[allow(dead_code)]
pub fn upsert_pipeline_conn(
    conn: &Connection,
    id: &str,
    project: &str,
    status: &str,
    phase: Option<&str>,
    mode: Option<&str>,
    state_json: &str,
) -> io::Result<()> {
    let now = crate::shared::helpers::now_iso();
    store_err(conn.execute(
        "INSERT OR REPLACE INTO orbit_pipelines
         (id, project, status, phase, mode, state_json, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,
                 COALESCE((SELECT created_at FROM orbit_pipelines WHERE id = ?1), ?7),
                 ?8)",
        rusqlite::params![id, project, status, phase, mode, state_json, now, now],
    ))?;
    Ok(())
}

/// Find a running pipeline, optionally filtered by project.
#[allow(dead_code)]
pub fn read_running_pipeline_conn(
    conn: &Connection,
    project: Option<&str>,
) -> io::Result<Option<serde_json::Value>> {
    let query = match project {
        Some(_) => {
            "SELECT state_json FROM orbit_pipelines WHERE status = 'running' AND project = ?1 LIMIT 1"
        }
        None => "SELECT state_json FROM orbit_pipelines WHERE status = 'running' LIMIT 1",
    };

    let result = if project.is_some() {
        conn.query_row(query, rusqlite::params![project], |row| {
            row.get::<_, String>(0)
        })
    } else {
        conn.query_row(query, [], |row| row.get::<_, String>(0))
    };

    match query_row_optional(result)? {
        Some(json_str) => {
            let val: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_else(|e| {
                eprintln!("[store/orbit] malformed state_json, using empty object: {e}");
                serde_json::Value::Object(Default::default())
            });
            Ok(Some(val))
        }
        None => Ok(None),
    }
}

/// List all pipelines across all projects (for dashboard).
/// Capped at `MAX_PIPELINE_LIST` results to prevent unbounded memory usage.
pub fn list_all_pipelines_conn(conn: &Connection) -> io::Result<Vec<serde_json::Value>> {
    list_all_pipelines_conn_limited(conn, MAX_PIPELINE_LIST)
}

/// List pipelines with a custom limit.
pub fn list_all_pipelines_conn_limited(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    let mut stmt = store_err(conn.prepare(
        "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
             FROM orbit_pipelines ORDER BY created_at DESC LIMIT ?1",
    ))?;

    let rows = store_err(stmt.query_map(rusqlite::params![limit as i64], |row| {
        let id: String = row.get(0)?;
        let project: String = row.get(1)?;
        let status: String = row.get(2)?;
        let phase: Option<String> = row.get(3)?;
        let mode: Option<String> = row.get(4)?;
        let state_json: String = row.get(5)?;
        let created_at: String = row.get(6)?;
        let updated_at: String = row.get(7)?;

        // Merge metadata into the state JSON
        let mut val: serde_json::Value = serde_json::from_str(&state_json).unwrap_or_else(|e| {
            eprintln!("[store/orbit] malformed state_json in listing, using empty object: {e}");
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
        Ok(val)
    }))?;

    let mut pipelines = Vec::new();
    for r in rows {
        pipelines.push(store_err(r)?);
    }
    Ok(pipelines)
}

/// Dismiss (delete) a pipeline by ID.
#[allow(dead_code)]
pub fn dismiss_pipeline_conn(conn: &Connection, pipeline_id: &str) -> io::Result<bool> {
    let count = store_err(conn.execute(
        "DELETE FROM orbit_pipelines WHERE id = ?1",
        rusqlite::params![pipeline_id],
    ))?;
    Ok(count > 0)
}

/// Update pipeline status only.
#[allow(dead_code)]
pub fn update_pipeline_status_conn(
    conn: &Connection,
    pipeline_id: &str,
    status: &str,
) -> io::Result<bool> {
    let now = crate::shared::helpers::now_iso();
    let count = store_err(conn.execute(
        "UPDATE orbit_pipelines SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, pipeline_id],
    ))?;
    Ok(count > 0)
}

/// Sync all `PIPELINE-*.json` files in `orbit_dir` to the `orbit_pipelines` table.
///
/// Called by the reflect hook (session end) and snapshot hook (pre-compact).
/// Each file is upserted with `INSERT OR REPLACE`, so re-running is safe.
/// Returns the number of pipelines successfully synced.
pub fn sync_orbit_files_to_db_conn(
    conn: &Connection,
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
                // Fall back to filename stem (e.g. "PIPELINE-20260603-121409")
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

        if let Err(e) = upsert_pipeline_conn(conn, &id, &project, status, phase, mode, &content) {
            eprintln!("[store/orbit] upsert failed for {id}: {e}");
            continue;
        }
        synced += 1;
    }

    Ok(synced)
}

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub async fn dismiss_pipeline_pool(pool: &SqlitePool, pipeline_id: &str) -> io::Result<bool> {
    let result = sqlx::query("DELETE FROM orbit_pipelines WHERE id = ?1")
        .bind(pipeline_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

#[allow(dead_code)]
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

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
    }

    #[test]
    fn upsert_and_read_running() {
        let conn = in_memory_db();
        upsert_pipeline_conn(
            &conn,
            "PIPELINE-20260602-abc",
            "my-project",
            "running",
            Some("go"),
            Some("direct"),
            r#"{"requirement":"fix bug"}"#,
        )
        .unwrap();

        let result = read_running_pipeline_conn(&conn, Some("my-project")).unwrap();
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["requirement"], "fix bug");
    }

    #[test]
    fn list_all_pipelines() {
        let conn = in_memory_db();
        upsert_pipeline_conn(&conn, "PIPELINE-1", "proj-a", "running", None, None, "{}").unwrap();
        upsert_pipeline_conn(&conn, "PIPELINE-2", "proj-b", "complete", None, None, "{}").unwrap();

        let pipelines = list_all_pipelines_conn(&conn).unwrap();
        assert_eq!(pipelines.len(), 2);
    }

    #[test]
    fn dismiss_pipeline() {
        let conn = in_memory_db();
        upsert_pipeline_conn(&conn, "PIPELINE-1", "proj", "running", None, None, "{}").unwrap();

        let dismissed = dismiss_pipeline_conn(&conn, "PIPELINE-1").unwrap();
        assert!(dismissed);

        let result = read_running_pipeline_conn(&conn, None).unwrap();
        assert!(result.is_none());
    }
}
