//! observations.rs — Observation records SQLite I/O
//!
//! Dual-API: `_conn()` variants are wired into hooks/serve; standalone functions
//! are public API for future CLI commands and batch import. Suppress dead_code
//! at module level because the store layer is built incrementally — not all
//! public functions have callers yet.
#![allow(dead_code)]

use rusqlite::Connection;
use std::io;

use crate::shared::obs::ObsRecord;
use crate::shared::scoring::ScoreDimensions;

use super::{query_row_optional, store_err};

/// Pad an ISO-8601 date string for lexicographic range comparison.
/// `"2026-06-02"` → `"2026-06-02T00:00:00"` / `"...T23:59:59"`.
fn pad_date(ts: &str, end_of_day: bool) -> String {
    if ts.len() == 10 {
        if end_of_day {
            format!("{ts}T23:59:59")
        } else {
            format!("{ts}T00:00:00")
        }
    } else {
        ts.to_string()
    }
}

/// Insert a single observation record.
pub fn insert_observation_conn(
    conn: &Connection,
    rec: &ObsRecord,
    session_id: &str,
) -> io::Result<i64> {
    let (dim_s, dim_q, dim_c) = match &rec.dimensions {
        Some(d) => (
            Some(d.tool_success),
            Some(d.output_quality),
            Some(d.execution_cost),
        ),
        None => (None, None, None),
    };
    store_err(conn.execute(
        "INSERT INTO observations
         (timestamp, session_id, tool, tool_category, action, result, score,
          dim_success, dim_quality, dim_cost, failure_category, error_snippet,
          file_ext, sequence_id, pipeline_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        rusqlite::params![
            rec.timestamp,
            session_id,
            rec.tool,
            rec.tool_category,
            rec.action,
            rec.result,
            rec.score,
            dim_s,
            dim_q,
            dim_c,
            rec.failure_category,
            rec.error_snippet,
            rec.file_ext,
            rec.sequence_id.map(super::u64_to_i64),
            rec.pipeline_id,
        ],
    ))?;
    Ok(conn.last_insert_rowid())
}

/// Standalone insert — opens own connection.
/// Currently unused; retained as public API for future batch import scenarios.
#[allow(dead_code)]
pub fn insert_observation(rec: &ObsRecord, session_id: &str) -> io::Result<i64> {
    let conn = super::open_harness_db()?;
    insert_observation_conn(&conn, rec, session_id)
}

/// Query observations for a date range (inclusive).
/// `from_ts` and `to_ts` are ISO-8601 date strings like "2026-06-02".
/// Automatically pads with T00:00:00 / T23:59:59 for range comparison.
/// Set `limit` to `None` for unlimited (use with caution on large date ranges).
pub fn query_obs_for_date_range_conn(
    conn: &Connection,
    from_ts: &str,
    to_ts: &str,
    limit: Option<usize>,
) -> io::Result<Vec<ObsRecord>> {
    let from = pad_date(from_ts, false);
    let to = pad_date(to_ts, true);
    // Use parameterized LIMIT for consistency with other queries.
    // SQLite treats -1 as "no limit", so use that when None is passed.
    let limit_val = limit.map(|l| l.min(50_000) as i64).unwrap_or(-1);

    let sql = "SELECT timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost,
                failure_category, error_snippet, file_ext, sequence_id, pipeline_id
         FROM observations
         WHERE timestamp >= ?1 AND timestamp <= ?2
         ORDER BY timestamp ASC
         LIMIT ?3";

    let mut stmt = store_err(conn.prepare(sql))?;

    let rows = store_err(
        stmt.query_map(rusqlite::params![from, to, limit_val], |row| {
            let dim_s: Option<f64> = row.get(6)?;
            let dim_q: Option<f64> = row.get(7)?;
            let dim_c: Option<f64> = row.get(8)?;
            Ok(ObsRecord {
                timestamp: row.get(0)?,
                tool: row.get(1)?,
                tool_category: row.get(2)?,
                action: row.get(3)?,
                result: row.get(4)?,
                score: row.get(5)?,
                dimensions: {
                    let any_some = dim_s.is_some() || dim_q.is_some() || dim_c.is_some();
                    let all_some = dim_s.is_some() && dim_q.is_some() && dim_c.is_some();
                    if any_some && !all_some {
                        eprintln!(
                            "[store] observations: partial dimensions (s={}, q={}, c={}) — \
                             treating as None to prevent score distortion",
                            dim_s.is_some(),
                            dim_q.is_some(),
                            dim_c.is_some()
                        );
                        // Treat partially-NULL dimensions as entirely absent to avoid
                        // distorting aggregates with spurious 0.0 ("complete failure") values.
                        None
                    } else if all_some {
                        Some(ScoreDimensions {
                            tool_success: dim_s.unwrap_or(0.0),
                            output_quality: dim_q.unwrap_or(0.0),
                            execution_cost: dim_c.unwrap_or(0.0),
                        })
                    } else {
                        None
                    }
                },
                failure_category: row.get(9)?,
                error_snippet: row.get(10)?,
                file_ext: row.get(11)?,
                sequence_id: row.get::<_, Option<i64>>(12)?.map(|v| v as u64),
                pipeline_id: row.get(13)?,
            })
        }),
    )?;

    let mut records = Vec::new();
    for r in rows {
        records.push(store_err(r)?);
    }
    Ok(records)
}

/// Aggregate observation stats via SQL.
/// Returns (total_count, success_count, avg_score, per_tool_stats_json, per_error_stats_json).
pub fn query_obs_stats_conn(conn: &Connection, from_ts: &str, to_ts: &str) -> io::Result<ObsStats> {
    let from = pad_date(from_ts, false);
    let to = pad_date(to_ts, true);

    // Overall stats
    let (total, successes, avg_score): (i64, i64, f64) = store_err(conn.query_row(
        "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(score), 0.0)
             FROM observations
             WHERE timestamp >= ?1 AND timestamp <= ?2",
        rusqlite::params![from, to],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ))?;

    // Per-tool stats — capped at 100 distinct tools to prevent unbounded result sets
    let mut tool_stmt = store_err(conn.prepare(
        "SELECT tool, COUNT(*) as calls,
                    SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as successes,
                    COALESCE(AVG(score), 0.0) as avg_score
             FROM observations
             WHERE timestamp >= ?1 AND timestamp <= ?2
             GROUP BY tool
             ORDER BY calls DESC
             LIMIT 100",
    ))?;

    let tool_rows = store_err(tool_stmt.query_map(rusqlite::params![from, to], |row| {
        Ok(ToolStatRow {
            tool: row.get(0)?,
            calls: row.get(1)?,
            successes: row.get(2)?,
            avg_score: row.get(3)?,
        })
    }))?;

    let mut tool_stats = Vec::new();
    for r in tool_rows {
        tool_stats.push(store_err(r)?);
    }

    // Per-error-category stats — capped at 50 distinct categories
    let mut err_stmt = store_err(conn.prepare(
        "SELECT failure_category, COUNT(*) as cnt
             FROM observations
             WHERE timestamp >= ?1 AND timestamp <= ?2
               AND failure_category IS NOT NULL
             GROUP BY failure_category
             ORDER BY cnt DESC
             LIMIT 50",
    ))?;

    let err_rows = store_err(err_stmt.query_map(rusqlite::params![from, to], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }))?;

    let mut error_stats = Vec::new();
    for r in err_rows {
        error_stats.push(store_err(r)?);
    }

    // Per-session stats
    let mut sess_stmt = store_err(conn
        .prepare(
            "SELECT session_id, COUNT(*) as calls,
                    COALESCE(AVG(score), 0.0) as avg_score,
                    SUM(CASE WHEN result != 'success' AND result IS NOT NULL THEN 1 ELSE 0 END) as failures
             FROM observations
             WHERE timestamp >= ?1 AND timestamp <= ?2
             GROUP BY session_id
             ORDER BY session_id DESC
             LIMIT 20",
        ))?;

    let sess_rows = store_err(sess_stmt.query_map(rusqlite::params![from, to], |row| {
        Ok(SessionStatRow {
            session_id: row.get(0)?,
            calls: row.get(1)?,
            avg_score: row.get(2)?,
            failures: row.get(3)?,
        })
    }))?;

    let mut session_stats = Vec::new();
    for r in sess_rows {
        session_stats.push(store_err(r)?);
    }

    Ok(ObsStats {
        total,
        successes,
        avg_score,
        tool_stats,
        error_stats,
        session_stats,
    })
}

/// Get the last action for a given session (replaces file tail read).
pub fn query_last_action_conn(conn: &Connection, session_id: &str) -> io::Result<Option<String>> {
    query_row_optional(conn.query_row(
        "SELECT action FROM observations
         WHERE session_id = ?1
         ORDER BY id DESC LIMIT 1",
        rusqlite::params![session_id],
        |row| row.get(0),
    ))
}

/// Delete observations older than the cutoff timestamp.
/// Returns the number of deleted rows.
pub fn delete_obs_older_than_conn(conn: &Connection, cutoff_ts: &str) -> io::Result<u64> {
    let count = store_err(conn.execute(
        "DELETE FROM observations WHERE timestamp < ?1",
        rusqlite::params![cutoff_ts],
    ))?;
    Ok(count as u64)
}

// ── Stats types ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ObsStats {
    pub total: i64,
    pub successes: i64,
    pub avg_score: f64,
    pub tool_stats: Vec<ToolStatRow>,
    pub error_stats: Vec<(String, i64)>,
    pub session_stats: Vec<SessionStatRow>,
}

#[derive(Debug, Clone)]
pub struct ToolStatRow {
    pub tool: String,
    pub calls: i64,
    pub successes: i64,
    pub avg_score: f64,
}

#[derive(Debug, Clone)]
pub struct SessionStatRow {
    pub session_id: String,
    pub calls: i64,
    pub avg_score: f64,
    pub failures: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
    }

    #[test]
    fn insert_and_query_observation() {
        let conn = in_memory_db();
        let rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("cargo test".into()),
            result: Some("success".into()),
            score: Some(0.95),
            dimensions: Some(ScoreDimensions {
                tool_success: 1.0,
                output_quality: 0.9,
                execution_cost: 1.0,
            }),
            failure_category: None,
            error_snippet: None,
            file_ext: Some(".rs".into()),
            sequence_id: Some(1),
            pipeline_id: None,
        };

        let id = insert_observation_conn(&conn, &rec, "20260602_12345").unwrap();
        assert!(id > 0);

        let results =
            query_obs_for_date_range_conn(&conn, "2026-06-02", "2026-06-02", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, "Bash");
        assert_eq!(results[0].score, Some(0.95));
    }

    #[test]
    fn query_stats_empty() {
        let conn = in_memory_db();
        let stats = query_obs_stats_conn(&conn, "2026-06-01", "2026-06-30").unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.successes, 0);
    }

    #[test]
    fn query_stats_with_data() {
        let conn = in_memory_db();

        for i in 0..5 {
            let rec = ObsRecord {
                timestamp: format!("2026-06-02T10:0{}:00Z", i),
                tool: "Edit".into(),
                tool_category: "edit".into(),
                action: Some("fix bug".into()),
                result: if i < 4 {
                    Some("success".into())
                } else {
                    Some("error".into())
                },
                score: Some(0.8),
                dimensions: None,
                failure_category: if i == 4 {
                    Some("syntax_error".into())
                } else {
                    None
                },
                error_snippet: None,
                file_ext: None,
                sequence_id: None,
                pipeline_id: None,
            };
            insert_observation_conn(&conn, &rec, "20260602_12345").unwrap();
        }

        let stats = query_obs_stats_conn(&conn, "2026-06-02", "2026-06-02").unwrap();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.tool_stats.len(), 1);
        assert_eq!(stats.tool_stats[0].tool, "Edit");
        assert_eq!(stats.error_stats.len(), 1);
        assert_eq!(stats.error_stats[0].0, "syntax_error");
    }

    #[test]
    fn delete_old_observations() {
        let conn = in_memory_db();

        let rec = ObsRecord {
            timestamp: "2026-05-01T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: None,
            result: Some("success".into()),
            score: Some(0.5),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };
        insert_observation_conn(&conn, &rec, "20260501_12345").unwrap();

        let deleted = delete_obs_older_than_conn(&conn, "2026-05-15").unwrap();
        assert_eq!(deleted, 1);

        let results =
            query_obs_for_date_range_conn(&conn, "2026-05-01", "2026-05-31", None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn query_last_action() {
        let conn = in_memory_db();

        let rec1 = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("first command".into()),
            result: Some("success".into()),
            score: None,
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };
        let rec2 = ObsRecord {
            timestamp: "2026-06-02T10:01:00Z".into(),
            tool: "Edit".into(),
            tool_category: "edit".into(),
            action: Some("second edit".into()),
            result: Some("success".into()),
            score: None,
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };

        insert_observation_conn(&conn, &rec1, "sess1").unwrap();
        insert_observation_conn(&conn, &rec2, "sess1").unwrap();

        let last = query_last_action_conn(&conn, "sess1").unwrap();
        assert_eq!(last, Some("second edit".to_string()));
    }
}
