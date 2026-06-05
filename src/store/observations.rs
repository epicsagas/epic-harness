//! observations.rs — Observation records SQLite I/O (async pool)

use sqlx::{Row, SqlitePool};
use std::io;

use crate::shared::obs::ObsRecord;
use crate::shared::scoring::ScoreDimensions;

/// Pad an ISO-8601 date string for lexicographic range comparison.
/// `"2026-06-02"` → `"2026-06-02T00:00:00Z"` / `"...T23:59:59Z"`.
///
/// Intentional choice: `T23:59:59Z` (not `T23:59:59.999Z`) is used as the upper sentinel
/// because `'Z'` (0x5A) > `'.'` (0x2E) and `'+'` (0x2B) in ASCII order. This means any
/// fractional-second or offset variant — `T23:59:59.999Z`, `T23:59:59+00:00` — compares
/// lexicographically *less than* `T23:59:59Z`, so all same-day timestamps are correctly
/// included in `<= upper` without needing to enumerate fractional-second forms.
fn pad_date(ts: &str, end_of_day: bool) -> String {
    if ts.len() == 10 {
        if end_of_day {
            format!("{ts}T23:59:59Z")
        } else {
            format!("{ts}T00:00:00Z")
        }
    } else {
        ts.to_string()
    }
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

// ── Async pool functions ─────────────────────────────

/// Map an sqlx observation row to ObsRecord.
fn row_to_obs_record(r: &sqlx::sqlite::SqliteRow) -> io::Result<ObsRecord> {
    let dim_s: Option<f64> = r.try_get(6).map_err(crate::store::sqlx_err)?;
    let dim_q: Option<f64> = r.try_get(7).map_err(crate::store::sqlx_err)?;
    let dim_c: Option<f64> = r.try_get(8).map_err(crate::store::sqlx_err)?;
    Ok(ObsRecord {
        timestamp: r.try_get(0).map_err(crate::store::sqlx_err)?,
        tool: r.try_get(1).map_err(crate::store::sqlx_err)?,
        tool_category: r.try_get(2).map_err(crate::store::sqlx_err)?,
        action: r.try_get(3).map_err(crate::store::sqlx_err)?,
        result: r.try_get(4).map_err(crate::store::sqlx_err)?,
        score: r.try_get(5).map_err(crate::store::sqlx_err)?,
        dimensions: {
            let any_some = dim_s.is_some() || dim_q.is_some() || dim_c.is_some();
            let all_some = dim_s.is_some() && dim_q.is_some() && dim_c.is_some();
            if any_some && !all_some {
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
        failure_category: r.try_get(9).map_err(crate::store::sqlx_err)?,
        error_snippet: r.try_get(10).map_err(crate::store::sqlx_err)?,
        file_ext: r.try_get(11).map_err(crate::store::sqlx_err)?,
        sequence_id: r
            .try_get::<Option<i64>, _>(12)
            .ok()
            .flatten()
            .map(super::i64_to_u64),
        pipeline_id: r.try_get(13).map_err(crate::store::sqlx_err)?,
    })
}

/// Async insert observation using pool.
pub async fn insert_observation_pool(
    pool: &SqlitePool,
    project: &str,
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
    let result = sqlx::query(
        "INSERT INTO observations
         (timestamp, session_id, tool, tool_category, action, result, score,
          dim_success, dim_quality, dim_cost, failure_category, error_snippet,
          file_ext, sequence_id, pipeline_id, project)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(project)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result.last_insert_rowid())
}

/// Async query observations for a date range.
pub async fn query_obs_for_date_range_pool(
    pool: &SqlitePool,
    project: &str,
    from_ts: &str,
    to_ts: &str,
    limit: Option<usize>,
) -> io::Result<Vec<ObsRecord>> {
    let from = pad_date(from_ts, false);
    let to = pad_date(to_ts, true);
    let limit_val = limit.map(|l| l.min(50_000) as i64).unwrap_or(-1);

    let rows = sqlx::query(
        "SELECT timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost,
                failure_category, error_snippet, file_ext, sequence_id, pipeline_id
         FROM observations
         WHERE project = ? AND timestamp >= ? AND timestamp <= ?
         ORDER BY timestamp ASC
         LIMIT ?",
    )
    .bind(project)
    .bind(&from)
    .bind(&to)
    .bind(limit_val)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    rows.iter().map(row_to_obs_record).collect()
}

/// Async query observations for a date range, filtering by multiple projects.
pub async fn query_obs_for_date_range_multi_pool(
    pool: &SqlitePool,
    projects: &[String],
    from_ts: &str,
    to_ts: &str,
    limit: Option<usize>,
) -> io::Result<Vec<ObsRecord>> {
    if projects.is_empty() {
        return Ok(vec![]);
    }
    let from = pad_date(from_ts, false);
    let to = pad_date(to_ts, true);
    let limit_val = limit.map(|l| l.min(50_000) as i64).unwrap_or(-1);

    let placeholders: Vec<&str> = projects.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost,
                failure_category, error_snippet, file_ext, sequence_id, pipeline_id
         FROM observations
         WHERE project IN ({}) AND timestamp >= ? AND timestamp <= ?
         ORDER BY timestamp ASC
         LIMIT ?",
        placeholders.join(",")
    );
    let mut q = sqlx::query(&sql);
    for p in projects {
        q = q.bind(p);
    }
    q = q.bind(&from).bind(&to).bind(limit_val);

    let rows = q.fetch_all(pool).await.map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_obs_record).collect()
}

/// Async aggregate observation stats.
pub async fn query_obs_stats_pool(
    pool: &SqlitePool,
    project: &str,
    from_ts: &str,
    to_ts: &str,
) -> io::Result<ObsStats> {
    let from = pad_date(from_ts, false);
    let to = pad_date(to_ts, true);

    // Overall stats
    let row = sqlx::query(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END), 0),
                COALESCE(AVG(score), 0.0)
         FROM observations
         WHERE project = ? AND timestamp >= ? AND timestamp <= ?",
    )
    .bind(project)
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let total: i64 = row.try_get(0).map_err(crate::store::sqlx_err)?;
    let successes: i64 = row.try_get(1).map_err(crate::store::sqlx_err)?;
    let avg_score: f64 = row.try_get(2).map_err(crate::store::sqlx_err)?;

    // Per-tool stats
    let tool_rows = sqlx::query(
        "SELECT tool, COUNT(*) as calls,
                SUM(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as successes,
                COALESCE(AVG(score), 0.0) as avg_score
         FROM observations
         WHERE project = ? AND timestamp >= ? AND timestamp <= ?
         GROUP BY tool ORDER BY calls DESC LIMIT 100",
    )
    .bind(project)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let tool_stats: Vec<ToolStatRow> = tool_rows
        .iter()
        .map(|r| ToolStatRow {
            tool: r.try_get(0).unwrap_or_default(),
            calls: r.try_get(1).unwrap_or(0),
            successes: r.try_get(2).unwrap_or(0),
            avg_score: r.try_get(3).unwrap_or(0.0),
        })
        .collect();

    // Per-error stats
    let err_rows = sqlx::query(
        "SELECT failure_category, COUNT(*) as cnt
         FROM observations
         WHERE project = ? AND timestamp >= ? AND timestamp <= ?
           AND failure_category IS NOT NULL
         GROUP BY failure_category ORDER BY cnt DESC LIMIT 50",
    )
    .bind(project)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let error_stats: Vec<(String, i64)> = err_rows
        .iter()
        .filter_map(|r| {
            let cat: String = r.try_get(0).ok()?;
            let cnt: i64 = r.try_get(1).ok()?;
            Some((cat, cnt))
        })
        .collect();

    // Per-session stats
    let sess_rows = sqlx::query(
        "SELECT session_id, COUNT(*) as calls,
                COALESCE(AVG(score), 0.0) as avg_score,
                SUM(CASE WHEN result != 'success' AND result IS NOT NULL THEN 1 ELSE 0 END) as failures
         FROM observations
         WHERE project = ? AND timestamp >= ? AND timestamp <= ?
         GROUP BY session_id ORDER BY session_id DESC LIMIT 20",
    )
    .bind(project)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let session_stats: Vec<SessionStatRow> = sess_rows
        .iter()
        .map(|r| SessionStatRow {
            session_id: r.try_get(0).unwrap_or_default(),
            calls: r.try_get(1).unwrap_or(0),
            avg_score: r.try_get(2).unwrap_or(0.0),
            failures: r.try_get(3).unwrap_or(0),
        })
        .collect();

    Ok(ObsStats {
        total,
        successes,
        avg_score,
        tool_stats,
        error_stats,
        session_stats,
    })
}

/// Async query latest observations.
#[cfg(test)]
pub async fn query_latest_observations_pool(
    pool: &SqlitePool,
    project: &str,
    limit: i64,
) -> io::Result<Vec<ObsRecord>> {
    let rows = sqlx::query(
        "SELECT timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost,
                failure_category, error_snippet, file_ext, sequence_id, pipeline_id
         FROM observations WHERE project = ? ORDER BY id DESC LIMIT ?",
    )
    .bind(project)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    rows.iter().map(row_to_obs_record).collect()
}

/// Async query last action for a session.
pub async fn query_last_action_pool(
    pool: &SqlitePool,
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
    .map_err(crate::store::sqlx_err)?;
    Ok(row.and_then(|r| r.try_get::<String, _>(0).ok()))
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
    async fn insert_and_query_observation() {
        let pool = in_memory_pool().await;
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

        let id = insert_observation_pool(&pool, "test-project", &rec, "20260602_12345")
            .await
            .unwrap();
        assert!(id > 0);

        let results =
            query_obs_for_date_range_pool(&pool, "test-project", "2026-06-02", "2026-06-02", None)
                .await
                .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, "Bash");
        assert_eq!(results[0].score, Some(0.95));
    }

    #[tokio::test]
    async fn query_stats_empty() {
        let pool = in_memory_pool().await;
        let stats = query_obs_stats_pool(&pool, "test-project", "2026-06-01", "2026-06-30")
            .await
            .unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.successes, 0);
    }

    #[tokio::test]
    async fn query_stats_with_data() {
        let pool = in_memory_pool().await;

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
            insert_observation_pool(&pool, "test-project", &rec, "20260602_12345")
                .await
                .unwrap();
        }

        let stats = query_obs_stats_pool(&pool, "test-project", "2026-06-02", "2026-06-02")
            .await
            .unwrap();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.tool_stats.len(), 1);
        assert_eq!(stats.tool_stats[0].tool, "Edit");
        assert_eq!(stats.error_stats.len(), 1);
        assert_eq!(stats.error_stats[0].0, "syntax_error");
    }

    #[tokio::test]
    async fn old_observations_not_in_range_query() {
        let pool = in_memory_pool().await;

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
        insert_observation_pool(&pool, "test-project", &rec, "20260501_12345")
            .await
            .unwrap();

        // Old record is outside the June query window
        let results =
            query_obs_for_date_range_pool(&pool, "test-project", "2026-06-01", "2026-06-30", None)
                .await
                .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn query_last_action() {
        let pool = in_memory_pool().await;

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

        insert_observation_pool(&pool, "test-project", &rec1, "sess1")
            .await
            .unwrap();
        insert_observation_pool(&pool, "test-project", &rec2, "sess1")
            .await
            .unwrap();

        let last = query_last_action_pool(&pool, "sess1").await.unwrap();
        assert_eq!(last, Some("second edit".to_string()));
    }

    #[tokio::test]
    async fn multi_project_query_returns_matching_projects() {
        let pool = in_memory_pool().await;

        let rec_a = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("test a".into()),
            result: Some("success".into()),
            score: Some(0.9),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };
        let rec_b = ObsRecord {
            timestamp: "2026-06-02T11:00:00Z".into(),
            tool: "Edit".into(),
            tool_category: "edit".into(),
            action: Some("test b".into()),
            result: Some("success".into()),
            score: Some(0.8),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };
        let rec_c = ObsRecord {
            timestamp: "2026-06-02T12:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: Some("test c".into()),
            result: Some("success".into()),
            score: Some(0.7),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        };

        insert_observation_pool(&pool, "proj-a", &rec_a, "sess_a")
            .await
            .unwrap();
        insert_observation_pool(&pool, "proj-b", &rec_b, "sess_b")
            .await
            .unwrap();
        insert_observation_pool(&pool, "proj-c", &rec_c, "sess_c")
            .await
            .unwrap();

        // Query for proj-a and proj-b only
        let projects = vec!["proj-a".to_string(), "proj-b".to_string()];
        let results =
            query_obs_for_date_range_multi_pool(&pool, &projects, "2026-06-02", "2026-06-02", None)
                .await
                .unwrap();

        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|r| r.tool != "Bash" || r.score == Some(0.9) || r.tool == "Edit")
        );
        // Verify proj-c is excluded
        assert!(
            results
                .iter()
                .all(|r| r.action.as_deref() != Some("test c"))
        );
    }

    #[tokio::test]
    async fn multi_project_query_empty_projects() {
        let pool = in_memory_pool().await;
        let results =
            query_obs_for_date_range_multi_pool(&pool, &[], "2026-06-02", "2026-06-02", None)
                .await
                .unwrap();
        assert!(results.is_empty());
    }
}
