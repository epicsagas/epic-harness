//! metrics.rs — Metrics state SQLite I/O (3-table normalized)
//!
//! Replaces the single `metrics.json` file with:
//! - `metrics_state` — key-value scalar fields
//! - `score_history` — session score entries (capped at 50)
//! - `skill_attribution` — per-skill A/B scores

use rusqlite::Connection;
use std::collections::HashMap;
use std::io;

use crate::shared::evolution::{Metrics, SessionScoreEntry, SkillAttribution};
use crate::shared::scoring::ScoreDimensions;

use super::{ImmediateTx, store_err};

/// Maximum score history entries to retain.
const MAX_SCORE_HISTORY: usize = 50;

// ── Load ─────────────────────────────────────────────

/// Load the full Metrics struct from SQLite for a given project.
pub fn load_metrics_conn(conn: &Connection, project: &str) -> io::Result<Metrics> {
    // Load all scalar state in one query instead of one SELECT per key.
    let mut kv_stmt =
        store_err(conn.prepare("SELECT key, value FROM metrics_state WHERE project = ?1"))?;
    let state: HashMap<String, String> =
        store_err(kv_stmt.query_map(rusqlite::params![project], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }))?
        .filter_map(|r| match r {
            Ok(pair) => Some(pair),
            Err(e) => {
                eprintln!("[store/metrics] error reading metrics_state row: {e}");
                None
            }
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
    let mut sh_stmt = store_err(conn.prepare(
        "SELECT timestamp, success_rate, avg_score, observations,
                    dim_success, dim_quality, dim_cost
             FROM score_history WHERE project = ?1 ORDER BY id ASC",
    ))?;

    let score_history: Vec<SessionScoreEntry> =
        store_err(sh_stmt.query_map(rusqlite::params![project], |row| {
            Ok(SessionScoreEntry {
                timestamp: row.get(0)?,
                success_rate: row.get(1)?,
                avg_score: row.get(2)?,
                observations: super::i64_to_u64(row.get::<_, i64>(3)?),
                dimension_averages: ScoreDimensions {
                    tool_success: row.get(4)?,
                    output_quality: row.get(5)?,
                    execution_cost: row.get(6)?,
                },
            })
        }))?
        .filter_map(|r| match r {
            Ok(entry) => Some(entry),
            Err(e) => {
                eprintln!("[store/metrics] skipping malformed score_history row: {e}");
                None
            }
        })
        .collect();

    // Skill attribution
    let mut sa_stmt = store_err(conn.prepare(
        "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen
             FROM skill_attribution WHERE project = ?1",
    ))?;

    let skill_attribution: HashMap<String, SkillAttribution> =
        store_err(sa_stmt.query_map(rusqlite::params![project], |row| {
            Ok(SkillAttribution {
                skill_name: row.get(0)?,
                sessions_active: super::i64_to_u64(row.get::<_, i64>(1)?),
                avg_score_with: row.get(2)?,
                avg_score_without: row.get(3)?,
                first_seen: row.get(4)?,
            })
        }))?
        .filter_map(|r| match r {
            Ok(sa) => Some(sa),
            Err(e) => {
                eprintln!("[store/metrics] skipping malformed skill_attribution row: {e}");
                None
            }
        })
        .map(|sa| (sa.skill_name.clone(), sa))
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

// ── Save ─────────────────────────────────────────────

/// Save the full Metrics struct to SQLite.
///
/// Uses UPSERT (INSERT OR REPLACE) for scalar state and skill_attribution
/// instead of full DELETE + reinsert to avoid data loss on partial failures.
/// Score history is capped at MAX_SCORE_HISTORY most recent entries.
///
/// Precondition: caller must not hold an active transaction on this connection.
pub fn save_metrics_conn(conn: &Connection, project: &str, m: &Metrics) -> io::Result<()> {
    let tx = ImmediateTx::begin(conn)?;

    // Scalar state — upsert each key with project
    let upsert = |key: &str, value: &str, proj: &str| {
        conn.execute(
            "INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, value, proj],
        )
    };

    store_err(upsert(
        "total_sessions",
        &m.total_sessions.to_string(),
        project,
    ))?;
    store_err(upsert(
        "avg_success_rate",
        &m.avg_success_rate.to_string(),
        project,
    ))?;
    store_err(upsert(
        "total_evolved_skills",
        &m.total_evolved_skills.to_string(),
        project,
    ))?;
    // Option fields follow a deliberate split policy:
    // - Fields with meaningful "absence" (last_session, best_score, last_error_context)
    //   → DELETE the key when None so load sees a clean absence, not a stale value.
    // - Fields with a domain-valid default (trend="stable", best_session="", stagnation=0)
    //   → always UPSERT; the default value IS the absent state.
    match &m.last_session {
        Some(v) => store_err(upsert("last_session", v, project))?,
        None => store_err(conn.execute(
            "DELETE FROM metrics_state WHERE key = 'last_session' AND project = ?1",
            rusqlite::params![project],
        ))?,
    };
    match m.best_score {
        Some(v) => store_err(upsert("best_score", &v.to_string(), project))?,
        None => store_err(conn.execute(
            "DELETE FROM metrics_state WHERE key = 'best_score' AND project = ?1",
            rusqlite::params![project],
        ))?,
    };
    store_err(upsert("best_session", &m.best_session, project))?;
    store_err(upsert("trend", &m.trend, project))?;
    store_err(upsert(
        "stagnation_count",
        &m.stagnation_count.to_string(),
        project,
    ))?;
    match &m.last_error_context {
        Some(v) => store_err(upsert("last_error_context", v, project))?,
        None => store_err(conn.execute(
            "DELETE FROM metrics_state WHERE key = 'last_error_context' AND project = ?1",
            rusqlite::params![project],
        ))?,
    };

    // Score history — UPSERT per entry to avoid rowid inflation from DELETE+INSERT.
    // timestamp is used as a natural key (sessions produce unique timestamps).
    // After upsert, prune any rows beyond MAX_SCORE_HISTORY.
    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    for entry in entries.into_iter().rev() {
        store_err(conn.execute(
            "INSERT INTO score_history (timestamp, success_rate, avg_score, observations,
             dim_success, dim_quality, dim_cost, project)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(timestamp) DO UPDATE SET
                 success_rate = excluded.success_rate,
                 avg_score = excluded.avg_score,
                 observations = excluded.observations,
                 dim_success = excluded.dim_success,
                 dim_quality = excluded.dim_quality,
                 dim_cost = excluded.dim_cost",
            rusqlite::params![
                entry.timestamp,
                entry.success_rate,
                entry.avg_score,
                super::u64_to_i64(entry.observations),
                entry.dimension_averages.tool_success,
                entry.dimension_averages.output_quality,
                entry.dimension_averages.execution_cost,
                project,
            ],
        ))?;
    }
    // Prune rows beyond the cap (oldest first, per-project)
    store_err(conn.execute(
        "DELETE FROM score_history WHERE project = ?1 AND id NOT IN (
            SELECT id FROM score_history WHERE project = ?1 ORDER BY id DESC LIMIT ?
        )",
        rusqlite::params![project, MAX_SCORE_HISTORY as i64],
    ))?;

    // Skill attribution — UPSERT preserving first_seen on conflict
    for sa in m.skill_attribution.values() {
        store_err(conn.execute(
            "INSERT INTO skill_attribution
             (skill_name, project, sessions_active, avg_score_with, avg_score_without, first_seen)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(skill_name, project) DO UPDATE SET
                 sessions_active = excluded.sessions_active,
                 avg_score_with = excluded.avg_score_with,
                 avg_score_without = excluded.avg_score_without,
                 first_seen = MIN(skill_attribution.first_seen, excluded.first_seen)",
            rusqlite::params![
                sa.skill_name,
                project,
                super::u64_to_i64(sa.sessions_active),
                sa.avg_score_with,
                sa.avg_score_without,
                sa.first_seen,
            ],
        ))?;
    }

    tx.commit()?;
    Ok(())
}

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

/// Load the full Metrics struct from SQLite using a pool.
#[allow(dead_code)]
pub async fn load_metrics_pool(pool: &SqlitePool, project: &str) -> io::Result<Metrics> {
    // Scalar state
    let kv_rows = sqlx::query("SELECT key, value FROM metrics_state WHERE project = ?1")
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
        "SELECT timestamp, success_rate, avg_score, observations, dim_success, dim_quality, dim_cost FROM score_history WHERE project = ?1 ORDER BY id ASC"
    )
    .bind(project)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let score_history: Vec<SessionScoreEntry> = sh_rows
        .iter()
        .filter_map(|r| {
            let obs: i64 = r.try_get(3).ok()?;
            Some(SessionScoreEntry {
                timestamp: r.try_get(0).ok()?,
                success_rate: r.try_get(1).ok()?,
                avg_score: r.try_get(2).ok()?,
                observations: obs as u64,
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
        "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen FROM skill_attribution WHERE project = ?1"
    )
    .bind(project)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

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
        last_error_context,
    })
}

/// Save the full Metrics struct to SQLite using a pool.
#[allow(dead_code)]
pub async fn save_metrics_pool(pool: &SqlitePool, project: &str, m: &Metrics) -> io::Result<()> {
    let mut tx = pool
        .begin()
        .await
        .map_err(crate::store::sqlx_err)?;

    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("total_sessions")
        .bind(m.total_sessions.to_string())
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("avg_success_rate")
        .bind(m.avg_success_rate.to_string())
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("total_evolved_skills")
        .bind(m.total_evolved_skills.to_string())
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;

    match &m.last_session {
        Some(v) => {
            sqlx::query(
                "INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)",
            )
            .bind("last_session")
            .bind(v)
            .bind(project)
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
        }
        None => {
            sqlx::query("DELETE FROM metrics_state WHERE key = 'last_session' AND project = ?1")
                .bind(project)
                .execute(&mut *tx)
                .await
                .map_err(crate::store::sqlx_err)?;
        }
    };
    match m.best_score {
        Some(v) => {
            sqlx::query(
                "INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)",
            )
            .bind("best_score")
            .bind(v.to_string())
            .bind(project)
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
        }
        None => {
            sqlx::query("DELETE FROM metrics_state WHERE key = 'best_score' AND project = ?1")
                .bind(project)
                .execute(&mut *tx)
                .await
                .map_err(crate::store::sqlx_err)?;
        }
    };
    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("best_session")
        .bind(&m.best_session)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("trend")
        .bind(&m.trend)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    sqlx::query("INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)")
        .bind("stagnation_count")
        .bind(m.stagnation_count.to_string())
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    match &m.last_error_context {
        Some(v) => {
            sqlx::query(
                "INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)",
            )
            .bind("last_error_context")
            .bind(v)
            .bind(project)
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
        }
        None => {
            sqlx::query(
                "DELETE FROM metrics_state WHERE key = 'last_error_context' AND project = ?1",
            )
            .bind(project)
            .execute(&mut *tx)
            .await
            .map_err(crate::store::sqlx_err)?;
        }
    };

    // Score history — UPSERT with cap
    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    for entry in entries.into_iter().rev() {
        sqlx::query(
            "INSERT INTO score_history (timestamp, success_rate, avg_score, observations, dim_success, dim_quality, dim_cost, project) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(timestamp) DO UPDATE SET success_rate = excluded.success_rate, avg_score = excluded.avg_score, observations = excluded.observations, dim_success = excluded.dim_success, dim_quality = excluded.dim_quality, dim_cost = excluded.dim_cost"
        )
        .bind(&entry.timestamp)
        .bind(entry.success_rate)
        .bind(entry.avg_score)
        .bind(crate::store::u64_to_i64(entry.observations))
        .bind(entry.dimension_averages.tool_success)
        .bind(entry.dimension_averages.output_quality)
        .bind(entry.dimension_averages.execution_cost)
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    }
    sqlx::query(
        "DELETE FROM score_history WHERE project = ?1 AND id NOT IN (SELECT id FROM score_history WHERE project = ?1 ORDER BY id DESC LIMIT ?2)"
    )
    .bind(project)
    .bind(MAX_SCORE_HISTORY as i64)
    .execute(&mut *tx)
    .await
    .map_err(crate::store::sqlx_err)?;

    // Skill attribution
    for sa in m.skill_attribution.values() {
        sqlx::query(
            "INSERT INTO skill_attribution (skill_name, project, sessions_active, avg_score_with, avg_score_without, first_seen) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(skill_name, project) DO UPDATE SET sessions_active = excluded.sessions_active, avg_score_with = excluded.avg_score_with, avg_score_without = excluded.avg_score_without, first_seen = MIN(skill_attribution.first_seen, excluded.first_seen)"
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

    tx.commit()
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
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

    #[test]
    fn save_and_load_metrics() {
        let conn = in_memory_db();
        let m = sample_metrics();
        save_metrics_conn(&conn, "test-project", &m).unwrap();

        let loaded = load_metrics_conn(&conn, "test-project").unwrap();
        assert_eq!(loaded.total_sessions, 10);
        assert_eq!(loaded.avg_success_rate, 0.92);
        assert_eq!(loaded.score_history.len(), 1);
        assert_eq!(loaded.score_history[0].observations, 42);
        assert_eq!(loaded.trend, "improving");
        assert!(loaded.skill_attribution.contains_key("rust-borrow-checker"));
    }

    #[test]
    fn load_empty_metrics() {
        let conn = in_memory_db();
        let loaded = load_metrics_conn(&conn, "test-project").unwrap();
        assert_eq!(loaded.total_sessions, 0);
        assert!(loaded.score_history.is_empty());
    }

    #[test]
    fn score_history_cap_retains_most_recent() {
        let conn = in_memory_db();
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
        save_metrics_conn(&conn, "test-project", &m).unwrap();

        let loaded = load_metrics_conn(&conn, "test-project").unwrap();
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
}
