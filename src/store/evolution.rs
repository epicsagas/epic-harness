//! evolution.rs — Evolution record SQLite I/O
#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

use crate::shared::evolution::EvolutionRecord;

/// Insert an evolution record.
pub async fn insert_record_pool(pool: &AnyPool, rec: &EvolutionRecord) -> io::Result<i64> {
    let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|_| "{}".into());
    let failure_json = serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|_| "[]".into());

    sqlx::query(
        "INSERT INTO evolution_records
         (timestamp, observations, success_rate, avg_score, error_patterns,
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary)
         VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&rec.timestamp)
    .bind(super::u64_to_i64(rec.observations))
    .bind(rec.success_rate)
    .bind(rec.avg_score)
    .bind(&error_json)
    .bind(&failure_json)
    .bind(super::u64_to_i64(rec.skills_seeded))
    .bind(super::u64_to_i64(rec.skills_rolled_back))
    .bind(super::u64_to_i64(rec.total_evolved))
    .bind(&rec.analysis_summary)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(id)
}

/// Standalone insert.
pub fn insert_record(rec: &EvolutionRecord) -> io::Result<i64> {
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        insert_record_pool(&pool, rec).await
    })
}

/// Query the N most recent evolution records.
pub async fn query_recent_records_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<EvolutionRecord>> {
    let rows = sqlx::query(
        "SELECT timestamp, observations, success_rate, avg_score, error_patterns,
                failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary
         FROM evolution_records ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut records = Vec::with_capacity(rows.len());
    for r in rows {
        let error_json: String = r.try_get(4).map_err(super::sqlx_err)?;
        let failure_json: String = r.try_get(5).map_err(super::sqlx_err)?;
        records.push(EvolutionRecord {
            timestamp: r.try_get(0).map_err(super::sqlx_err)?,
            observations: r.try_get::<i64, _>(1).map_err(super::sqlx_err)? as u64,
            success_rate: r.try_get(2).map_err(super::sqlx_err)?,
            avg_score: r.try_get(3).map_err(super::sqlx_err)?,
            error_patterns: serde_json::from_str(&error_json).unwrap_or_default(),
            failure_patterns: serde_json::from_str(&failure_json).unwrap_or_default(),
            skills_seeded: r.try_get::<i64, _>(6).map_err(super::sqlx_err)? as u64,
            skills_rolled_back: r.try_get::<i64, _>(7).map_err(super::sqlx_err)? as u64,
            total_evolved: r.try_get::<i64, _>(8).map_err(super::sqlx_err)? as u64,
            analysis_summary: r.try_get(9).map_err(super::sqlx_err)?,
            edit_type: crate::shared::evolution::EditType::AddSkill,
        });
    }
    records.reverse();
    Ok(records)
}

/// Query all evolution records.
pub async fn query_all_records_pool(pool: &AnyPool) -> io::Result<Vec<EvolutionRecord>> {
    query_recent_records_pool(pool, i64::MAX).await
}

/// Alias: query recent records without project filter (all projects).
pub async fn query_recent_records_all_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<EvolutionRecord>> {
    query_recent_records_pool(pool, limit).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_query() {
        let pool = test_pool().await;
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

        insert_record_pool(&pool, &rec).await.unwrap();

        let results = query_recent_records_pool(&pool, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observations, 42);
        assert_eq!(results[0].error_patterns.get("syntax_error"), Some(&3));
    }
}
