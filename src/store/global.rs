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
    query_patterns_conn(
        conn,
        Some(exclude_project),
        limit,
        "WHERE project != ?1",
    )
}

/// Query all patterns (regardless of project).
pub fn query_all_patterns_conn(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_conn(conn, None, limit, "")
}

/// Shared query implementation for global patterns.
fn query_patterns_conn(
    conn: &Connection,
    exclude_project: Option<&str>,
    limit: usize,
    where_clause: &str,
) -> io::Result<Vec<serde_json::Value>> {
    let sql = format!(
        "SELECT timestamp, project, success_rate, avg_score,
                per_error_stats, failure_patterns, weak_tools
         FROM global_patterns
         {where_clause}
         ORDER BY timestamp DESC LIMIT ?",
    );
    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;

    let rows = if exclude_project.is_some() {
        stmt.query_map(
            rusqlite::params![exclude_project, limit as i64],
            map_pattern_row,
        )
    } else {
        stmt.query_map(rusqlite::params![limit as i64], map_pattern_row)
    }
    .map_err(io::Error::other)?;

    let mut patterns = Vec::new();
    for r in rows {
        let (ts, project, success_rate, avg_score, per_error_raw, failure_raw, weak_raw) =
            r.map_err(io::Error::other)?;
        patterns.push(serde_json::json!({
            "timestamp": ts,
            "project": project,
            "success_rate": success_rate,
            "avg_score": avg_score,
            "per_error_stats": parse_json_field(&per_error_raw, serde_json::json!({})),
            "failure_patterns": parse_json_field(&failure_raw, serde_json::json!([])),
            "weak_tools": parse_json_field(&weak_raw, serde_json::json!([])),
        }));
    }
    Ok(patterns)
}

/// Row mapper for global_patterns queries.
type PatternRow = (String, String, f64, f64, String, String, String);

#[allow(clippy::type_complexity)]
fn map_pattern_row(row: &rusqlite::Row<'_>) -> Result<PatternRow, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get::<_, String>(4).unwrap_or_else(|_| "{}".into()),
        row.get::<_, String>(5).unwrap_or_else(|_| "[]".into()),
        row.get::<_, String>(6).unwrap_or_else(|_| "[]".into()),
    ))
}

/// Parse a JSON field from a DB column, logging a warning on failure.
fn parse_json_field(raw: &str, fallback: serde_json::Value) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[store/global] JSON parse failed ({}): '{}' — using fallback",
                e,
                &raw[..raw.len().min(100)]
            );
            fallback
        }
    }
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
