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
    // Scalar state
    let get = |key: &str| -> String {
        conn.query_row(
            "SELECT value FROM metrics_state WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    };

    let total_sessions: u64 = get("total_sessions").parse().unwrap_or(0);
    let avg_success_rate: f64 = get("avg_success_rate").parse().unwrap_or(0.0);
    let total_evolved_skills: u64 = get("total_evolved_skills").parse().unwrap_or(0);
    let last_session = {
        let v = get("last_session");
        if v.is_empty() { None } else { Some(v) }
    };
    let best_score: Option<f64> = {
        let v = get("best_score");
        v.parse().ok()
    };
    let best_session = get("best_session");
    let trend = {
        let v = get("trend");
        if v.is_empty() { "stable".into() } else { v }
    };
    let stagnation_count: u64 = get("stagnation_count").parse().unwrap_or(0);
    let last_error_context = {
        let v = get("last_error_context");
        if v.is_empty() { None } else { Some(v) }
    };

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

    // Score history — clear and rewrite (capped)
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
                entry.observations as i64,
                entry.dimension_averages.tool_success,
                entry.dimension_averages.output_quality,
                entry.dimension_averages.execution_cost,
            ],
        )
        .map_err(io::Error::other)?;
    }

    // Skill attribution — clear and rewrite
    tx.execute("DELETE FROM skill_attribution", [])
        .map_err(io::Error::other)?;
    for sa in m.skill_attribution.values() {
        tx.execute(
            "INSERT INTO skill_attribution (skill_name, sessions_active, avg_score_with,
             avg_score_without, first_seen) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![
                sa.skill_name,
                sa.sessions_active as i64,
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
    fn score_history_cap_at_50() {
        let conn = in_memory_db();
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
        save_metrics_conn(&conn, &m).unwrap();

        let loaded = load_metrics_conn(&conn).unwrap();
        assert!(loaded.score_history.len() <= 50);
    }
}
