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
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
          analysis_summary, edit_type)
         VALUES (?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(rec.edit_type.as_str())
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
                failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
                analysis_summary, edit_type
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
        let edit_type_str: String = r.try_get(10).map_err(super::sqlx_err)?;
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
            edit_type: crate::shared::evolution::EditType::from_db_str(&edit_type_str),
            // Manifests are persisted to the JSONL sidecar, not the SQLite
            // scalar columns; reads from SQLite leave them empty.
            manifests: vec![],
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

/// Project-scoped recent records. `Some(p)` adds `WHERE project = ?`;
/// `None` behaves like the unfiltered variant. Same row decoding as
/// query_recent_records_pool (static-SQL two-branch form because sqlx 0.9's
/// SqlSafeStr rejects dynamically-built strings).
pub async fn query_recent_records_scoped_pool(
    pool: &AnyPool,
    limit: i64,
    project: Option<&str>,
) -> io::Result<Vec<EvolutionRecord>> {
    let rows = if let Some(p) = project {
        sqlx::query(
            "SELECT timestamp, observations, success_rate, avg_score, error_patterns,
                    failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
                    analysis_summary, edit_type
             FROM evolution_records WHERE project = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(p)
        .bind(limit)
    } else {
        sqlx::query(
            "SELECT timestamp, observations, success_rate, avg_score, error_patterns,
                    failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
                    analysis_summary, edit_type
             FROM evolution_records ORDER BY id DESC LIMIT ?",
        )
        .bind(limit)
    }
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut records = Vec::with_capacity(rows.len());
    for r in rows {
        let error_json: String = r.try_get(4).map_err(super::sqlx_err)?;
        let failure_json: String = r.try_get(5).map_err(super::sqlx_err)?;
        let edit_type_str: String = r.try_get(10).map_err(super::sqlx_err)?;
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
            edit_type: crate::shared::evolution::EditType::from_db_str(&edit_type_str),
            manifests: vec![],
        });
    }
    records.reverse();
    Ok(records)
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
            edit_type: crate::shared::evolution::EditType::AddSkill,
            manifests: vec![],
        };

        insert_record_pool(&pool, &rec).await.unwrap();

        let results = query_recent_records_pool(&pool, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observations, 42);
        assert_eq!(results[0].error_patterns.get("syntax_error"), Some(&3));
    }

    #[tokio::test]
    async fn edit_type_roundtrips() {
        // R1: edit_type must persist and read back for every variant.
        let pool = test_pool().await;
        for ty in crate::shared::evolution::EditType::all() {
            let rec = EvolutionRecord {
                timestamp: format!("2026-06-16T10:00:0{}Z", ty.as_str().len()),
                observations: 1,
                success_rate: 0.5,
                avg_score: 0.5,
                error_patterns: HashMap::new(),
                failure_patterns: vec![],
                skills_seeded: 0,
                skills_rolled_back: 0,
                total_evolved: 0,
                analysis_summary: String::new(),
                edit_type: ty.clone(),
                manifests: vec![],
            };
            insert_record_pool(&pool, &rec).await.unwrap();
        }

        let results = query_recent_records_pool(&pool, 100).await.unwrap();
        assert_eq!(
            results.len(),
            crate::shared::evolution::EditType::all().len()
        );
        // Every persisted edit type must survive the round trip.
        for r in &results {
            let expected = crate::shared::evolution::EditType::from_db_str(r.edit_type.as_str());
            assert_eq!(
                expected,
                r.edit_type,
                "edit_type round-trip failed for {}",
                r.edit_type.as_str()
            );
        }
    }
}
