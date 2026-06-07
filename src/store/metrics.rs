//! metrics.rs — Metrics state SQLite I/O (3-table normalized, async pool)
//!
//! Replaces the single `metrics.json` file with:
//! - `metrics_state` — key-value scalar fields
//! - `score_history` — session score entries (capped at 50)
//! - `skill_attribution` — per-skill A/B scores

use std::collections::HashMap;
use std::io;

use crate::shared::evolution::{Metrics, SessionScoreEntry, SkillAttribution};
use crate::shared::scoring::ScoreDimensions;

/// Maximum score history entries to retain.
const MAX_SCORE_HISTORY: usize = 50;

// ── Async pool functions ─────────────────────────────

use sqlx::{AnyPool, Row};

/// Load the full Metrics struct from SQLite using a pool.
pub async fn load_metrics_pool(pool: &AnyPool, project: &str) -> io::Result<Metrics> {
    // Scalar state
    let kv_rows = sqlx::query("SELECT key, value FROM metrics_state WHERE project = $1")
        .bind(project)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;

    let state: HashMap<String, String> = kv_rows
        .iter()
        .filter_map(|r| {
            let key: String = r.try_get(0).ok()?;
            let value: String = r.try_get(1).ok()?;
            Some((key, value))
        })
        .collect();

    let get = |key: &str| -> Option<String> { state.get(key).cloned() };

    let total_sessions: u64 = get("total_sessions")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let avg_success_rate: f64 = get("avg_success_rate")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let total_evolved_skills: u64 = get("total_evolved_skills")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let last_session = get("last_session").filter(|v| !v.is_empty());
    let best_score: Option<f64> = get("best_score").and_then(|v| v.parse().ok());
    let best_session = get("best_session").unwrap_or_default();
    let trend = get("trend").unwrap_or_else(|| "stable".into());
    let stagnation_count: u64 = get("stagnation_count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let last_error_context = get("last_error_context").filter(|v| !v.is_empty());

    // Score history
    let sh_rows = sqlx::query(
        "SELECT timestamp, success_rate, avg_score, observations, dim_success, dim_quality, dim_cost FROM score_history WHERE project = $1 ORDER BY id ASC"
    )
    .bind(project)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let score_history: Vec<SessionScoreEntry> = sh_rows
        .iter()
        .filter_map(|r| {
            let obs: i64 = match r.try_get(3) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                    return None;
                }
            };
            Some(SessionScoreEntry {
                timestamp: match r.try_get(0) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                        return None;
                    }
                },
                success_rate: match r.try_get(1) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                        return None;
                    }
                },
                avg_score: match r.try_get(2) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                        return None;
                    }
                },
                observations: crate::store::i64_to_u64(obs),
                dimension_averages: ScoreDimensions {
                    tool_success: match r.try_get(4) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                            return None;
                        }
                    },
                    output_quality: match r.try_get(5) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                            return None;
                        }
                    },
                    execution_cost: match r.try_get(6) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                            return None;
                        }
                    },
                },
            })
        })
        .collect();

    // Skill attribution
    let sa_rows = sqlx::query(
        "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen FROM skill_attribution WHERE project = $1"
    )
    .bind(project)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let skill_attribution: HashMap<String, SkillAttribution> = sa_rows
        .iter()
        .filter_map(|r| {
            let skill_name: String = match r.try_get(0) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                    return None;
                }
            };
            let sessions_active: i64 = match r.try_get(1) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                    return None;
                }
            };
            let avg_score_with: f64 = match r.try_get(2) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                    return None;
                }
            };
            let avg_score_without: f64 = match r.try_get(3) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                    return None;
                }
            };
            let first_seen: String = match r.try_get(4) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                    return None;
                }
            };
            let sa = SkillAttribution {
                skill_name,
                sessions_active: crate::store::i64_to_u64(sessions_active),
                avg_score_with,
                avg_score_without,
                first_seen,
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
        last_error_context,
    })
}

/// Load aggregated metrics across all projects.
///
/// Aggregation strategy:
/// - Scalar sums: total_sessions, total_evolved_skills, stagnation_count
/// - Weighted avg: avg_success_rate (weighted by session count)
/// - Best/worst: best_score (max), trend (worst: "declining" > "stable" > "improving")
/// - Merge: score_history (all projects combined, sorted by timestamp)
/// - Merge: skill_attribution (aggregate across projects)
/// - First non-empty: last_session, last_error_context
pub async fn load_metrics_all_pool(pool: &AnyPool) -> io::Result<Metrics> {
    use crate::shared::paths::list_harness_project_slugs;
    let slugs = list_harness_project_slugs();

    let mut total_sessions: u64 = 0;
    let mut weighted_success_sum: f64 = 0.0;
    let mut total_evolved_skills: u64 = 0;
    let mut last_session: Option<String> = None;
    let mut all_scores: Vec<SessionScoreEntry> = Vec::new();
    let mut best_score: Option<f64> = None;
    let mut best_session = String::new();
    let mut worst_trend = "stable".to_string();
    let mut stagnation_count: u64 = 0;
    let mut skill_attribution: HashMap<String, SkillAttribution> = HashMap::new();
    let mut last_error_context: Option<String> = None;

    for slug in &slugs {
        let m = match load_metrics_pool(pool, slug).await {
            Ok(m) => m,
            Err(_) => continue,
        };
        total_sessions += m.total_sessions;
        weighted_success_sum += m.avg_success_rate * m.total_sessions as f64;
        total_evolved_skills += m.total_evolved_skills;
        if last_session.is_none() && m.last_session.is_some() {
            last_session = m.last_session;
        }
        all_scores.extend(m.score_history);
        if m.best_score > best_score {
            best_score = m.best_score;
        }
        if m.best_session > best_session {
            best_session = m.best_session;
        }
        // Worst trend: declining > stable > improving
        match m.trend.as_str() {
            "declining" => worst_trend = "declining".into(),
            "stable" if worst_trend != "declining" => worst_trend = "stable".into(),
            _ => {}
        }
        stagnation_count = stagnation_count.max(m.stagnation_count);
        for (name, sa) in m.skill_attribution {
            skill_attribution
                .entry(name)
                .and_modify(|existing: &mut SkillAttribution| {
                    let prev_sess = existing.sessions_active as f64;
                    let new_sess = sa.sessions_active as f64;
                    let total_sess = prev_sess + new_sess;
                    if total_sess > 0.0 {
                        existing.avg_score_with = (existing.avg_score_with * prev_sess
                            + sa.avg_score_with * new_sess)
                            / total_sess;
                        existing.avg_score_without = (existing.avg_score_without * prev_sess
                            + sa.avg_score_without * new_sess)
                            / total_sess;
                    }
                    existing.sessions_active += sa.sessions_active;
                    if sa.first_seen < existing.first_seen {
                        existing.first_seen = sa.first_seen.clone();
                    }
                })
                .or_insert(sa);
        }
        if last_error_context.is_none() && m.last_error_context.is_some() {
            last_error_context = m.last_error_context;
        }
    }

    // Sort and cap score history
    all_scores.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    all_scores.truncate(MAX_SCORE_HISTORY * 3); // Allow more for multi-project

    let avg_success_rate = if total_sessions > 0 {
        weighted_success_sum / total_sessions as f64
    } else {
        0.0
    };

    Ok(Metrics {
        total_sessions,
        avg_success_rate,
        total_evolved_skills,
        last_session,
        score_history: all_scores,
        best_score,
        best_session,
        trend: worst_trend,
        stagnation_count,
        skill_attribution,
        last_error_context,
    })
}

/// Save the full Metrics struct to SQLite using a pool.
pub async fn save_metrics_pool(pool: &AnyPool, project: &str, m: &Metrics) -> io::Result<()> {
    let mut tx = pool.begin().await.map_err(crate::store::sqlx_err)?;

    // Fixed scalar fields — always upsert.
    let fixed: &[(&str, String)] = &[
        ("total_sessions", m.total_sessions.to_string()),
        ("avg_success_rate", m.avg_success_rate.to_string()),
        ("total_evolved_skills", m.total_evolved_skills.to_string()),
        ("best_session", m.best_session.clone()),
        ("trend", m.trend.clone()),
        ("stagnation_count", m.stagnation_count.to_string()),
    ];
    for (key, val) in fixed {
        sqlx::query(
            "INSERT INTO metrics_state (key, value, project) VALUES ($1, $2, $3) ON CONFLICT (key, project) DO UPDATE SET value = excluded.value",
        )
        .bind(*key)
        .bind(val)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    }

    // Optional fields — upsert when present, delete the key when absent so that
    // load sees a clean absence rather than a stale value.
    let optional: &[(&str, Option<String>)] = &[
        ("last_session", m.last_session.clone()),
        ("best_score", m.best_score.map(|v| v.to_string())),
        ("last_error_context", m.last_error_context.clone()),
    ];
    for (key, opt_val) in optional {
        match opt_val {
            Some(val) => {
                sqlx::query(
                    "INSERT INTO metrics_state (key, value, project) VALUES ($1, $2, $3) ON CONFLICT (key, project) DO UPDATE SET value = excluded.value",
                )
                .bind(*key)
                .bind(val)
                .bind(project)
                .execute(&mut *tx)
                .await
                .map_err(crate::store::sqlx_err)?;
            }
            None => {
                sqlx::query("DELETE FROM metrics_state WHERE key = $1 AND project = $2")
                    .bind(*key)
                    .bind(project)
                    .execute(&mut *tx)
                    .await
                    .map_err(crate::store::sqlx_err)?;
            }
        }
    }

    // Score history — UPSERT with cap
    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    if !entries.is_empty() {
        let mut qb = sqlx::QueryBuilder::<sqlx::Any>::new(
            "INSERT INTO score_history \
             (timestamp, success_rate, avg_score, observations, \
              dim_success, dim_quality, dim_cost, project) ",
        );
        qb.push_values(entries.into_iter().rev(), |mut b, entry| {
            b.push_bind(&entry.timestamp)
                .push_bind(entry.success_rate)
                .push_bind(entry.avg_score)
                .push_bind(crate::store::u64_to_i64(entry.observations))
                .push_bind(entry.dimension_averages.tool_success)
                .push_bind(entry.dimension_averages.output_quality)
                .push_bind(entry.dimension_averages.execution_cost)
                .push_bind(project);
        });
        qb.push(
            " ON CONFLICT(timestamp, project) DO UPDATE SET \
             success_rate = excluded.success_rate, \
             avg_score = excluded.avg_score, \
             observations = excluded.observations, \
             dim_success = excluded.dim_success, \
             dim_quality = excluded.dim_quality, \
             dim_cost = excluded.dim_cost",
        );
        qb.build()
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
    }
    sqlx::query(
        "DELETE FROM score_history WHERE project = $1 AND id NOT IN (SELECT id FROM score_history WHERE project = $1 ORDER BY id DESC LIMIT $2)"
    )
    .bind(project)
    .bind(MAX_SCORE_HISTORY as i64)
    .execute(&mut *tx)
    .await
    .map_err(crate::store::sqlx_err)?;

    // Skill attribution
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT INTO skill_attribution (skill_name, project, sessions_active, avg_score_with, avg_score_without, first_seen) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT(skill_name, project) DO UPDATE SET sessions_active = excluded.sessions_active, avg_score_with = excluded.avg_score_with, avg_score_without = excluded.avg_score_without, first_seen = MIN(skill_attribution.first_seen, excluded.first_seen)"
        )
        .bind(&sa.skill_name)
        .bind(project)
        .bind(crate::store::u64_to_i64(sa.sessions_active))
        .bind(sa.avg_score_with)
        .bind(sa.avg_score_without)
        .bind(&sa.first_seen)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    }

    tx.commit().await.map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> sqlx::AnyPool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
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
        let pool = in_memory_pool().await;
        let m = sample_metrics();
        save_metrics_pool(&pool, "test-project", &m).await.unwrap();

        let loaded = load_metrics_pool(&pool, "test-project").await.unwrap();
        assert_eq!(loaded.total_sessions, 10);
        assert_eq!(loaded.avg_success_rate, 0.92);
        assert_eq!(loaded.score_history.len(), 1);
        assert_eq!(loaded.score_history[0].observations, 42);
        assert_eq!(loaded.trend, "improving");
        assert!(loaded.skill_attribution.contains_key("rust-borrow-checker"));
    }

    #[tokio::test]
    async fn load_empty_metrics() {
        let pool = in_memory_pool().await;
        let loaded = load_metrics_pool(&pool, "test-project").await.unwrap();
        assert_eq!(loaded.total_sessions, 0);
        assert!(loaded.score_history.is_empty());
    }

    #[tokio::test]
    async fn score_history_cap_retains_most_recent() {
        let pool = in_memory_pool().await;
        let mut m = sample_metrics();
        // Add 60 entries using July dates so none overlap with the base "2026-06-02" entry.
        // Each entry gets a unique timestamp (day 1..3, hour 0..23).
        for i in 0..60 {
            m.score_history.push(SessionScoreEntry {
                timestamp: format!("2026-07-{:02}T{:02}:00:00Z", i / 24 + 1, i % 24),
                success_rate: 0.9,
                avg_score: 0.85,
                observations: i,
                dimension_averages: ScoreDimensions::default(),
            });
        }
        save_metrics_pool(&pool, "test-project", &m).await.unwrap();

        let loaded = load_metrics_pool(&pool, "test-project").await.unwrap();
        // Should be capped at 50
        assert!(loaded.score_history.len() <= 50);

        // The most recent entries (observations 10..59) should be retained,
        // not the oldest (0..9). The last entry should have observations=59.
        let last = loaded.score_history.last().unwrap();
        assert_eq!(last.observations, 59);

        // The first retained entry should have observations=10 (pruned 0..9)
        let first = &loaded.score_history[0];
        assert!(
            first.observations >= 10,
            "expected observations >= 10, got {}",
            first.observations
        );
    }

    #[tokio::test]
    async fn metrics_state_two_projects_isolated() {
        let pool = in_memory_pool().await;

        let mut m_a = sample_metrics();
        m_a.total_sessions = 10;
        m_a.trend = "improving".into();

        let mut m_b = sample_metrics();
        m_b.total_sessions = 99;
        m_b.trend = "declining".into();

        save_metrics_pool(&pool, "project-a", &m_a).await.unwrap();
        save_metrics_pool(&pool, "project-b", &m_b).await.unwrap();

        let a = load_metrics_pool(&pool, "project-a").await.unwrap();
        let b = load_metrics_pool(&pool, "project-b").await.unwrap();

        assert_eq!(a.total_sessions, 10);
        assert_eq!(a.trend, "improving");
        assert_eq!(b.total_sessions, 99);
        assert_eq!(b.trend, "declining");
    }

    #[tokio::test]
    async fn score_history_same_timestamp_different_projects() {
        let pool = in_memory_pool().await;

        let entry = SessionScoreEntry {
            timestamp: "2026-06-02T10:00:00Z".into(),
            success_rate: 0.9,
            avg_score: 0.85,
            observations: 5,
            dimension_averages: ScoreDimensions::default(),
        };

        let mut m_a = sample_metrics();
        m_a.score_history = vec![entry.clone()];
        let mut m_b = sample_metrics();
        m_b.score_history = vec![SessionScoreEntry {
            observations: 50, // different value
            ..entry
        }];

        save_metrics_pool(&pool, "project-a", &m_a).await.unwrap();
        save_metrics_pool(&pool, "project-b", &m_b).await.unwrap();

        let a = load_metrics_pool(&pool, "project-a").await.unwrap();
        let b = load_metrics_pool(&pool, "project-b").await.unwrap();

        assert_eq!(a.score_history[0].observations, 5);
        assert_eq!(b.score_history[0].observations, 50);
    }
}
