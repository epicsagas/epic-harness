//! observations.rs — Observation records SQLite I/O

#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use sqlx::any::AnyRow;
use std::io;

use crate::shared::obs::ObsRecord;
use crate::shared::scoring::ScoreDimensions;

/// Normalize one date bound to the ISO shape the `timestamp` column stores.
///
/// Accepts `YYYYMMDD` (what `shared::helpers::today()` produces) and
/// `YYYY-MM-DD`, expanding both to `YYYY-MM-DDTHH:MM:SS`. Anything else passes
/// through so callers may still supply an exact timestamp.
///
/// `timestamp` is compared lexicographically, and the bare `20260727` form
/// sorts *above* every `2026-07-27T..` value because `'0'` (0x30) > `'-'`
/// (0x2D). An un-normalized `YYYYMMDD` bound therefore matches zero rows
/// instead of a whole day — which silently starved the Ring 3 evolution loop.
fn iso_bound(raw: &str, end_of_day: bool) -> String {
    let dashed = if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..8])
    } else {
        raw.to_string()
    };
    if dashed.len() == 10 {
        let suffix = if end_of_day { "T23:59:59" } else { "T00:00:00" };
        format!("{dashed}{suffix}")
    } else {
        dashed
    }
}

/// Inclusive ISO bounds for a `(from, to)` date pair. Single source of truth
/// for every observation range query.
pub fn day_bounds(from_ts: &str, to_ts: &str) -> (String, String) {
    (iso_bound(from_ts, false), iso_bound(to_ts, true))
}

/// Insert a single observation record.
///
/// `project` is the project slug to attribute this observation to. The caller
/// (observe hook) resolves it via `paths::project_slug()`; passing it explicitly
/// keeps the store layer free of CWD/git dependencies.
pub async fn insert_observation_pool(
    pool: &AnyPool,
    rec: &ObsRecord,
    session_id: &str,
    project: &str,
) -> io::Result<i64> {
    let (dim_s, dim_q, dim_c) = match &rec.dimensions {
        Some(d) => (
            Some(d.tool_success),
            Some(d.output_quality),
            Some(d.execution_cost),
        ),
        None => (None, None, None),
    };
    sqlx::query(
        "INSERT INTO observations
         (timestamp, session_id, tool, tool_category, action, result, score,
          dim_success, dim_quality, dim_cost, failure_category, error_snippet,
          file_ext, sequence_id, pipeline_id, tool_use_id, project)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&rec.timestamp)
    .bind(session_id)
    .bind(&rec.tool)
    .bind(&rec.tool_category)
    .bind(&rec.action)
    .bind(&rec.result)
    .bind(rec.score)
    .bind(dim_s)
    .bind(dim_q)
    .bind(dim_c)
    .bind(&rec.failure_category)
    .bind(&rec.error_snippet)
    .bind(&rec.file_ext)
    .bind(rec.sequence_id.map(super::u64_to_i64))
    .bind(&rec.pipeline_id)
    .bind(&rec.tool_use_id)
    .bind(project)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(id)
}

/// Standalone insert — uses global pool via runtime bridge.
#[allow(dead_code)]
pub fn insert_observation(rec: &ObsRecord, session_id: &str) -> io::Result<i64> {
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        let project = crate::shared::paths::project_slug();
        insert_observation_pool(&pool, rec, session_id, &project).await
    })
}

/// Query observations for a date range (inclusive), optionally scoped to one
/// project.
///
/// When `project` is `Some(p)` only rows attributed to `p` are returned. Rows
/// written before observations carried a project (legacy `NULL`) are therefore
/// excluded from scoped reads — they cannot be attributed after the fact.
/// `None` aggregates across every project.
pub async fn query_obs_for_date_range_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
    project: Option<&str>,
) -> io::Result<Vec<ObsRecord>> {
    let (from, to) = day_bounds(from_ts, to_ts);

    // Two static-SQL branches so sqlx 0.9's SqlSafeStr guard (which rejects
    // dynamically built strings) is satisfied — same pattern as the stats query.
    let rows = if let Some(p) = project {
        sqlx::query(
            "SELECT timestamp, tool, tool_category, action, result, score,
                    dim_success, dim_quality, dim_cost,
                    failure_category, error_snippet, file_ext, sequence_id, pipeline_id, tool_use_id
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ? AND project = ?
             ORDER BY timestamp ASC",
        )
        .bind(&from)
        .bind(&to)
        .bind(p)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT timestamp, tool, tool_category, action, result, score,
                    dim_success, dim_quality, dim_cost,
                    failure_category, error_snippet, file_ext, sequence_id, pipeline_id, tool_use_id
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ?
             ORDER BY timestamp ASC",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let dim_s: Option<f64> = row.try_get(6).ok();
        let dim_q: Option<f64> = row.try_get(7).ok();
        let dim_c: Option<f64> = row.try_get(8).ok();

        let any_some = dim_s.is_some() || dim_q.is_some() || dim_c.is_some();
        let all_some = dim_s.is_some() && dim_q.is_some() && dim_c.is_some();
        if any_some && !all_some {
            eprintln!(
                "[store] observations: partial dimensions (s={}, q={}, c={}) — \
                 defaulting missing fields to 0.0",
                dim_s.is_some(),
                dim_q.is_some(),
                dim_c.is_some()
            );
        }

        let seq_id: Option<i64> = row.try_get(12).ok();
        records.push(ObsRecord {
            timestamp: row.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            tool: row.try_get::<String, _>(1).map_err(super::sqlx_err)?,
            tool_category: row.try_get::<String, _>(2).map_err(super::sqlx_err)?,
            action: row
                .try_get::<Option<String>, _>(3)
                .map_err(super::sqlx_err)?,
            result: row
                .try_get::<Option<String>, _>(4)
                .map_err(super::sqlx_err)?,
            score: row.try_get::<Option<f64>, _>(5).map_err(super::sqlx_err)?,
            dimensions: if any_some {
                Some(ScoreDimensions {
                    tool_success: dim_s.unwrap_or(0.0),
                    output_quality: dim_q.unwrap_or(0.0),
                    execution_cost: dim_c.unwrap_or(0.0),
                })
            } else {
                None
            },
            failure_category: row
                .try_get::<Option<String>, _>(9)
                .map_err(super::sqlx_err)?,
            error_snippet: row
                .try_get::<Option<String>, _>(10)
                .map_err(super::sqlx_err)?,
            file_ext: row
                .try_get::<Option<String>, _>(11)
                .map_err(super::sqlx_err)?,
            sequence_id: seq_id.map(|v| v as u64),
            pipeline_id: row
                .try_get::<Option<String>, _>(13)
                .map_err(super::sqlx_err)?,
            tool_use_id: row
                .try_get::<Option<String>, _>(14)
                .map_err(super::sqlx_err)?,
        });
    }
    Ok(records)
}

/// Observation row with database provenance preserved for cross-store merging.
#[derive(Debug, Clone)]
pub(crate) struct StoredObservation {
    pub row_id: i64,
    pub session_id: String,
    pub project: String,
    pub record: ObsRecord,
}

fn decode_stored_observations(rows: Vec<AnyRow>) -> io::Result<Vec<StoredObservation>> {
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let dim_s: Option<f64> = row.try_get(9).ok();
        let dim_q: Option<f64> = row.try_get(10).ok();
        let dim_c: Option<f64> = row.try_get(11).ok();
        let any_some = dim_s.is_some() || dim_q.is_some() || dim_c.is_some();
        let seq_id: Option<i64> = row.try_get(15).ok();
        records.push(StoredObservation {
            row_id: row.try_get(0).map_err(super::sqlx_err)?,
            session_id: row.try_get(1).map_err(super::sqlx_err)?,
            project: row
                .try_get::<Option<String>, _>(2)
                .map_err(super::sqlx_err)?
                .unwrap_or_default(),
            record: ObsRecord {
                timestamp: row.try_get(3).map_err(super::sqlx_err)?,
                tool: row.try_get(4).map_err(super::sqlx_err)?,
                tool_category: row.try_get(5).map_err(super::sqlx_err)?,
                action: row.try_get(6).map_err(super::sqlx_err)?,
                result: row.try_get(7).map_err(super::sqlx_err)?,
                score: row.try_get(8).map_err(super::sqlx_err)?,
                dimensions: any_some.then(|| ScoreDimensions {
                    tool_success: dim_s.unwrap_or(0.0),
                    output_quality: dim_q.unwrap_or(0.0),
                    execution_cost: dim_c.unwrap_or(0.0),
                }),
                failure_category: row.try_get(12).map_err(super::sqlx_err)?,
                error_snippet: row.try_get(13).map_err(super::sqlx_err)?,
                file_ext: row.try_get(14).map_err(super::sqlx_err)?,
                sequence_id: seq_id.map(|v| v as u64),
                pipeline_id: row.try_get(16).map_err(super::sqlx_err)?,
                tool_use_id: row.try_get(17).map_err(super::sqlx_err)?,
            },
        });
    }
    Ok(records)
}

/// Load one project's observations for one host session.
///
/// The inner descending query keeps the most recent `limit` rows when a session
/// exceeds the reflection budget. The outer ascending order restores event
/// order for pattern analysis. Database row ids remain attached as provenance,
/// so same-second identical calls are never collapsed.
pub(crate) async fn query_obs_for_session_pool(
    pool: &AnyPool,
    session_id: &str,
    project: &str,
    limit: i64,
) -> io::Result<Vec<StoredObservation>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT id, session_id, project, timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost, failure_category,
                error_snippet, file_ext, sequence_id, pipeline_id, tool_use_id
         FROM observations
         WHERE id IN (
             SELECT id FROM observations
             WHERE session_id = ? AND project = ?
             ORDER BY id DESC LIMIT ?
         )
         ORDER BY id ASC",
    )
    .bind(session_id)
    .bind(project)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    decode_stored_observations(rows)
}

/// Bounded date-range load for reflection context.
pub(crate) async fn query_obs_for_date_range_bounded_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
    project: Option<&str>,
    limit: i64,
) -> io::Result<Vec<StoredObservation>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let (from, to) = day_bounds(from_ts, to_ts);
    let rows = if let Some(project) = project {
        sqlx::query(
            "SELECT id, session_id, project, timestamp, tool, tool_category, action, result, score,
                    dim_success, dim_quality, dim_cost, failure_category,
                    error_snippet, file_ext, sequence_id, pipeline_id, tool_use_id
             FROM observations
             WHERE id IN (
                 SELECT id FROM observations
                 WHERE timestamp >= ? AND timestamp <= ? AND project = ?
                 ORDER BY timestamp DESC, id DESC LIMIT ?
             )
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(&from)
        .bind(&to)
        .bind(project)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, session_id, project, timestamp, tool, tool_category, action, result, score,
                    dim_success, dim_quality, dim_cost, failure_category,
                    error_snippet, file_ext, sequence_id, pipeline_id, tool_use_id
             FROM observations
             WHERE id IN (
                 SELECT id FROM observations
                 WHERE timestamp >= ? AND timestamp <= ?
                 ORDER BY timestamp DESC, id DESC LIMIT ?
             )
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(&from)
        .bind(&to)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;
    decode_stored_observations(rows)
}

/// Aggregate observation stats via SQL.
pub async fn query_obs_stats_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
) -> io::Result<ObsStats> {
    let (from, to) = day_bounds(from_ts, to_ts);

    // Overall stats
    let row = sqlx::query(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END), 0),
                COALESCE(AVG(score), 0.0),
                COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0)
         FROM observations
         WHERE timestamp >= ? AND timestamp <= ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await
    .map_err(super::sqlx_err)?;

    let total: i64 = row.try_get(0).map_err(super::sqlx_err)?;
    let successes: i64 = row.try_get(1).map_err(super::sqlx_err)?;
    let avg_score: f64 = row.try_get(2).map_err(super::sqlx_err)?;
    let unknowns: i64 = row.try_get(3).map_err(super::sqlx_err)?;

    // Per-tool stats
    let tool_rows = sqlx::query(
        "SELECT tool, COUNT(*) as calls,
                SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as successes,
                COALESCE(AVG(score), 0.0) as avg_score,
                COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0) as unknowns
         FROM observations
         WHERE timestamp >= ? AND timestamp <= ?
         GROUP BY tool
         ORDER BY calls DESC
         LIMIT 100",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut tool_stats = Vec::with_capacity(tool_rows.len());
    for r in tool_rows {
        tool_stats.push(ToolStatRow {
            tool: r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            calls: r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
            successes: r.try_get::<i64, _>(2).map_err(super::sqlx_err)?,
            avg_score: r.try_get::<f64, _>(3).map_err(super::sqlx_err)?,
            unknowns: r.try_get::<i64, _>(4).map_err(super::sqlx_err)?,
        });
    }

    // Per-error stats
    let err_rows = sqlx::query(
        "SELECT failure_category, COUNT(*) as cnt
         FROM observations
         WHERE timestamp >= ? AND timestamp <= ?
           AND failure_category IS NOT NULL
         GROUP BY failure_category
         ORDER BY cnt DESC
         LIMIT 50",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut error_stats = Vec::with_capacity(err_rows.len());
    for r in err_rows {
        error_stats.push((
            r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
        ));
    }

    // Per-session stats. Failures are counted from the explicit 'error' value:
    // `result != 'success'` would also sweep in 'unknown', which is exactly the
    // outcome we could not determine.
    let sess_rows = sqlx::query(
        "SELECT session_id, COUNT(*) as calls,
                COALESCE(AVG(score), 0.0) as avg_score,
                SUM(CASE WHEN result = 'error' THEN 1 ELSE 0 END) as failures
         FROM observations
         WHERE timestamp >= ? AND timestamp <= ?
         GROUP BY session_id
         ORDER BY session_id DESC
         LIMIT 20",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut session_stats = Vec::with_capacity(sess_rows.len());
    for r in sess_rows {
        session_stats.push(SessionStatRow {
            session_id: r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            calls: r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
            avg_score: r.try_get::<f64, _>(2).map_err(super::sqlx_err)?,
            failures: r.try_get::<i64, _>(3).map_err(super::sqlx_err)?,
        });
    }

    Ok(ObsStats {
        total,
        successes,
        unknowns,
        avg_score,
        tool_stats,
        error_stats,
        session_stats,
    })
}

/// Alias: query obs stats without project filter (all projects).
pub async fn query_obs_stats_all_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
) -> io::Result<ObsStats> {
    query_obs_stats_pool(pool, from_ts, to_ts).await
}

/// Project-scoped observation stats.
///
/// When `project` is `Some(p)`, every query adds `AND project = ?`. When
/// `None`, behaves like the unfiltered variant (cross-project aggregate).
pub async fn query_obs_stats_scoped_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
    project: Option<&str>,
) -> io::Result<ObsStats> {
    let (from, to) = day_bounds(from_ts, to_ts);

    // Overall stats — two static-SQL branches so sqlx 0.9's SqlSafeStr guard
    // (which rejects dynamic strings to prevent injection) is satisfied.
    let (total, successes, avg_score, unknowns): (i64, i64, f64, i64) = if let Some(p) = project {
        let row = sqlx::query(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(score), 0.0),
                    COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0)
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ? AND project = ?",
        )
        .bind(&from)
        .bind(&to)
        .bind(p)
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
        (
            row.try_get(0).map_err(super::sqlx_err)?,
            row.try_get(1).map_err(super::sqlx_err)?,
            row.try_get(2).map_err(super::sqlx_err)?,
            row.try_get(3).map_err(super::sqlx_err)?,
        )
    } else {
        let row = sqlx::query(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(score), 0.0),
                    COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0)
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ?",
        )
        .bind(&from)
        .bind(&to)
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
        (
            row.try_get(0).map_err(super::sqlx_err)?,
            row.try_get(1).map_err(super::sqlx_err)?,
            row.try_get(2).map_err(super::sqlx_err)?,
            row.try_get(3).map_err(super::sqlx_err)?,
        )
    };

    // Per-tool stats
    let tool_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT tool, COUNT(*) as calls,
                    SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as successes,
                    COALESCE(AVG(score), 0.0) as avg_score,
                    COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0) as unknowns
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ? AND project = ?
             GROUP BY tool ORDER BY calls DESC LIMIT 100",
        )
        .bind(&from)
        .bind(&to)
        .bind(p)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT tool, COUNT(*) as calls,
                    SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as successes,
                    COALESCE(AVG(score), 0.0) as avg_score,
                    COALESCE(SUM(CASE WHEN result = 'unknown' OR result IS NULL THEN 1 ELSE 0 END), 0) as unknowns
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ?
             GROUP BY tool ORDER BY calls DESC LIMIT 100",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;
    let mut tool_stats = Vec::with_capacity(tool_rows.len());
    for r in tool_rows {
        tool_stats.push(ToolStatRow {
            tool: r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            calls: r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
            successes: r.try_get::<i64, _>(2).map_err(super::sqlx_err)?,
            avg_score: r.try_get::<f64, _>(3).map_err(super::sqlx_err)?,
            unknowns: r.try_get::<i64, _>(4).map_err(super::sqlx_err)?,
        });
    }

    // Per-error stats
    let err_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT failure_category, COUNT(*) as cnt
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ? AND project = ?
               AND failure_category IS NOT NULL
             GROUP BY failure_category ORDER BY cnt DESC LIMIT 50",
        )
        .bind(&from)
        .bind(&to)
        .bind(p)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT failure_category, COUNT(*) as cnt
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ?
               AND failure_category IS NOT NULL
             GROUP BY failure_category ORDER BY cnt DESC LIMIT 50",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;
    let mut error_stats = Vec::with_capacity(err_rows.len());
    for r in err_rows {
        error_stats.push((
            r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
        ));
    }

    // Per-session stats
    let sess_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT session_id, COUNT(*) as calls,
                    COALESCE(AVG(score), 0.0) as avg_score,
                    SUM(CASE WHEN result = 'error' THEN 1 ELSE 0 END) as failures
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ? AND project = ?
             GROUP BY session_id ORDER BY session_id DESC LIMIT 20",
        )
        .bind(&from)
        .bind(&to)
        .bind(p)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT session_id, COUNT(*) as calls,
                    COALESCE(AVG(score), 0.0) as avg_score,
                    SUM(CASE WHEN result = 'error' THEN 1 ELSE 0 END) as failures
             FROM observations
             WHERE timestamp >= ? AND timestamp <= ?
             GROUP BY session_id ORDER BY session_id DESC LIMIT 20",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;
    let mut session_stats = Vec::with_capacity(sess_rows.len());
    for r in sess_rows {
        session_stats.push(SessionStatRow {
            session_id: r.try_get::<String, _>(0).map_err(super::sqlx_err)?,
            calls: r.try_get::<i64, _>(1).map_err(super::sqlx_err)?,
            avg_score: r.try_get::<f64, _>(2).map_err(super::sqlx_err)?,
            failures: r.try_get::<i64, _>(3).map_err(super::sqlx_err)?,
        });
    }

    Ok(ObsStats {
        total,
        successes,
        unknowns,
        avg_score,
        tool_stats,
        error_stats,
        session_stats,
    })
}

/// Get the last action for a given session.
pub async fn query_last_action_pool(
    pool: &AnyPool,
    session_id: &str,
) -> io::Result<Option<String>> {
    let row = sqlx::query(
        "SELECT action FROM observations
         WHERE session_id = ?
         ORDER BY id DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(r.try_get::<String, _>(0).map_err(super::sqlx_err)?)),
        None => Ok(None),
    }
}

/// Delete observations older than the cutoff timestamp.
/// Returns the number of deleted rows.
pub async fn delete_obs_older_than_pool(pool: &AnyPool, cutoff_ts: &str) -> io::Result<u64> {
    let result = sqlx::query("DELETE FROM observations WHERE timestamp < ?")
        .bind(cutoff_ts)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(result.rows_affected())
}

/// Delete expired rows while preserving the session currently being reflected.
/// A long-running session can start before the cutoff and must remain readable
/// until its SessionEnd analysis has completed.
pub async fn delete_obs_older_than_except_session_pool(
    pool: &AnyPool,
    cutoff_ts: &str,
    session_id: &str,
    project: &str,
) -> io::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM observations
         WHERE timestamp < ? AND NOT (session_id = ? AND project = ?)",
    )
    .bind(cutoff_ts)
    .bind(session_id)
    .bind(project)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(result.rows_affected())
}

/// Delete expired rows while preserving every queued or actively-reflecting
/// session. A global retention sweep can be triggered by another project, so
/// excluding only its ending session would still destroy a long-running peer.
pub async fn delete_obs_older_than_except_sessions_pool(
    pool: &AnyPool,
    cutoff_ts: &str,
    sessions: &[(String, String)],
) -> io::Result<u64> {
    if sessions.is_empty() {
        return delete_obs_older_than_pool(pool, cutoff_ts).await;
    }

    let sessions_json = serde_json::to_string(sessions).map_err(io::Error::other)?;
    let result = sqlx::query(
        "DELETE FROM observations
         WHERE timestamp < ?
           AND NOT EXISTS (
                SELECT 1 FROM json_each(?) AS active
                WHERE json_extract(active.value, '$[0]') = observations.session_id
                  AND json_extract(active.value, '$[1]') = observations.project
           )",
    )
    .bind(cutoff_ts)
    .bind(sessions_json)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(result.rows_affected())
}

// ── Stats types ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ObsStats {
    /// Every observation in range, including those with an undetermined outcome.
    pub total: i64,
    pub successes: i64,
    /// Observations the host gave no outcome evidence for. Neither a success nor
    /// a failure — see `evaluated`. A NULL `result` counts here too: rows written
    /// before the tri-state outcome existed carry no verdict, and reading them as
    /// successes inflated every rate on the dashboard.
    pub unknowns: i64,
    pub avg_score: f64,
    pub tool_stats: Vec<ToolStatRow>,
    pub error_stats: Vec<(String, i64)>,
    pub session_stats: Vec<SessionStatRow>,
}

impl ObsStats {
    /// Observations with a determined outcome — the correct denominator for a
    /// success rate. Using `total` would let undetermined calls read as failures.
    pub fn evaluated(&self) -> i64 {
        self.total - self.unknowns
    }
}

#[derive(Debug, Clone)]
pub struct ToolStatRow {
    pub tool: String,
    pub calls: i64,
    pub successes: i64,
    pub avg_score: f64,
    /// Calls with an undetermined outcome. See `ObsStats::unknowns`.
    pub unknowns: i64,
}

impl ToolStatRow {
    /// Calls with a determined outcome — the denominator for this tool's rate.
    pub fn evaluated(&self) -> i64 {
        self.calls - self.unknowns
    }
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

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_query_observation() {
        let pool = test_pool().await;
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
            tool_use_id: None,
        };

        let id = insert_observation_pool(&pool, &rec, "20260602_12345", "test-project")
            .await
            .unwrap();
        assert!(id > 0);

        let results = query_obs_for_date_range_pool(&pool, "2026-06-02", "2026-06-02", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, "Bash");
        assert_eq!(results[0].score, Some(0.95));
    }

    /// Regression: `helpers::today()` yields `YYYYMMDD`, and that form must
    /// match the same rows as the dashed form. Before `day_bounds()` the bare
    /// digits sorted above every `YYYY-MM-DDT..` timestamp, so reflect read
    /// zero observations and the Ring 3 loop never closed.
    #[tokio::test]
    async fn compact_date_bound_matches_same_day() {
        let pool = test_pool().await;
        let rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("cargo test".into()),
            result: Some("success".into()),
            score: Some(0.9),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "20260602_1", "p1")
            .await
            .unwrap();

        let compact = query_obs_for_date_range_pool(&pool, "20260602", "20260602", None)
            .await
            .unwrap();
        let dashed = query_obs_for_date_range_pool(&pool, "2026-06-02", "2026-06-02", None)
            .await
            .unwrap();
        assert_eq!(compact.len(), 1, "YYYYMMDD bound must match the day");
        assert_eq!(compact.len(), dashed.len());
    }

    #[test]
    fn day_bounds_normalizes_both_date_forms() {
        assert_eq!(
            day_bounds("20260602", "20260602"),
            (
                "2026-06-02T00:00:00".to_string(),
                "2026-06-02T23:59:59".to_string()
            )
        );
        assert_eq!(
            day_bounds("2026-06-02", "2026-06-02"),
            (
                "2026-06-02T00:00:00".to_string(),
                "2026-06-02T23:59:59".to_string()
            )
        );
        // Exact timestamps pass through untouched.
        let exact = day_bounds("2026-06-02T08:30:00", "2026-06-02T09:00:00");
        assert_eq!(exact.0, "2026-06-02T08:30:00");
        assert_eq!(exact.1, "2026-06-02T09:00:00");
        // Non-numeric 8-char input is not misread as a compact date.
        assert_eq!(day_bounds("garbage!", "garbage!").0, "garbage!");
    }

    /// Reflection must not mix other projects' observations into this
    /// project's analysis.
    #[tokio::test]
    async fn date_range_query_scopes_to_project() {
        let pool = test_pool().await;
        let mk = |tool: &str| ObsRecord {
            timestamp: "2026-06-02T10:00:00".into(),
            tool: tool.into(),
            tool_category: "bash".into(),
            action: None,
            result: Some("success".into()),
            score: Some(0.8),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &mk("Mine"), "s1", "mine")
            .await
            .unwrap();
        insert_observation_pool(&pool, &mk("Theirs"), "s2", "theirs")
            .await
            .unwrap();

        let scoped = query_obs_for_date_range_pool(&pool, "20260602", "20260602", Some("mine"))
            .await
            .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].tool, "Mine");

        let all = query_obs_for_date_range_pool(&pool, "20260602", "20260602", None)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn reflection_query_is_session_scoped_and_keeps_distinct_calls() {
        let pool = test_pool().await;
        let rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("cargo test".into()),
            result: Some("success".into()),
            score: Some(0.9),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: Some(0),
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "session-a", "project-a")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "session-a", "project-a")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "session-b", "project-a")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "session-a", "project-b")
            .await
            .unwrap();

        let rows = query_obs_for_session_pool(&pool, "session-a", "project-a", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].row_id, rows[1].row_id);
    }

    #[tokio::test]
    async fn reflection_query_caps_to_most_recent_records_in_time_order() {
        let pool = test_pool().await;
        for i in 0..4 {
            let rec = ObsRecord {
                timestamp: format!("2026-06-02T10:00:0{i}"),
                tool: format!("tool-{i}"),
                tool_category: "bash".into(),
                action: None,
                result: Some("success".into()),
                score: Some(0.9),
                dimensions: None,
                failure_category: None,
                error_snippet: None,
                file_ext: None,
                sequence_id: Some(i),
                pipeline_id: None,
                tool_use_id: None,
            };
            insert_observation_pool(&pool, &rec, "session-a", "project-a")
                .await
                .unwrap();
        }

        let rows = query_obs_for_session_pool(&pool, "session-a", "project-a", 2)
            .await
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].record.tool, "tool-2");
        assert_eq!(rows[1].record.tool, "tool-3");
    }

    #[tokio::test]
    async fn context_range_query_is_project_scoped_and_bounded() {
        let pool = test_pool().await;
        for (project, tool) in [
            ("project-a", "a-1"),
            ("project-a", "a-2"),
            ("project-a", "a-3"),
            ("project-b", "b-1"),
        ] {
            let rec = ObsRecord {
                timestamp: "2026-06-02T10:00:00".into(),
                tool: tool.into(),
                tool_category: "bash".into(),
                action: None,
                result: Some("success".into()),
                score: Some(0.9),
                dimensions: None,
                failure_category: None,
                error_snippet: None,
                file_ext: None,
                sequence_id: None,
                pipeline_id: None,
                tool_use_id: None,
            };
            insert_observation_pool(&pool, &rec, "session-a", project)
                .await
                .unwrap();
        }

        let rows = query_obs_for_date_range_bounded_pool(
            &pool,
            "20260602",
            "20260602",
            Some("project-a"),
            2,
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.project == "project-a"));
        assert_eq!(rows[0].record.tool, "a-2");
        assert_eq!(rows[1].record.tool, "a-3");
    }

    #[tokio::test]
    async fn query_stats_empty() {
        let pool = test_pool().await;
        let stats = query_obs_stats_pool(&pool, "2026-06-01", "2026-06-30")
            .await
            .unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.successes, 0);
    }

    #[tokio::test]
    async fn query_stats_with_data() {
        let pool = test_pool().await;

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
                tool_use_id: None,
            };
            insert_observation_pool(&pool, &rec, "20260602_12345", "test-project")
                .await
                .unwrap();
        }

        let stats = query_obs_stats_pool(&pool, "2026-06-02", "2026-06-02")
            .await
            .unwrap();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.tool_stats.len(), 1);
        assert_eq!(stats.tool_stats[0].tool, "Edit");
        assert_eq!(stats.error_stats.len(), 1);
        assert_eq!(stats.error_stats[0].0, "syntax_error");
    }

    /// Regression: a row with no verdict was counted as a success. Rows written
    /// before the tri-state outcome existed carry `result = NULL`, and so did
    /// every subagent spawn once `Agent` was wired to `PreToolUse` — each one
    /// added a phantom success and hid real failures behind a diluted rate.
    #[tokio::test]
    async fn null_result_is_unknown_not_success() {
        let pool = test_pool().await;

        let mut rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: "Agent".into(),
            tool_category: "other".into(),
            action: None,
            result: None,
            score: None,
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "20260602_12345", "test-project")
            .await
            .unwrap();

        rec.timestamp = "2026-06-02T10:01:00Z".into();
        rec.result = Some("error".into());
        rec.score = Some(0.3);
        rec.failure_category = Some("runtime_error".into());
        insert_observation_pool(&pool, &rec, "20260602_12345", "test-project")
            .await
            .unwrap();

        let stats = query_obs_stats_pool(&pool, "2026-06-02", "2026-06-02")
            .await
            .unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.successes, 0, "a verdict-less row is not a success");
        assert_eq!(stats.unknowns, 1, "NULL is an undetermined outcome");
        assert_eq!(
            stats.evaluated(),
            1,
            "only the row with a real verdict belongs in the denominator"
        );

        let agent = &stats.tool_stats[0];
        assert_eq!(agent.calls, 2);
        assert_eq!(agent.successes, 0);
        assert_eq!(agent.unknowns, 1);
        assert_eq!(agent.evaluated(), 1);
    }

    #[tokio::test]
    async fn delete_old_observations() {
        let pool = test_pool().await;

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
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "20260501_12345", "test-project")
            .await
            .unwrap();

        let deleted = delete_obs_older_than_pool(&pool, "2026-05-15")
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let results = query_obs_for_date_range_pool(&pool, "2026-05-01", "2026-05-31", None)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn retention_preserves_the_ending_session() {
        let pool = test_pool().await;
        let rec = ObsRecord {
            timestamp: "2026-05-01T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: None,
            result: Some("success".into()),
            score: Some(1.0),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "ending", "project-a")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "old", "project-a")
            .await
            .unwrap();

        let deleted =
            delete_obs_older_than_except_session_pool(&pool, "2026-05-15", "ending", "project-a")
                .await
                .unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(
            query_obs_for_session_pool(&pool, "ending", "project-a", 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn retention_preserves_all_queued_long_running_sessions() {
        let pool = test_pool().await;
        let rec = ObsRecord {
            timestamp: "2026-05-01T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: None,
            result: Some("success".into()),
            score: Some(1.0),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
            tool_use_id: None,
        };
        insert_observation_pool(&pool, &rec, "ending", "project-a")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "long-running", "project-b")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec, "expired", "project-c")
            .await
            .unwrap();

        let deleted = delete_obs_older_than_except_sessions_pool(
            &pool,
            "2026-05-15",
            &[
                ("ending".into(), "project-a".into()),
                ("long-running".into(), "project-b".into()),
            ],
        )
        .await
        .unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(
            query_obs_for_session_pool(&pool, "ending", "project-a", 10)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            query_obs_for_session_pool(&pool, "long-running", "project-b", 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn query_last_action() {
        let pool = test_pool().await;

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
            tool_use_id: None,
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
            tool_use_id: None,
        };

        insert_observation_pool(&pool, &rec1, "sess1", "test-project")
            .await
            .unwrap();
        insert_observation_pool(&pool, &rec2, "sess1", "test-project")
            .await
            .unwrap();

        let last = query_last_action_pool(&pool, "sess1").await.unwrap();
        assert_eq!(last, Some("second edit".to_string()));
    }
}
