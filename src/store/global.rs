//! global.rs — Cross-project pattern SQLite I/O

use rusqlite::Connection;
use std::io;

use super::store_err;

/// Type-safe filter for pattern queries — prevents raw SQL injection.
pub enum PatternFilter {
    /// Return all patterns regardless of project.
    #[allow(dead_code)]
    All,
    /// Return patterns excluding the given project.
    Excluding(String),
}

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
    store_err(conn.execute(
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
    ))?;
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
        &PatternFilter::Excluding(exclude_project.to_string()),
        limit,
    )
}

/// Query all patterns (regardless of project).
#[allow(dead_code)]
pub fn query_all_patterns_conn(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_conn(conn, &PatternFilter::All, limit)
}

/// Shared query implementation for global patterns.
fn query_patterns_conn(
    conn: &Connection,
    filter: &PatternFilter,
    limit: usize,
) -> io::Result<Vec<serde_json::Value>> {
    let mut stmt = match filter {
        PatternFilter::Excluding(_) => store_err(conn.prepare(
            "SELECT timestamp, project, success_rate, avg_score,
                        per_error_stats, failure_patterns, weak_tools
                 FROM global_patterns
                 WHERE project != ?1
                 ORDER BY timestamp DESC LIMIT ?2",
        ))?,
        PatternFilter::All => store_err(conn.prepare(
            "SELECT timestamp, project, success_rate, avg_score,
                        per_error_stats, failure_patterns, weak_tools
                 FROM global_patterns
                 ORDER BY timestamp DESC LIMIT ?1",
        ))?,
    };

    let rows = match filter {
        PatternFilter::Excluding(project) => {
            store_err(stmt.query_map(rusqlite::params![project, limit as i64], map_pattern_row))?
        }
        PatternFilter::All => {
            store_err(stmt.query_map(rusqlite::params![limit as i64], map_pattern_row))?
        }
    };

    let mut patterns = Vec::new();
    for r in rows {
        let (ts, project, success_rate, avg_score, per_error_raw, failure_raw, weak_raw) =
            store_err(r)?;
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
    let per_err = row.get::<_, String>(4).unwrap_or_else(|e| {
        eprintln!(
            "[store/global] schema mismatch: per_error_stats col missing ({e}) — using fallback"
        );
        "{}".into()
    });
    let failure = row.get::<_, String>(5).unwrap_or_else(|e| {
        eprintln!(
            "[store/global] schema mismatch: failure_patterns col missing ({e}) — using fallback"
        );
        "[]".into()
    });
    let weak = row.get::<_, String>(6).unwrap_or_else(|e| {
        eprintln!("[store/global] schema mismatch: weak_tools col missing ({e}) — using fallback");
        "[]".into()
    });
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        per_err,
        failure,
        weak,
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

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

#[allow(dead_code, clippy::too_many_arguments)]
pub async fn insert_pattern_pool(
    pool: &SqlitePool,
    timestamp: &str,
    project: &str,
    success_rate: f64,
    avg_score: f64,
    per_error_stats_json: &str,
    failure_patterns_json: &str,
    weak_tools_json: &str,
) -> io::Result<i64> {
    let result = sqlx::query(
        "INSERT INTO global_patterns (timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools) VALUES (?1,?2,?3,?4,?5,?6,?7)"
    )
    .bind(timestamp)
    .bind(project)
    .bind(success_rate)
    .bind(avg_score)
    .bind(per_error_stats_json)
    .bind(failure_patterns_json)
    .bind(weak_tools_json)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result.last_insert_rowid())
}

#[allow(dead_code)]
pub async fn query_patterns_excluding_pool(
    pool: &SqlitePool,
    exclude_project: &str,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_pool_inner(pool, Some(exclude_project), limit).await
}

#[allow(dead_code)]
pub async fn query_all_patterns_pool(
    pool: &SqlitePool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_pool_inner(pool, None, limit).await
}

#[allow(dead_code)]
async fn query_patterns_pool_inner(
    pool: &SqlitePool,
    exclude_project: Option<&str>,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let rows = if let Some(proj) = exclude_project {
        sqlx::query(
            "SELECT timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools FROM global_patterns WHERE project != ?1 ORDER BY timestamp DESC LIMIT ?2"
        )
        .bind(proj)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools FROM global_patterns ORDER BY timestamp DESC LIMIT ?1"
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .map_err(crate::store::sqlx_err)?;

    let patterns: Vec<serde_json::Value> = rows.iter().map(|r| {
        let per_err: String = r.try_get(4).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: per_error_stats col missing ({e}) — using fallback");
            "{}".into()
        });
        let failure: String = r.try_get(5).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: failure_patterns col missing ({e}) — using fallback");
            "[]".into()
        });
        let weak: String = r.try_get(6).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: weak_tools col missing ({e}) — using fallback");
            "[]".into()
        });
        serde_json::json!({
            "timestamp": r.try_get::<String, _>(0).unwrap_or_default(),
            "project": r.try_get::<String, _>(1).unwrap_or_default(),
            "success_rate": r.try_get::<f64, _>(2).unwrap_or(0.0),
            "avg_score": r.try_get::<f64, _>(3).unwrap_or(0.0),
            "per_error_stats": parse_json_field(&per_err, serde_json::json!({})),
            "failure_patterns": parse_json_field(&failure, serde_json::json!([])),
            "weak_tools": parse_json_field(&weak, serde_json::json!([])),
        })
    }).collect();
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
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
