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
        "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen
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
        epoch_class: None,
        last_error_context,
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

/// Save the full Metrics struct to SQLite.
pub async fn save_metrics_pool(pool: &AnyPool, m: &Metrics) -> io::Result<()> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    // Scalar state — upsert each key
    async fn upsert(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        key: &str,
        value: &str,
    ) -> io::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&mut **tx)
            .await
            .map_err(super::sqlx_err)?;
        Ok(())
    }

    upsert(&mut tx, "total_sessions", &m.total_sessions.to_string()).await?;
    upsert(&mut tx, "avg_success_rate", &m.avg_success_rate.to_string()).await?;
    upsert(
        &mut tx,
        "total_evolved_skills",
        &m.total_evolved_skills.to_string(),
    )
    .await?;
    if let Some(ref v) = m.last_session {
        upsert(&mut tx, "last_session", v).await?;
    }
    if let Some(v) = m.best_score {
        upsert(&mut tx, "best_score", &v.to_string()).await?;
    }
    upsert(&mut tx, "best_session", &m.best_session).await?;
    upsert(&mut tx, "trend", &m.trend).await?;
    upsert(&mut tx, "stagnation_count", &m.stagnation_count.to_string()).await?;
    if let Some(ref v) = m.last_error_context {
        upsert(&mut tx, "last_error_context", v).await?;
    }

    // Score history — keep the most recent MAX_SCORE_HISTORY entries.
    sqlx::query("DELETE FROM score_history")
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
             dim_success, dim_quality, dim_cost) VALUES (?,?,?,?,?,?,?)",
        )
        .bind(&entry.timestamp)
        .bind(entry.success_rate)
        .bind(entry.avg_score)
        .bind(super::u64_to_i64(entry.observations))
        .bind(entry.dimension_averages.tool_success)
        .bind(entry.dimension_averages.output_quality)
        .bind(entry.dimension_averages.execution_cost)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    // Skill attribution — UPSERT per skill
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT OR REPLACE INTO skill_attribution
             (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen)
             VALUES (?,?,?,?,?)",
        )
        .bind(&sa.skill_name)
        .bind(super::u64_to_i64(sa.sessions_active))
        .bind(sa.avg_score_with)
        .bind(sa.avg_score_without)
        .bind(&sa.first_seen)
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(())
}

/// Standalone save.
pub fn save_metrics(m: &Metrics) -> io::Result<()> {
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        save_metrics_pool(&pool, m).await
    })
}

/// Save metrics directly into an existing transaction (no inner BEGIN/COMMIT).
/// Used by `migrate.rs` which already holds a transaction.
pub async fn save_metrics_direct(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    m: &Metrics,
) -> io::Result<()> {
    async fn upsert(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        key: &str,
        value: &str,
    ) -> io::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&mut **tx)
            .await
            .map_err(super::sqlx_err)?;
        Ok(())
    }

    upsert(tx, "total_sessions", &m.total_sessions.to_string()).await?;
    upsert(tx, "avg_success_rate", &m.avg_success_rate.to_string()).await?;
    upsert(
        tx,
        "total_evolved_skills",
        &m.total_evolved_skills.to_string(),
    )
    .await?;
    if let Some(ref v) = m.last_session {
        upsert(tx, "last_session", v).await?;
    }
    if let Some(v) = m.best_score {
        upsert(tx, "best_score", &v.to_string()).await?;
    }
    upsert(tx, "best_session", &m.best_session).await?;
    upsert(tx, "trend", &m.trend).await?;
    upsert(tx, "stagnation_count", &m.stagnation_count.to_string()).await?;
    if let Some(ref v) = m.last_error_context {
        upsert(tx, "last_error_context", v).await?;
    }

    // Score history
    sqlx::query("DELETE FROM score_history")
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
             dim_success, dim_quality, dim_cost) VALUES (?,?,?,?,?,?,?)",
        )
        .bind(&entry.timestamp)
        .bind(entry.success_rate)
        .bind(entry.avg_score)
        .bind(super::u64_to_i64(entry.observations))
        .bind(entry.dimension_averages.tool_success)
        .bind(entry.dimension_averages.output_quality)
        .bind(entry.dimension_averages.execution_cost)
        .execute(&mut **tx)
        .await
        .map_err(super::sqlx_err)?;
    }

    // Skill attribution
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT OR REPLACE INTO skill_attribution
             (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen)
             VALUES (?,?,?,?,?)",
        )
        .bind(&sa.skill_name)
        .bind(super::u64_to_i64(sa.sessions_active))
        .bind(sa.avg_score_with)
        .bind(sa.avg_score_without)
        .bind(&sa.first_seen)
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
            epoch_class: None,
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
                    },
                );
                m
            },
            last_error_context: Some("type_error in main.rs".into()),
        }
    }

    #[tokio::test]
    async fn save_and_load_metrics() {
        let pool = test_pool().await;
        let m = sample_metrics();
        save_metrics_pool(&pool, &m).await.unwrap();

        let loaded = load_metrics_pool(&pool).await.unwrap();
        assert_eq!(loaded.total_sessions, 10);
        assert_eq!(loaded.avg_success_rate, 0.92);
        assert_eq!(loaded.score_history.len(), 1);
        assert_eq!(loaded.score_history[0].observations, 42);
        assert_eq!(loaded.trend, "improving");
        assert!(loaded.skill_attribution.contains_key("rust-borrow-checker"));
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
        save_metrics_pool(&pool, &m).await.unwrap();

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
}
