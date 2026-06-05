//! evolution.rs — Evolution record SQLite I/O

use std::io;

use crate::shared::evolution::EvolutionRecord;

// ── Async pool functions ─────────────────────────────

use sqlx::{Row, SqlitePool};

pub async fn insert_record_pool(
    pool: &SqlitePool,
    project: &str,
    rec: &EvolutionRecord,
) -> io::Result<i64> {
    let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|e| {
        eprintln!("[store/evolution] error_patterns serialization failed: {e}");
        "{}".into()
    });
    let failure_json = serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|e| {
        eprintln!("[store/evolution] failure_patterns serialization failed: {e}");
        "[]".into()
    });

    let result = sqlx::query(
        "INSERT INTO evolution_records (timestamp, observations, success_rate, avg_score, error_patterns, failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary, project) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"
    )
    .bind(&rec.timestamp)
    .bind(crate::store::u64_to_i64(rec.observations))
    .bind(rec.success_rate)
    .bind(rec.avg_score)
    .bind(&error_json)
    .bind(&failure_json)
    .bind(crate::store::u64_to_i64(rec.skills_seeded))
    .bind(crate::store::u64_to_i64(rec.skills_rolled_back))
    .bind(crate::store::u64_to_i64(rec.total_evolved))
    .bind(&rec.analysis_summary)
    .bind(project)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result.last_insert_rowid())
}

pub async fn query_recent_records_pool(
    pool: &SqlitePool,
    project: &str,
    limit: i64,
) -> io::Result<Vec<EvolutionRecord>> {
    let rows = sqlx::query(
        "SELECT timestamp, observations, success_rate, avg_score, error_patterns, failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary FROM evolution_records WHERE project = ?1 ORDER BY id DESC LIMIT ?2"
    )
    .bind(project)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    let mut records: Vec<EvolutionRecord> = rows
        .iter()
        .map(|r| {
            let error_json: String = r.try_get(4).unwrap_or_else(|_| "{}".into());
            let failure_json: String = r.try_get(5).unwrap_or_else(|_| "[]".into());
            EvolutionRecord {
                timestamp: r.try_get(0).unwrap_or_default(),
                observations: crate::store::i64_to_u64(r.try_get::<i64, _>(1).unwrap_or(0)),
                success_rate: r.try_get(2).unwrap_or(0.0),
                avg_score: r.try_get(3).unwrap_or(0.0),
                error_patterns: serde_json::from_str(&error_json).unwrap_or_default(),
                failure_patterns: serde_json::from_str(&failure_json).unwrap_or_default(),
                skills_seeded: crate::store::i64_to_u64(r.try_get::<i64, _>(6).unwrap_or(0)),
                skills_rolled_back: crate::store::i64_to_u64(r.try_get::<i64, _>(7).unwrap_or(0)),
                total_evolved: crate::store::i64_to_u64(r.try_get::<i64, _>(8).unwrap_or(0)),
                analysis_summary: r.try_get(9).unwrap_or_default(),
            }
        })
        .collect();
    records.reverse();
    Ok(records)
}

pub async fn query_all_records_pool(
    pool: &SqlitePool,
    project: &str,
) -> io::Result<Vec<EvolutionRecord>> {
    query_recent_records_pool(pool, project, 10_000).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    async fn in_memory_pool() -> sqlx::SqlitePool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_query() {
        let pool = in_memory_pool().await;
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

        insert_record_pool(&pool, "test-project", &rec)
            .await
            .unwrap();

        let results = query_recent_records_pool(&pool, "test-project", 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observations, 42);
        assert_eq!(results[0].error_patterns.get("syntax_error"), Some(&3));
    }

    #[tokio::test]
    async fn insert_saturates_u64_counters_for_sqlite_storage() {
        let pool = in_memory_pool().await;
        let rec = EvolutionRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            observations: u64::MAX,
            success_rate: 0.95,
            avg_score: 0.89,
            error_patterns: HashMap::new(),
            failure_patterns: vec![],
            skills_seeded: u64::MAX,
            skills_rolled_back: u64::MAX,
            total_evolved: u64::MAX,
            analysis_summary: "Overflow counters".into(),
        };

        insert_record_pool(&pool, "test-project", &rec)
            .await
            .unwrap();

        let row = sqlx::query(
            "SELECT observations, skills_seeded, skills_rolled_back, total_evolved
             FROM evolution_records
             WHERE project = ?1",
        )
        .bind("test-project")
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.try_get::<i64, _>(0).unwrap(), i64::MAX);
        assert_eq!(row.try_get::<i64, _>(1).unwrap(), i64::MAX);
        assert_eq!(row.try_get::<i64, _>(2).unwrap(), i64::MAX);
        assert_eq!(row.try_get::<i64, _>(3).unwrap(), i64::MAX);
    }

    #[tokio::test]
    async fn records_are_isolated_per_project() {
        let pool = in_memory_pool().await;

        let rec_a = EvolutionRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            observations: 10,
            success_rate: 0.9,
            avg_score: 0.8,
            error_patterns: HashMap::new(),
            failure_patterns: vec![],
            skills_seeded: 1,
            skills_rolled_back: 0,
            total_evolved: 0,
            analysis_summary: "Project A".into(),
        };
        let rec_b = EvolutionRecord {
            observations: 99,
            ..rec_a.clone()
        };

        insert_record_pool(&pool, "project-a", &rec_a).await.unwrap();
        insert_record_pool(&pool, "project-b", &rec_b).await.unwrap();

        let a = query_recent_records_pool(&pool, "project-a", 10).await.unwrap();
        let b = query_recent_records_pool(&pool, "project-b", 10).await.unwrap();

        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(a[0].observations, 10);
        assert_eq!(b[0].observations, 99);
    }
}
