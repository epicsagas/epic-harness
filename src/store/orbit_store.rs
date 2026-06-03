//! orbit_store.rs — Orbit pipeline SQLite I/O
//!
//! See observations.rs for dead_code rationale.
#![allow(dead_code)]

use rusqlite::Connection;
use std::io;

use super::store_err;

/// Upsert a pipeline state. If a pipeline with the same id exists, it's replaced.
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

    match result {
        Ok(json_str) => {
            let val: serde_json::Value = serde_json::from_str(&json_str)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            Ok(Some(val))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

/// List all pipelines across all projects (for dashboard).
/// Capped at `limit` results to prevent unbounded memory usage.
pub fn list_all_pipelines_conn(conn: &Connection) -> io::Result<Vec<serde_json::Value>> {
    list_all_pipelines_conn_limited(conn, 200)
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
        let mut val: serde_json::Value = serde_json::from_str(&state_json)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        if !val.is_object() {
            val = serde_json::Value::Object(Default::default());
        }
        let map = val.as_object_mut().unwrap();
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
        Ok(val)
    }))?;

    let mut pipelines = Vec::new();
    for r in rows {
        pipelines.push(store_err(r)?);
    }
    Ok(pipelines)
}

/// Dismiss (delete) a pipeline by ID.
pub fn dismiss_pipeline_conn(conn: &Connection, pipeline_id: &str) -> io::Result<bool> {
    let count = store_err(conn.execute(
        "DELETE FROM orbit_pipelines WHERE id = ?1",
        rusqlite::params![pipeline_id],
    ))?;
    Ok(count > 0)
}

/// Update pipeline status only.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        super::super::in_memory_db()
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
