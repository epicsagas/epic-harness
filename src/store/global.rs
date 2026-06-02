//! global.rs — Cross-project pattern SQLite I/O
#![allow(dead_code)]

use rusqlite::Connection;
use std::io;

/// Insert a global pattern record.
#[allow(clippy::too_many_arguments)]
pub fn insert_pattern_conn(
    conn: &Connection,
    timestamp: &str,
    project: &str,
    success_rate: f64,
    avg_score: f64,
    per_error_stats_json: &str,
    failure_patterns_json: &str,
    weak_tools_json: &str,
) -> io::Result<i64> {
    conn.execute(
        "INSERT INTO global_patterns
         (timestamp, project, success_rate, avg_score, per_error_stats,
          failure_patterns, weak_tools)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            timestamp,
            project,
            success_rate,
            avg_score,
            per_error_stats_json,
            failure_patterns_json,
            weak_tools_json,
        ],
    )
    .map_err(io::Error::other)?;
    Ok(conn.last_insert_rowid())
}

/// Query patterns for all projects except the given one.
pub fn query_patterns_excluding_conn(
    conn: &Connection,
    exclude_project: &str,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, project, success_rate, avg_score,
                    per_error_stats, failure_patterns, weak_tools
             FROM global_patterns
             WHERE project != ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )
        .map_err(io::Error::other)?;

    let rows = stmt
        .query_map(rusqlite::params![exclude_project, limit as i64], |row| {
            Ok(serde_json::json!({
                "timestamp": row.get::<_, String>(0)?,
                "project": row.get::<_, String>(1)?,
                "success_rate": row.get::<_, f64>(2)?,
                "avg_score": row.get::<_, f64>(3)?,
                "per_error_stats": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(4).unwrap_or_else(|_| "{}".into())
                ).unwrap_or_else(|_| serde_json::json!({})),
                "failure_patterns": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(5).unwrap_or_else(|_| "[]".into())
                ).unwrap_or_else(|_| serde_json::json!([])),
                "weak_tools": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(6).unwrap_or_else(|_| "[]".into())
                ).unwrap_or_else(|_| serde_json::json!([])),
            }))
        })
        .map_err(io::Error::other)?;

    let mut patterns = Vec::new();
    for r in rows {
        patterns.push(r.map_err(io::Error::other)?);
    }
    Ok(patterns)
}

/// Query all patterns (regardless of project).
pub fn query_all_patterns_conn(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, project, success_rate, avg_score,
                    per_error_stats, failure_patterns, weak_tools
             FROM global_patterns ORDER BY timestamp DESC LIMIT ?1",
        )
        .map_err(io::Error::other)?;

    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok(serde_json::json!({
                "timestamp": row.get::<_, String>(0)?,
                "project": row.get::<_, String>(1)?,
                "success_rate": row.get::<_, f64>(2)?,
                "avg_score": row.get::<_, f64>(3)?,
                "per_error_stats": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(4).unwrap_or_else(|_| "{}".into())
                ).unwrap_or_else(|_| serde_json::json!({})),
                "failure_patterns": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(5).unwrap_or_else(|_| "[]".into())
                ).unwrap_or_else(|_| serde_json::json!([])),
                "weak_tools": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(6).unwrap_or_else(|_| "[]".into())
                ).unwrap_or_else(|_| serde_json::json!([])),
            }))
        })
        .map_err(io::Error::other)?;

    let mut patterns = Vec::new();
    for r in rows {
        patterns.push(r.map_err(io::Error::other)?);
    }
    Ok(patterns)
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
    fn insert_and_query() {
        let conn = in_memory_db();
        insert_pattern_conn(
            &conn,
            "2026-06-02T10:00:00Z",
            "project-a",
            0.9,
            0.85,
            "{}",
            "[]",
            "[]",
        )
        .unwrap();

        let patterns = query_all_patterns_conn(&conn, 10).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["project"], "project-a");
        assert!(patterns[0]["per_error_stats"].is_object());
        assert!(patterns[0]["failure_patterns"].is_array());
        assert!(patterns[0]["weak_tools"].is_array());
    }

    #[test]
    fn query_excluding_project() {
        let conn = in_memory_db();
        insert_pattern_conn(
            &conn,
            "2026-06-02T10:00:00Z",
            "project-a",
            0.9,
            0.85,
            "{}",
            "[]",
            "[]",
        )
        .unwrap();
        insert_pattern_conn(
            &conn,
            "2026-06-02T11:00:00Z",
            "project-b",
            0.8,
            0.75,
            "{}",
            "[]",
            "[]",
        )
        .unwrap();

        let patterns = query_patterns_excluding_conn(&conn, "project-a", 10).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["project"], "project-b");
    }
}
