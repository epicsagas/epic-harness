//! evolution.rs — Evolution record SQLite I/O
#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

use crate::shared::evolution::EvolutionRecord;

/// Durable SessionEnd replay boundary. Queue markers are disposable retention
/// data; this key is the source of truth for completed reflection work.
pub async fn reflection_completed_pool(
    pool: &AnyPool,
    session_id: &str,
    project: &str,
) -> io::Result<bool> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM reflection_sessions WHERE session_id = ? AND project = ? LIMIT 1",
    )
    .bind(session_id)
    .bind(project)
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(found.is_some())
}

pub async fn mark_reflection_completed_pool(
    pool: &AnyPool,
    session_id: &str,
    project: &str,
) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO reflection_sessions (session_id, project, completed_at)
         VALUES (?, ?, ?)
         ON CONFLICT (session_id, project) DO NOTHING",
    )
    .bind(session_id)
    .bind(project)
    .bind(crate::shared::helpers::now_iso())
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Insert an evolution record, scoped to `project`.
pub async fn insert_record_pool(
    pool: &AnyPool,
    rec: &EvolutionRecord,
    project: &str,
) -> io::Result<i64> {
    let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|_| "{}".into());
    let failure_json = serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|_| "[]".into());

    sqlx::query(
        "INSERT INTO evolution_records
         (timestamp, observations, success_rate, avg_score, error_patterns,
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
          analysis_summary, edit_type, project)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(project)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(id)
}

/// Insert a reflection record exactly once for its `(project, session_id)`.
/// The unique index is the durable completion checkpoint: a retry after the
/// SQLite commit reports `false` instead of creating a second record.
pub async fn insert_reflection_record_once_pool(
    pool: &AnyPool,
    rec: &EvolutionRecord,
    project: &str,
    session_id: &str,
) -> io::Result<bool> {
    let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|_| "{}".into());
    let failure_json = serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|_| "[]".into());
    let result = sqlx::query(
        "INSERT INTO evolution_records
         (timestamp, observations, success_rate, avg_score, error_patterns,
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved,
          analysis_summary, edit_type, session_id, project)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT (project, session_id) DO NOTHING",
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
    .bind(session_id)
    .bind(project)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() == 1)
}

/// Standalone insert.
pub fn insert_record(rec: &EvolutionRecord) -> io::Result<i64> {
    let project = crate::shared::paths::project_slug().to_string();
    super::runtime::block_on(async {
        let pool = super::pool::harness_pool().await?;
        insert_record_pool(&pool, rec, &project).await
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
            session_id: None,
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
            session_id: None,
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

/// Count evolution records without materializing history.
pub async fn count_records_scoped_pool(pool: &AnyPool, project: Option<&str>) -> io::Result<u64> {
    let count: i64 = if let Some(project) = project {
        sqlx::query_scalar("SELECT COUNT(*) FROM evolution_records WHERE project = ?")
            .bind(project)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM evolution_records")
            .fetch_one(pool)
            .await
    }
    .map_err(super::sqlx_err)?;
    Ok(count.max(0) as u64)
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
    async fn reflection_completion_survives_queue_marker_removal() {
        let pool = test_pool().await;
        assert!(
            !reflection_completed_pool(&pool, "session-a", "project-a")
                .await
                .unwrap()
        );

        mark_reflection_completed_pool(&pool, "session-a", "project-a")
            .await
            .unwrap();
        // Queue `.completed` files are disposable retention data. A duplicate
        // insert proves the SQLite key remains the replay boundary by itself.
        mark_reflection_completed_pool(&pool, "session-a", "project-a")
            .await
            .unwrap();

        assert!(
            reflection_completed_pool(&pool, "session-a", "project-a")
                .await
                .unwrap()
        );
        assert!(
            !reflection_completed_pool(&pool, "session-a", "project-b")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn reflection_record_replay_after_write_is_not_duplicated() {
        let pool = test_pool().await;
        let record = EvolutionRecord {
            session_id: None,
            timestamp: "2026-07-28T10:00:00Z".into(),
            observations: 3,
            success_rate: 1.0,
            avg_score: 1.0,
            error_patterns: HashMap::new(),
            failure_patterns: vec![],
            skills_seeded: 0,
            skills_rolled_back: 0,
            total_evolved: 0,
            analysis_summary: "test".into(),
            edit_type: crate::shared::evolution::EditType::AddSkill,
            manifests: vec![],
        };
        assert!(
            insert_reflection_record_once_pool(&pool, &record, "project-a", "session-a")
                .await
                .unwrap()
        );
        assert!(
            !insert_reflection_record_once_pool(&pool, &record, "project-a", "session-a")
                .await
                .unwrap()
        );
        assert_eq!(
            count_records_scoped_pool(&pool, Some("project-a"))
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn insert_and_query() {
        let pool = test_pool().await;
        let rec = EvolutionRecord {
            session_id: None,
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

        insert_record_pool(&pool, &rec, "test-project")
            .await
            .unwrap();

        let results = query_recent_records_pool(&pool, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observations, 42);
        assert_eq!(results[0].error_patterns.get("syntax_error"), Some(&3));
    }

    #[tokio::test]
    async fn edit_type_roundtrips() {
        // R1: edit_type must persist and read back for every variant.
        let pool = test_pool().await;
        for (i, ty) in crate::shared::evolution::EditType::all().iter().enumerate() {
            let rec = EvolutionRecord {
                session_id: None,
                timestamp: format!("2026-06-16T10:00:0{}Z", i),
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
            insert_record_pool(&pool, &rec, "test-project")
                .await
                .unwrap();
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

    #[tokio::test]
    async fn recent_history_and_count_are_project_scoped_and_bounded() {
        let pool = test_pool().await;
        for (project, count) in [("project-a", 3), ("project-b", 2)] {
            for i in 0..count {
                let rec = EvolutionRecord {
                    session_id: None,
                    timestamp: format!("2026-06-16T10:00:0{i}Z"),
                    observations: i,
                    success_rate: 0.5,
                    avg_score: 0.5,
                    error_patterns: HashMap::new(),
                    failure_patterns: vec![],
                    skills_seeded: 0,
                    skills_rolled_back: 0,
                    total_evolved: 0,
                    analysis_summary: String::new(),
                    edit_type: crate::shared::evolution::EditType::AddSkill,
                    manifests: vec![],
                };
                insert_record_pool(&pool, &rec, project).await.unwrap();
            }
        }

        let recent = query_recent_records_scoped_pool(&pool, 2, Some("project-a"))
            .await
            .unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(
            count_records_scoped_pool(&pool, Some("project-a"))
                .await
                .unwrap(),
            3
        );
    }
}
