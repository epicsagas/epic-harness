//! observations.rs — Observation records SQLite I/O

#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

use crate::shared::obs::ObsRecord;
use crate::shared::scoring::ScoreDimensions;

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

/// Query observations for a date range (inclusive).
pub async fn query_obs_for_date_range_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
) -> io::Result<Vec<ObsRecord>> {
    let from = if from_ts.len() == 10 {
        format!("{}T00:00:00", from_ts)
    } else {
        from_ts.to_string()
    };
    let to = if to_ts.len() == 10 {
        format!("{}T23:59:59", to_ts)
    } else {
        to_ts.to_string()
    };

    let rows = sqlx::query(
        "SELECT timestamp, tool, tool_category, action, result, score,
                dim_success, dim_quality, dim_cost,
                failure_category, error_snippet, file_ext, sequence_id, pipeline_id
         FROM observations
         WHERE timestamp >= ? AND timestamp <= ?
         ORDER BY timestamp ASC",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
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
        });
    }
    Ok(records)
}

/// Aggregate observation stats via SQL.
pub async fn query_obs_stats_pool(
    pool: &AnyPool,
    from_ts: &str,
    to_ts: &str,
) -> io::Result<ObsStats> {
    let from = if from_ts.len() == 10 {
        format!("{}T00:00:00", from_ts)
    } else {
        from_ts.to_string()
    };
    let to = if to_ts.len() == 10 {
        format!("{}T23:59:59", to_ts)
    } else {
        to_ts.to_string()
    };

    // Overall stats
    let row = sqlx::query(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END), 0),
                COALESCE(AVG(score), 0.0)
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

    // Per-tool stats
    let tool_rows = sqlx::query(
        "SELECT tool, COUNT(*) as calls,
                SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END) as successes,
                COALESCE(AVG(score), 0.0) as avg_score
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

    // Per-session stats
    let sess_rows = sqlx::query(
        "SELECT session_id, COUNT(*) as calls,
                COALESCE(AVG(score), 0.0) as avg_score,
                SUM(CASE WHEN result != 'success' AND result IS NOT NULL THEN 1 ELSE 0 END) as failures
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
    let from = if from_ts.len() == 10 {
        format!("{}T00:00:00", from_ts)
    } else {
        from_ts.to_string()
    };
    let to = if to_ts.len() == 10 {
        format!("{}T23:59:59", to_ts)
    } else {
        to_ts.to_string()
    };

    // Overall stats — two static-SQL branches so sqlx 0.9's SqlSafeStr guard
    // (which rejects dynamic strings to prevent injection) is satisfied.
    let (total, successes, avg_score): (i64, i64, f64) = if let Some(p) = project {
        let row = sqlx::query(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(score), 0.0)
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
        )
    } else {
        let row = sqlx::query(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(score), 0.0)
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
        )
    };

    // Per-tool stats
    let tool_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT tool, COUNT(*) as calls,
                    SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END) as successes,
                    COALESCE(AVG(score), 0.0) as avg_score
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
                    SUM(CASE WHEN result = 'success' OR result IS NULL THEN 1 ELSE 0 END) as successes,
                    COALESCE(AVG(score), 0.0) as avg_score
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
                    SUM(CASE WHEN result != 'success' AND result IS NOT NULL THEN 1 ELSE 0 END) as failures
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
                    SUM(CASE WHEN result != 'success' AND result IS NOT NULL THEN 1 ELSE 0 END) as failures
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
        };

        let id = insert_observation_pool(&pool, &rec, "20260602_12345", "test-project")
            .await
            .unwrap();
        assert!(id > 0);

        let results = query_obs_for_date_range_pool(&pool, "2026-06-02", "2026-06-02")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool, "Bash");
        assert_eq!(results[0].score, Some(0.95));
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
        };
        insert_observation_pool(&pool, &rec, "20260501_12345", "test-project")
            .await
            .unwrap();

        let deleted = delete_obs_older_than_pool(&pool, "2026-05-15")
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let results = query_obs_for_date_range_pool(&pool, "2026-05-01", "2026-05-31")
            .await
            .unwrap();
        assert!(results.is_empty());
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
