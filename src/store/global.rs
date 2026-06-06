//! global.rs — Cross-project pattern SQLite I/O

use std::io;

/// Parse a JSON field from a DB column, logging a warning on failure.
fn parse_json_field(raw: &str, fallback: serde_json::Value) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[store/global] JSON parse failed ({}): '{}' — using fallback",
                e,
                &raw[..raw.len().min(100)]
            );
            fallback
        }
    }
}

// ── Async pool functions ─────────────────────────────

use sqlx::{AnyPool, Row};

#[allow(clippy::too_many_arguments)]
pub async fn insert_pattern_pool(
    pool: &AnyPool,
    timestamp: &str,
    project: &str,
    success_rate: f64,
    avg_score: f64,
    per_error_stats_json: &str,
    failure_patterns_json: &str,
    weak_tools_json: &str,
) -> io::Result<i64> {
    let result = sqlx::query(
        "INSERT INTO global_patterns (timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools) VALUES (?1,?2,?3,?4,?5,?6,?7)"
    )
    .bind(timestamp)
    .bind(project)
    .bind(success_rate)
    .bind(avg_score)
    .bind(per_error_stats_json)
    .bind(failure_patterns_json)
    .bind(weak_tools_json)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    // AnyPool::last_insert_id() returns None for SQLite via sqlx any-driver.
    // No caller depends on a non-zero return — insert success is verified by the
    // absence of an error from .execute().
    Ok(result.last_insert_id().unwrap_or(0))
}

pub async fn query_patterns_excluding_pool(
    pool: &AnyPool,
    exclude_project: &str,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_pool_inner(pool, Some(exclude_project), limit).await
}

pub async fn query_all_patterns_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    query_patterns_pool_inner(pool, None, limit).await
}

async fn query_patterns_pool_inner(
    pool: &AnyPool,
    exclude_project: Option<&str>,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let rows = if let Some(proj) = exclude_project {
        sqlx::query(
            "SELECT timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools FROM global_patterns WHERE project != ?1 ORDER BY timestamp DESC LIMIT ?2"
        )
        .bind(proj)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools FROM global_patterns ORDER BY timestamp DESC LIMIT ?1"
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .map_err(crate::store::sqlx_err)?;

    let patterns: Vec<serde_json::Value> = rows.iter().map(|r| {
        let per_err: String = r.try_get(4).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: per_error_stats col missing ({e}) — using fallback");
            "{}".into()
        });
        let failure: String = r.try_get(5).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: failure_patterns col missing ({e}) — using fallback");
            "[]".into()
        });
        let weak: String = r.try_get(6).unwrap_or_else(|e| {
            eprintln!("[store/global] schema mismatch: weak_tools col missing ({e}) — using fallback");
            "[]".into()
        });
        serde_json::json!({
            "timestamp": r.try_get::<String, _>(0).unwrap_or_default(),
            "project": r.try_get::<String, _>(1).unwrap_or_default(),
            "success_rate": r.try_get::<f64, _>(2).unwrap_or(0.0),
            "avg_score": r.try_get::<f64, _>(3).unwrap_or(0.0),
            "per_error_stats": parse_json_field(&per_err, serde_json::json!({})),
            "failure_patterns": parse_json_field(&failure, serde_json::json!([])),
            "weak_tools": parse_json_field(&weak, serde_json::json!([])),
        })
    }).collect();
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> sqlx::AnyPool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_query() {
        let pool = in_memory_pool().await;
        insert_pattern_pool(
            &pool,
            "2026-06-02T10:00:00Z",
            "project-a",
            0.9,
            0.85,
            "{}",
            "[]",
            "[]",
        )
        .await
        .unwrap();

        let patterns = query_all_patterns_pool(&pool, 10).await.unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["project"], "project-a");
        assert!(patterns[0]["per_error_stats"].is_object());
        assert!(patterns[0]["failure_patterns"].is_array());
        assert!(patterns[0]["weak_tools"].is_array());
    }

    #[tokio::test]
    async fn query_excluding_project() {
        let pool = in_memory_pool().await;
        insert_pattern_pool(
            &pool,
            "2026-06-02T10:00:00Z",
            "project-a",
            0.9,
            0.85,
            "{}",
            "[]",
            "[]",
        )
        .await
        .unwrap();
        insert_pattern_pool(
            &pool,
            "2026-06-02T11:00:00Z",
            "project-b",
            0.8,
            0.75,
            "{}",
            "[]",
            "[]",
        )
        .await
        .unwrap();

        let patterns = query_patterns_excluding_pool(&pool, "project-a", 10)
            .await
            .unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["project"], "project-b");
    }
}
