//! metrics.rs — Metrics state SQLite I/O (3-table normalized)
#![allow(dead_code)]
//!
//! Replaces the single `metrics.json` file with:
//! - `metrics_state` — key-value scalar fields
//! - `score_history` — session score entries (capped at 50)
//! - `skill_attribution` — per-skill A/B scores

use sqlx::AnyPool;
use sqlx::Row;
use std::collections::HashMap;
use std::io;

use crate::shared::evolution::{Metrics, SessionScoreEntry, SkillAttribution};
use crate::shared::scoring::ScoreDimensions;

/// Maximum score history entries to retain.
const MAX_SCORE_HISTORY: usize = 50;
/// Bounded cross-project views protect dashboard and reflection requests from
/// materializing the sum of every project's retained history.
const MAX_AGGREGATE_SCORE_HISTORY: i64 = 100;
const MAX_AGGREGATE_SKILL_ATTRIBUTIONS: i64 = 500;

// ── Load ─────────────────────────────────────────────

/// Load the full Metrics struct from SQLite.
pub async fn load_metrics_pool(pool: &AnyPool) -> io::Result<Metrics> {
    // Scalar state helper
    async fn get(pool: &AnyPool, key: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT value FROM metrics_state WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
    }

    let total_sessions: u64 = get(pool, "total_sessions")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let avg_success_rate: f64 = get(pool, "avg_success_rate")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let total_evolved_skills: u64 = get(pool, "total_evolved_skills")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let last_session = get(pool, "last_session").await.filter(|v| !v.is_empty());
    let best_score: Option<f64> = get(pool, "best_score").await.and_then(|v| v.parse().ok());
    let best_session = get(pool, "best_session").await.unwrap_or_default();
    let trend = get(pool, "trend").await.unwrap_or_else(|| "stable".into());
    let stagnation_count: u64 = get(pool, "stagnation_count")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let last_error_context = get(pool, "last_error_context")
        .await
        .filter(|v| !v.is_empty());
    let reward_hacking_suspected: bool = get(pool, "reward_hacking_suspected")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);
    let epoch_class: Option<crate::shared::evolution::EpochClass> = get(pool, "epoch_class")
        .await
        .filter(|v| !v.is_empty())
        .and_then(|v| serde_json::from_str::<crate::shared::evolution::EpochClass>(&v).ok());

    // Score history
    let sh_rows = sqlx::query(
        "SELECT timestamp, success_rate, avg_score, observations,
                dim_success, dim_quality, dim_cost
         FROM score_history ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let score_history: Vec<SessionScoreEntry> = sh_rows
        .iter()
        .filter_map(|r| {
            Some(SessionScoreEntry {
                timestamp: r.try_get(0).ok()?,
                success_rate: r.try_get(1).ok()?,
                avg_score: r.try_get(2).ok()?,
                observations: r.try_get::<i64, _>(3).ok()? as u64,
                dimension_averages: ScoreDimensions {
                    tool_success: r.try_get(4).ok()?,
                    output_quality: r.try_get(5).ok()?,
                    execution_cost: r.try_get(6).ok()?,
                },
            })
        })
        .collect();

    // Skill attribution
    let sa_rows = sqlx::query(
        "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, sessions_holdout
         FROM skill_attribution",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let skill_attribution: HashMap<String, SkillAttribution> = sa_rows
        .iter()
        .filter_map(|r| {
            let sa = SkillAttribution {
                skill_name: r.try_get(0).ok()?,
                sessions_active: r.try_get::<i64, _>(1).ok()? as u64,
                avg_score_with: r.try_get(2).ok()?,
                avg_score_without: r.try_get(3).ok()?,
                first_seen: r.try_get(4).ok()?,
                sessions_holdout: r.try_get::<i64, _>(5).ok()? as u64,
            };
            Some((sa.skill_name.clone(), sa))
        })
        .collect();

    Ok(Metrics {
        total_sessions,
        avg_success_rate,
        total_evolved_skills,
        last_session,
        score_history,
        best_score,
        best_session,
        trend,
        stagnation_count,
        skill_attribution,
        epoch_class,
        last_error_context,
        reward_hacking_suspected,
    })
}

/// Standalone load.
pub fn load_metrics() -> io::Result<Metrics> {
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        load_metrics_pool(&pool).await
    })
}

// ── Save ─────────────────────────────────────────────

/// Save the full Metrics struct to SQLite, scoped to `project`.
pub async fn save_metrics_pool(pool: &AnyPool, m: &Metrics, project: &str) -> io::Result<()> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    // Scalar state — upsert each key scoped to (key, project).
    async fn upsert(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        key: &str,
        value: &str,
        project: &str,
    ) -> io::Result<()> {
        sqlx::query(
            "INSERT INTO metrics_state (key, value, project) VALUES (?, ?, ?) \
             ON CONFLICT (key, project) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .bind(project)
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;
        Ok(())
    }

    upsert(
        &mut tx,
        "total_sessions",
        &m.total_sessions.to_string(),
        project,
    )
    .await?;
    upsert(
        &mut tx,
        "avg_success_rate",
        &m.avg_success_rate.to_string(),
        project,
    )
    .await?;
    upsert(
        &mut tx,
        "total_evolved_skills",
        &m.total_evolved_skills.to_string(),
        project,
    )
    .await?;
    if let Some(ref v) = m.last_session {
        upsert(&mut tx, "last_session", v, project).await?;
    }
    if let Some(v) = m.best_score {
        upsert(&mut tx, "best_score", &v.to_string(), project).await?;
    }
    upsert(&mut tx, "best_session", &m.best_session, project).await?;
    upsert(&mut tx, "trend", &m.trend, project).await?;
    upsert(
        &mut tx,
        "stagnation_count",
        &m.stagnation_count.to_string(),
        project,
    )
    .await?;
    if let Some(ref v) = m.last_error_context {
        upsert(&mut tx, "last_error_context", v, project).await?;
    }
    upsert(
        &mut tx,
        "reward_hacking_suspected",
        &m.reward_hacking_suspected.to_string(),
        project,
    )
    .await?;
    if let Some(ref epoch) = m.epoch_class
        && let Ok(s) = serde_json::to_string(epoch)
    {
        upsert(&mut tx, "epoch_class", &s, project).await?;
    }

    // Score history — project-scoped: delete only this project's rows, then
    // re-insert the most recent MAX_SCORE_HISTORY entries.
    sqlx::query("DELETE FROM score_history WHERE project = ?")
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    for entry in entries.into_iter().rev() {
        sqlx::query(
            "INSERT INTO score_history (timestamp, success_rate, avg_score, observations,
             dim_success, dim_quality, dim_cost, project) VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&entry.timestamp)
        .bind(entry.success_rate)
        .bind(entry.avg_score)
        .bind(super::u64_to_i64(entry.observations))
        .bind(entry.dimension_averages.tool_success)
        .bind(entry.dimension_averages.output_quality)
        .bind(entry.dimension_averages.execution_cost)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    // Skill attribution — scoped to (skill_name, project).
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT INTO skill_attribution
             (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, project, sessions_holdout)
             VALUES (?,?,?,?,?,?,?)
             ON CONFLICT (skill_name, project) DO UPDATE SET
                sessions_active = excluded.sessions_active,
                avg_score_with = excluded.avg_score_with,
                avg_score_without = excluded.avg_score_without,
                first_seen = excluded.first_seen,
                sessions_holdout = excluded.sessions_holdout",
        )
        .bind(&sa.skill_name)
        .bind(super::u64_to_i64(sa.sessions_active))
        .bind(sa.avg_score_with)
        .bind(sa.avg_score_without)
        .bind(&sa.first_seen)
        .bind(project)
        .bind(super::u64_to_i64(sa.sessions_holdout))
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(())
}

/// Atomically apply one reflection's metrics state. The marker and the full
/// metrics update share a transaction, so retries observe either neither or
/// the completed application — never a duplicate increment.
pub async fn save_metrics_once_pool(
    pool: &AnyPool,
    m: &Metrics,
    project: &str,
    session_id: &str,
) -> io::Result<bool> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;
    let marker = sqlx::query(
        "INSERT INTO reflection_metrics (session_id, project) VALUES (?, ?)
         ON CONFLICT (session_id, project) DO NOTHING",
    )
    .bind(session_id)
    .bind(project)
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;
    if marker.rows_affected() == 0 {
        tx.commit().await.map_err(super::sqlx_err)?;
        return Ok(false);
    }
    save_metrics_direct(&mut tx, m, project).await?;
    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(true)
}

/// Standalone save.
pub fn save_metrics(m: &Metrics) -> io::Result<()> {
    let project = crate::shared::paths::project_slug().to_string();
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        save_metrics_pool(&pool, m, &project).await
    })
}

/// Save metrics directly into an existing transaction (no inner BEGIN/COMMIT).
/// Used by `migrate.rs` which already holds a transaction.
pub async fn save_metrics_direct(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    m: &Metrics,
    project: &str,
) -> io::Result<()> {
    async fn upsert(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        key: &str,
        value: &str,
        project: &str,
    ) -> io::Result<()> {
        sqlx::query(
            "INSERT INTO metrics_state (key, value, project) VALUES (?, ?, ?) \
             ON CONFLICT (key, project) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .bind(project)
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;
        Ok(())
    }

    upsert(tx, "total_sessions", &m.total_sessions.to_string(), project).await?;
    upsert(
        tx,
        "avg_success_rate",
        &m.avg_success_rate.to_string(),
        project,
    )
    .await?;
    upsert(
        tx,
        "total_evolved_skills",
        &m.total_evolved_skills.to_string(),
        project,
    )
    .await?;
    if let Some(ref v) = m.last_session {
        upsert(tx, "last_session", v, project).await?;
    }
    if let Some(v) = m.best_score {
        upsert(tx, "best_score", &v.to_string(), project).await?;
    }
    upsert(tx, "best_session", &m.best_session, project).await?;
    upsert(tx, "trend", &m.trend, project).await?;
    upsert(
        tx,
        "stagnation_count",
        &m.stagnation_count.to_string(),
        project,
    )
    .await?;
    if let Some(ref v) = m.last_error_context {
        upsert(tx, "last_error_context", v, project).await?;
    }
    upsert(
        tx,
        "reward_hacking_suspected",
        &m.reward_hacking_suspected.to_string(),
        project,
    )
    .await?;
    if let Some(ref epoch) = m.epoch_class
        && let Ok(s) = serde_json::to_string(epoch)
    {
        upsert(tx, "epoch_class", &s, project).await?;
    }

    // Score history — project-scoped.
    sqlx::query("DELETE FROM score_history WHERE project = ?")
        .bind(project)
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;

    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    for entry in entries.into_iter().rev() {
        sqlx::query(
            "INSERT INTO score_history (timestamp, success_rate, avg_score, observations,
             dim_success, dim_quality, dim_cost, project) VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&entry.timestamp)
        .bind(entry.success_rate)
        .bind(entry.avg_score)
        .bind(super::u64_to_i64(entry.observations))
        .bind(entry.dimension_averages.tool_success)
        .bind(entry.dimension_averages.output_quality)
        .bind(entry.dimension_averages.execution_cost)
        .bind(project)
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    // Skill attribution — scoped to (skill_name, project).
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT INTO skill_attribution
             (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, project, sessions_holdout)
             VALUES (?,?,?,?,?,?,?)
             ON CONFLICT (skill_name, project) DO UPDATE SET
                sessions_active = excluded.sessions_active,
                avg_score_with = excluded.avg_score_with,
                avg_score_without = excluded.avg_score_without,
                first_seen = excluded.first_seen,
                sessions_holdout = excluded.sessions_holdout",
        )
        .bind(&sa.skill_name)
        .bind(super::u64_to_i64(sa.sessions_active))
        .bind(sa.avg_score_with)
        .bind(sa.avg_score_without)
        .bind(&sa.first_seen)
        .bind(project)
        .bind(super::u64_to_i64(sa.sessions_holdout))
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    // No commit — caller manages the transaction
    Ok(())
}

/// Alias: load metrics without project filter (cross-project aggregate view).
pub async fn load_metrics_all_pool(pool: &AnyPool) -> io::Result<Metrics> {
    load_metrics_pool(pool).await
}

/// Load metrics for a specific project slug, or aggregated across all
/// projects when `project` is `None`.
///
/// The `metrics_state` table is keyed by `(key, project)`, so the unfiltered
/// `load_metrics_pool` returns indeterminate results when multiple projects
/// have written rows (it used `fetch_optional` on a multi-row result). This
/// variant scopes every query to the requested project, or aggregates (SUM /
/// weighted avg) across all projects when `None`.
pub async fn load_metrics_scoped_pool(
    pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<Metrics> {
    // Scalar state — aggregate across projects.
    async fn get_sum(pool: &AnyPool, key: &str, project: Option<&str>) -> Option<f64> {
        let q = if project.is_some() {
            "SELECT value FROM metrics_state WHERE key = ? AND project = ? LIMIT 1"
        } else {
            "SELECT value FROM metrics_state WHERE key = ? LIMIT 1"
        };
        let row = if let Some(p) = project {
            sqlx::query_scalar::<_, String>(q).bind(key).bind(p)
        } else {
            sqlx::query_scalar::<_, String>(q).bind(key)
        }
        .fetch_optional(pool)
        .await
        .ok()??;
        row.parse().ok()
    }
    async fn get_str(pool: &AnyPool, key: &str, project: Option<&str>) -> Option<String> {
        let row = if let Some(p) = project {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM metrics_state WHERE key = ? AND project = ? LIMIT 1",
            )
            .bind(key)
            .bind(p)
            .fetch_optional(pool)
            .await
        } else {
            sqlx::query_scalar::<_, String>("SELECT value FROM metrics_state WHERE key = ? LIMIT 1")
                .bind(key)
                .fetch_optional(pool)
                .await
        };
        row.ok().flatten()
    }

    // Aggregate scalar metrics across projects for the "all" view, or take
    // the single project row otherwise.
    let (total_sessions, avg_success_rate, total_evolved_skills, stagnation_count): (
        u64,
        f64,
        u64,
        u64,
    ) = if let Some(p) = project {
        (
            get_sum(pool, "total_sessions", Some(p))
                .await
                .unwrap_or(0.0) as u64,
            get_sum(pool, "avg_success_rate", Some(p))
                .await
                .unwrap_or(0.0),
            get_sum(pool, "total_evolved_skills", Some(p))
                .await
                .unwrap_or(0.0) as u64,
            get_sum(pool, "stagnation_count", Some(p))
                .await
                .unwrap_or(0.0) as u64,
        )
    } else {
        // Sum counts across every project row. SQLite SUM over REAL columns
        // returns a REAL, so decode as f64 (i64 decode silently yields None).
        let ts: f64 = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT SUM(CAST(value AS REAL)) FROM metrics_state WHERE key = 'total_sessions'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .unwrap_or(0.0);
        let tes: f64 = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT SUM(CAST(value AS REAL)) FROM metrics_state WHERE key = 'total_evolved_skills'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .unwrap_or(0.0);
        let st: u64 = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT SUM(CAST(value AS REAL)) FROM metrics_state WHERE key = 'stagnation_count'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .unwrap_or(0.0) as u64;
        // Weighted-average success rate by per-project session count.
        let avg = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT CASE WHEN SUM(ts) > 0 THEN SUM(asr * ts) / SUM(ts) ELSE 0 END
             FROM (SELECT CAST(value AS REAL) AS asr,
                          (SELECT CAST(value AS REAL) FROM metrics_state m2
                           WHERE m2.key='total_sessions' AND m2.project = m1.project) AS ts
                   FROM metrics_state m1 WHERE m1.key = 'avg_success_rate')",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .unwrap_or(0.0);
        (ts as u64, avg, tes as u64, st)
    };

    let last_session = if let Some(p) = project {
        get_str(pool, "last_session", Some(p))
            .await
            .filter(|v| !v.is_empty())
    } else {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT MAX(value) FROM metrics_state WHERE key = 'last_session'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .filter(|v| !v.is_empty())
    };
    let best_score: Option<f64> = if let Some(p) = project {
        get_sum(pool, "best_score", Some(p)).await
    } else {
        sqlx::query_scalar::<_, Option<f64>>(
            "SELECT MAX(CAST(value AS REAL)) FROM metrics_state WHERE key = 'best_score'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
    };
    let best_session = if let Some(p) = project {
        get_str(pool, "best_session", Some(p))
            .await
            .unwrap_or_default()
    } else {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT value FROM metrics_state WHERE key = 'best_session'
             ORDER BY CAST((SELECT value FROM metrics_state m2 WHERE m2.key='best_score' AND m2.project=metrics_state.project) AS REAL) DESC LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .unwrap_or_default()
    };
    let trend = if let Some(p) = project {
        get_str(pool, "trend", Some(p))
            .await
            .unwrap_or_else(|| "stable".into())
    } else {
        "stable".to_string()
    };
    let last_error_context = if let Some(p) = project {
        get_str(pool, "last_error_context", Some(p))
            .await
            .filter(|v| !v.is_empty())
    } else {
        None
    };
    let reward_hacking_suspected: bool = if let Some(p) = project {
        get_sum(pool, "reward_hacking_suspected", Some(p))
            .await
            .map(|v| v != 0.0)
            .unwrap_or(false)
    } else {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT CASE WHEN MAX(CAST(value AS REAL)) > 0 THEN 1 ELSE 0 END
             FROM metrics_state WHERE key = 'reward_hacking_suspected'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(None)
        .map(|v| v != 0)
        .unwrap_or(false)
    };
    let epoch_class: Option<crate::shared::evolution::EpochClass> = if let Some(p) = project {
        get_str(pool, "epoch_class", Some(p))
            .await
            .filter(|v| !v.is_empty())
            .and_then(|v| serde_json::from_str::<crate::shared::evolution::EpochClass>(&v).ok())
    } else {
        None
    };

    // Score history — filter by project, or take a bounded cross-project tail.
    let sh_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT timestamp, success_rate, avg_score, observations,
                    dim_success, dim_quality, dim_cost
             FROM score_history WHERE project = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(p)
        .bind(MAX_SCORE_HISTORY as i64)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT timestamp, success_rate, avg_score, observations,
                    dim_success, dim_quality, dim_cost
             FROM score_history ORDER BY id DESC LIMIT ?",
        )
        .bind(MAX_AGGREGATE_SCORE_HISTORY)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;

    let mut score_history: Vec<SessionScoreEntry> = sh_rows
        .iter()
        .filter_map(|r| {
            Some(SessionScoreEntry {
                timestamp: r.try_get(0).ok()?,
                success_rate: r.try_get(1).ok()?,
                avg_score: r.try_get(2).ok()?,
                observations: r.try_get::<i64, _>(3).ok()? as u64,
                dimension_averages: ScoreDimensions {
                    tool_success: r.try_get(4).ok()?,
                    output_quality: r.try_get(5).ok()?,
                    execution_cost: r.try_get(6).ok()?,
                },
            })
        })
        .collect();
    score_history.reverse();

    // Skill attribution — scoped by project (Some), or all (None).
    let sa_rows = if let Some(p) = project {
        sqlx::query(
            "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, sessions_holdout
             FROM skill_attribution WHERE project = ?",
        )
        .bind(p)
    } else {
        sqlx::query(
            "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, sessions_holdout
             FROM skill_attribution
             ORDER BY sessions_active DESC, first_seen DESC LIMIT ?",
        )
        .bind(MAX_AGGREGATE_SKILL_ATTRIBUTIONS)
    }
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let skill_attribution: HashMap<String, SkillAttribution> = sa_rows
        .iter()
        .filter_map(|r| {
            let sa = SkillAttribution {
                skill_name: r.try_get(0).ok()?,
                sessions_active: r.try_get::<i64, _>(1).ok()? as u64,
                avg_score_with: r.try_get(2).ok()?,
                avg_score_without: r.try_get(3).ok()?,
                first_seen: r.try_get(4).ok()?,
                sessions_holdout: r.try_get::<i64, _>(5).ok()? as u64,
            };
            Some((sa.skill_name.clone(), sa))
        })
        .collect();

    Ok(Metrics {
        total_sessions,
        avg_success_rate,
        total_evolved_skills,
        last_session,
        score_history,
        best_score,
        best_session,
        trend,
        stagnation_count,
        skill_attribution,
        epoch_class,
        last_error_context,
        reward_hacking_suspected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    fn sample_metrics() -> Metrics {
        Metrics {
            total_sessions: 10,
            avg_success_rate: 0.92,
            total_evolved_skills: 3,
            last_session: Some("2026-06-02".into()),
            score_history: vec![SessionScoreEntry {
                timestamp: "2026-06-02T10:00:00Z".into(),
                success_rate: 0.95,
                avg_score: 0.89,
                observations: 42,
                dimension_averages: ScoreDimensions {
                    tool_success: 1.0,
                    output_quality: 0.9,
                    execution_cost: 1.0,
                },
            }],
            best_score: Some(0.95),
            best_session: "2026-06-01".into(),
            trend: "improving".into(),
            epoch_class: Some(crate::shared::evolution::EpochClass::Improving),
            stagnation_count: 0,
            skill_attribution: {
                let mut m = HashMap::new();
                m.insert(
                    "rust-borrow-checker".into(),
                    SkillAttribution {
                        skill_name: "rust-borrow-checker".into(),
                        sessions_active: 5,
                        avg_score_with: 0.9,
                        avg_score_without: 0.7,
                        first_seen: "2026-05-01".into(),
                        sessions_holdout: 0,
                    },
                );
                m
            },
            last_error_context: Some("type_error in main.rs".into()),
            reward_hacking_suspected: false,
        }
    }

    #[tokio::test]
    async fn reflection_metrics_replay_after_commit_is_not_applied_twice() {
        let pool = test_pool().await;
        let metrics = sample_metrics();
        assert!(
            save_metrics_once_pool(&pool, &metrics, "project-a", "session-a")
                .await
                .unwrap()
        );
        assert!(
            !save_metrics_once_pool(&pool, &metrics, "project-a", "session-a")
                .await
                .unwrap()
        );
        let loaded = load_metrics_scoped_pool(&pool, Some("project-a"))
            .await
            .unwrap();
        assert_eq!(loaded.total_sessions, metrics.total_sessions);
        assert_eq!(loaded.score_history.len(), metrics.score_history.len());
    }

    fn sample_metrics_reward_hacking() -> Metrics {
        let mut m = sample_metrics();
        m.reward_hacking_suspected = true;
        m.epoch_class = Some(crate::shared::evolution::EpochClass::Regressing);
        m
    }

    #[tokio::test]
    async fn save_and_load_metrics() {
        let pool = test_pool().await;
        let m = sample_metrics();
        save_metrics_pool(&pool, &m, "test-project").await.unwrap();

        let loaded = load_metrics_pool(&pool).await.unwrap();
        assert_eq!(loaded.total_sessions, 10);
        assert_eq!(loaded.avg_success_rate, 0.92);
        assert_eq!(loaded.score_history.len(), 1);
        assert_eq!(loaded.score_history[0].observations, 42);
        assert_eq!(loaded.trend, "improving");
        assert!(loaded.skill_attribution.contains_key("rust-borrow-checker"));
        assert_eq!(
            loaded.epoch_class,
            Some(crate::shared::evolution::EpochClass::Improving)
        );
        assert!(!loaded.reward_hacking_suspected);
    }

    #[tokio::test]
    async fn save_and_load_metrics_round_trips_reward_hacking_flag() {
        let pool = test_pool().await;
        let m = sample_metrics_reward_hacking();
        save_metrics_pool(&pool, &m, "test-project").await.unwrap();

        let loaded = load_metrics_pool(&pool).await.unwrap();
        assert!(
            loaded.reward_hacking_suspected,
            "reward_hacking_suspected must round-trip through SQLite"
        );
        assert_eq!(
            loaded.epoch_class,
            Some(crate::shared::evolution::EpochClass::Regressing),
            "epoch_class must round-trip through SQLite"
        );
    }

    #[tokio::test]
    async fn load_empty_metrics() {
        let pool = test_pool().await;
        let loaded = load_metrics_pool(&pool).await.unwrap();
        assert_eq!(loaded.total_sessions, 0);
        assert!(loaded.score_history.is_empty());
    }

    #[tokio::test]
    async fn score_history_cap_retains_most_recent() {
        let pool = test_pool().await;
        let mut m = sample_metrics();
        for i in 0..60 {
            m.score_history.push(SessionScoreEntry {
                timestamp: format!("2026-06-{:02}T10:00:00Z", i % 28 + 1),
                success_rate: 0.9,
                avg_score: 0.85,
                observations: i,
                dimension_averages: ScoreDimensions::default(),
            });
        }
        save_metrics_pool(&pool, &m, "test-project").await.unwrap();

        let loaded = load_metrics_pool(&pool).await.unwrap();
        assert!(loaded.score_history.len() <= 50);

        let last = loaded.score_history.last().unwrap();
        assert_eq!(last.observations, 59);

        let first = &loaded.score_history[0];
        assert!(
            first.observations >= 10,
            "expected observations >= 10, got {}",
            first.observations
        );
    }

    #[tokio::test]
    async fn metrics_state_isolates_by_project() {
        // AC1: writers scoped to project; a scoped read returns only that
        // project's rows. Requires the composite PK (key, project).
        let pool = test_pool().await;
        let mut a = sample_metrics();
        a.total_sessions = 5;
        save_metrics_pool(&pool, &a, "proj-a").await.unwrap();

        let mut b = sample_metrics();
        b.total_sessions = 99;
        save_metrics_pool(&pool, &b, "proj-b").await.unwrap();

        let la = load_metrics_scoped_pool(&pool, Some("proj-a"))
            .await
            .unwrap();
        let lb = load_metrics_scoped_pool(&pool, Some("proj-b"))
            .await
            .unwrap();
        assert_eq!(
            la.total_sessions, 5,
            "proj-a must not be overwritten by proj-b"
        );
        assert_eq!(lb.total_sessions, 99);
    }

    #[tokio::test]
    async fn scoped_string_metrics_round_trip() {
        let pool = test_pool().await;
        let m = sample_metrics();
        save_metrics_pool(&pool, &m, "proj-a").await.unwrap();

        let loaded = load_metrics_scoped_pool(&pool, Some("proj-a"))
            .await
            .unwrap();

        assert_eq!(loaded.last_session, m.last_session);
        assert_eq!(loaded.best_session, m.best_session);
        assert_eq!(loaded.trend, m.trend);
        assert_eq!(loaded.last_error_context, m.last_error_context);
        assert_eq!(loaded.epoch_class, m.epoch_class);
    }

    #[tokio::test]
    async fn score_history_delete_is_project_scoped() {
        // AC3: saving proj-a must not wipe proj-b's score_history.
        let pool = test_pool().await;
        let mut b = sample_metrics();
        b.score_history.push(SessionScoreEntry {
            timestamp: "2026-06-01T10:00:00Z".into(),
            success_rate: 0.9,
            avg_score: 0.85,
            observations: 1,
            dimension_averages: ScoreDimensions::default(),
        });
        save_metrics_pool(&pool, &b, "proj-b").await.unwrap();

        let a = sample_metrics();
        save_metrics_pool(&pool, &a, "proj-a").await.unwrap();

        let lb = load_metrics_scoped_pool(&pool, Some("proj-b"))
            .await
            .unwrap();
        assert_eq!(
            lb.score_history.len(),
            2,
            "proj-b score_history must survive a proj-a save"
        );
    }

    #[tokio::test]
    async fn skill_attribution_isolates_by_project() {
        // AC2: skill_attribution rows are scoped to (skill_name, project).
        let pool = test_pool().await;
        let mut a = sample_metrics();
        a.skill_attribution.clear();
        a.skill_attribution.insert(
            "skill-a".into(),
            SkillAttribution {
                skill_name: "skill-a".into(),
                sessions_active: 1,
                avg_score_with: 0.8,
                avg_score_without: 0.5,
                first_seen: "2026-06-19".into(),
                sessions_holdout: 0,
            },
        );
        save_metrics_pool(&pool, &a, "proj-a").await.unwrap();

        let mut b = sample_metrics();
        b.skill_attribution.clear();
        b.skill_attribution.insert(
            "skill-b".into(),
            SkillAttribution {
                skill_name: "skill-b".into(),
                sessions_active: 2,
                avg_score_with: 0.7,
                avg_score_without: 0.4,
                first_seen: "2026-06-19".into(),
                sessions_holdout: 0,
            },
        );
        save_metrics_pool(&pool, &b, "proj-b").await.unwrap();

        let la = load_metrics_scoped_pool(&pool, Some("proj-a"))
            .await
            .unwrap();
        let lb = load_metrics_scoped_pool(&pool, Some("proj-b"))
            .await
            .unwrap();
        assert!(la.skill_attribution.contains_key("skill-a"));
        assert!(!la.skill_attribution.contains_key("skill-b"));
        assert!(lb.skill_attribution.contains_key("skill-b"));
        assert!(!lb.skill_attribution.contains_key("skill-a"));
    }

    #[tokio::test]
    async fn init_schema_pool_is_idempotent() {
        // AC2: re-running init (and thus both PK migrations) must be a no-op.
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        super::super::schema::init_schema_pool(&pool).await.unwrap();
    }
}
