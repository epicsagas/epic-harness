//! metrics.rs — Metrics state SQLite I/O (3-table normalized)
#![allow(dead_code)]
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

/// Maximum score history entries to retain.
const MAX_SCORE_HISTORY: usize = 50;

// ── Load ─────────────────────────────────────────────

/// Load the full Metrics struct from SQLite.
pub fn load_metrics_conn(conn: &Connection) -> io::Result<Metrics> {
    // Scalar state — use explicit Option to distinguish missing vs present
    let get = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM metrics_state WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
    };

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
    let mut sh_stmt = conn
        .prepare(
            "SELECT timestamp, success_rate, avg_score, observations,
                    dim_success, dim_quality, dim_cost
             FROM score_history ORDER BY id ASC",
        )
        .map_err(io::Error::other)?;

    let score_history: Vec<SessionScoreEntry> = sh_stmt
        .query_map([], |row| {
            Ok(SessionScoreEntry {
                timestamp: row.get(0)?,
                success_rate: row.get(1)?,
                avg_score: row.get(2)?,
                observations: row.get::<_, i64>(3)? as u64,
                dimension_averages: ScoreDimensions {
                    tool_success: row.get(4)?,
                    output_quality: row.get(5)?,
                    execution_cost: row.get(6)?,
                },
            })
        })
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();

    // Skill attribution
    let mut sa_stmt = conn
        .prepare(
            "SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen
             FROM skill_attribution",
        )
        .map_err(io::Error::other)?;

    let skill_attribution: HashMap<String, SkillAttribution> = sa_stmt
        .query_map([], |row| {
            Ok(SkillAttribution {
                skill_name: row.get(0)?,
                sessions_active: row.get::<_, i64>(1)? as u64,
                avg_score_with: row.get(2)?,
                avg_score_without: row.get(3)?,
                first_seen: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
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

/// Standalone load.
pub fn load_metrics() -> io::Result<Metrics> {
    let conn = super::open_harness_db()?;
    load_metrics_conn(&conn)
}

// ── Save ─────────────────────────────────────────────

/// Save the full Metrics struct to SQLite.
///
/// Uses UPSERT (INSERT OR REPLACE) for scalar state and skill_attribution
/// instead of full DELETE + reinsert to avoid data loss on partial failures.
/// Score history is capped at MAX_SCORE_HISTORY most recent entries.
pub fn save_metrics_conn(conn: &Connection, m: &Metrics) -> io::Result<()> {
    let tx = conn.unchecked_transaction().map_err(io::Error::other)?;

    // Scalar state — upsert each key
    let upsert = |key: &str, value: &str| {
        tx.execute(
            "INSERT OR REPLACE INTO metrics_state (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )
    };

    upsert("total_sessions", &m.total_sessions.to_string()).map_err(io::Error::other)?;
    upsert("avg_success_rate", &m.avg_success_rate.to_string()).map_err(io::Error::other)?;
    upsert("total_evolved_skills", &m.total_evolved_skills.to_string())
        .map_err(io::Error::other)?;
    if let Some(ref v) = m.last_session {
        upsert("last_session", v).map_err(io::Error::other)?;
    }
    if let Some(v) = m.best_score {
        upsert("best_score", &v.to_string()).map_err(io::Error::other)?;
    }
    upsert("best_session", &m.best_session).map_err(io::Error::other)?;
    upsert("trend", &m.trend).map_err(io::Error::other)?;
    upsert("stagnation_count", &m.stagnation_count.to_string()).map_err(io::Error::other)?;
    if let Some(ref v) = m.last_error_context {
        upsert("last_error_context", v).map_err(io::Error::other)?;
    }

    // Score history — keep the most recent MAX_SCORE_HISTORY entries.
    // Use DELETE + batch INSERT in transaction for ordered append-only data.
    // Double-reverse: first .rev().take(N) selects the N most recent entries
    // (from the tail), then the second .rev() restores chronological order
    // so the DB rows are inserted oldest-first (matching the original append order).
    tx.execute("DELETE FROM score_history", [])
        .map_err(io::Error::other)?;
    let entries: Vec<&SessionScoreEntry> = m
        .score_history
        .iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect();
    for entry in entries.into_iter().rev() {
        tx.execute(
            "INSERT INTO score_history (timestamp, success_rate, avg_score, observations,
             dim_success, dim_quality, dim_cost) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                entry.timestamp,
                entry.success_rate,
                entry.avg_score,
                super::u64_to_i64(entry.observations),
                entry.dimension_averages.tool_success,
                entry.dimension_averages.output_quality,
                entry.dimension_averages.execution_cost,
            ],
        )
        .map_err(io::Error::other)?;
    }

    // Skill attribution — UPSERT per skill instead of DELETE all + reinsert
    for sa in m.skill_attribution.values() {
        tx.execute(
            "INSERT OR REPLACE INTO skill_attribution
             (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen)
             VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![
                sa.skill_name,
                super::u64_to_i64(sa.sessions_active),
                sa.avg_score_with,
                sa.avg_score_without,
                sa.first_seen,
            ],
        )
        .map_err(io::Error::other)?;
    }

    tx.commit().map_err(io::Error::other)?;
    Ok(())
}

/// Standalone save.
pub fn save_metrics(m: &Metrics) -> io::Result<()> {
    let conn = super::open_harness_db()?;
    save_metrics_conn(&conn, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();
        conn
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
        save_metrics_conn(&conn, &m).unwrap();

        let loaded = load_metrics_conn(&conn).unwrap();
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
        let loaded = load_metrics_conn(&conn).unwrap();
        assert_eq!(loaded.total_sessions, 0);
        assert!(loaded.score_history.is_empty());
    }

    #[test]
    fn score_history_cap_retains_most_recent() {
        let conn = in_memory_db();
        let mut m = sample_metrics();
        // Add 60 entries with ascending observations values
        for i in 0..60 {
            m.score_history.push(SessionScoreEntry {
                timestamp: format!("2026-06-{:02}T10:00:00Z", i % 28 + 1),
                success_rate: 0.9,
                avg_score: 0.85,
                observations: i,
                dimension_averages: ScoreDimensions::default(),
            });
        }
        save_metrics_conn(&conn, &m).unwrap();

        let loaded = load_metrics_conn(&conn).unwrap();
        // Should be capped at 50
        assert!(loaded.score_history.len() <= 50);

        // The most recent entries (observations 11..60) should be retained,
        // not the oldest (0..10). The last entry should have observations=59.
        let last = loaded.score_history.last().unwrap();
        assert_eq!(last.observations, 59);

        // The first retained entry should have observations=10 (skipped 0..9)
        let first = &loaded.score_history[0];
        assert!(
            first.observations >= 10,
            "expected observations >= 10, got {}",
            first.observations
        );
    }
}
