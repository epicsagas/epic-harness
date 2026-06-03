//! evolution.rs — Evolution record SQLite I/O

use rusqlite::Connection;
use std::io;

use crate::shared::evolution::EvolutionRecord;

use super::store_err;

/// Insert an evolution record.
pub fn insert_record_conn(conn: &Connection, rec: &EvolutionRecord) -> io::Result<i64> {
    let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|e| {
        eprintln!("[store/evolution] error_patterns serialization failed: {e}");
        "{}".into()
    });
    let failure_json = serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|e| {
        eprintln!("[store/evolution] failure_patterns serialization failed: {e}");
        "[]".into()
    });

    store_err(conn.execute(
        "INSERT INTO evolution_records
         (timestamp, observations, success_rate, avg_score, error_patterns,
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![
            rec.timestamp,
            super::u64_to_i64(rec.observations),
            rec.success_rate,
            rec.avg_score,
            error_json,
            failure_json,
            super::u64_to_i64(rec.skills_seeded),
            super::u64_to_i64(rec.skills_rolled_back),
            super::u64_to_i64(rec.total_evolved),
            rec.analysis_summary,
        ],
    ))?;
    Ok(conn.last_insert_rowid())
}

/// Standalone insert — opens own connection.
/// Currently unused; retained for future batch import scenarios.
#[allow(dead_code)]
pub(crate) fn insert_record(rec: &EvolutionRecord) -> io::Result<i64> {
    let conn = super::open_harness_db()?;
    insert_record_conn(&conn, rec)
}

/// Query the N most recent evolution records.
pub fn query_recent_records_conn(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<EvolutionRecord>> {
    let mut stmt = store_err(conn
        .prepare(
            "SELECT timestamp, observations, success_rate, avg_score, error_patterns,
                    failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary
             FROM evolution_records ORDER BY id DESC LIMIT ?1",
        ))?;

    let rows = store_err(stmt.query_map(rusqlite::params![limit as i64], |row| {
        let error_json: String = row.get(4)?;
        let failure_json: String = row.get(5)?;
        Ok(EvolutionRecord {
            timestamp: row.get(0)?,
            observations: super::i64_to_u64(row.get::<_, i64>(1)?),
            success_rate: row.get(2)?,
            avg_score: row.get(3)?,
            error_patterns: serde_json::from_str(&error_json).unwrap_or_default(),
            failure_patterns: serde_json::from_str(&failure_json).unwrap_or_default(),
            skills_seeded: super::i64_to_u64(row.get::<_, i64>(6)?),
            skills_rolled_back: super::i64_to_u64(row.get::<_, i64>(7)?),
            total_evolved: super::i64_to_u64(row.get::<_, i64>(8)?),
            analysis_summary: row.get(9)?,
        })
    }))?;

    let mut records = Vec::new();
    for r in rows {
        records.push(store_err(r)?);
    }
    // Reverse so oldest-first (matching original JSONL read order)
    records.reverse();
    Ok(records)
}

/// Query all evolution records (capped at 10_000 to prevent unbounded memory use).
pub fn query_all_records_conn(conn: &Connection) -> io::Result<Vec<EvolutionRecord>> {
    query_recent_records_conn(conn, 10_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn in_memory_db() -> Connection {
        crate::store::in_memory_db()
    }

    #[test]
    fn insert_and_query() {
        let conn = in_memory_db();
        let rec = EvolutionRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            observations: 42,
            success_rate: 0.95,
            avg_score: 0.89,
            error_patterns: {
                let mut m = HashMap::new();
                m.insert("syntax_error".into(), 3);
                m
            },
            failure_patterns: vec![],
            skills_seeded: 1,
            skills_rolled_back: 0,
            total_evolved: 3,
            analysis_summary: "Good session".into(),
        };

        insert_record_conn(&conn, &rec).unwrap();

        let results = query_recent_records_conn(&conn, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observations, 42);
        assert_eq!(results[0].error_patterns.get("syntax_error"), Some(&3));
    }
}
