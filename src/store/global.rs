//! global.rs — Cross-project pattern SQLite I/O
#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
use std::io;

/// Insert a global pattern record.
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
    sqlx::query(
        "INSERT INTO global_patterns
         (timestamp, project, success_rate, avg_score, per_error_stats,
          failure_patterns, weak_tools)
         VALUES (?,?,?,?,?,?,?)",
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
    .map_err(super::sqlx_err)?;

    let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(id)
}

/// Insert a reflection's global export at most once for one project.
///
/// Legacy rows use an empty `session_id` and remain queryable, but new
/// reflection writes must carry a stable session identity so replay cannot
/// duplicate cross-project evidence.
#[allow(clippy::too_many_arguments)]
pub async fn insert_pattern_once_pool(
    pool: &AnyPool,
    session_id: &str,
    timestamp: &str,
    project: &str,
    success_rate: f64,
    avg_score: f64,
    per_error_stats_json: &str,
    failure_patterns_json: &str,
    weak_tools_json: &str,
) -> io::Result<bool> {
    if session_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "global pattern export requires a reflection session id",
        ));
    }
    let result = sqlx::query(
        "INSERT INTO global_patterns
         (timestamp, project, session_id, success_rate, avg_score, per_error_stats,
          failure_patterns, weak_tools)
         VALUES (?,?,?,?,?,?,?,?)
         ON CONFLICT(project, session_id) WHERE session_id <> '' DO NOTHING",
    )
    .bind(timestamp)
    .bind(project)
    .bind(session_id)
    .bind(success_rate)
    .bind(avg_score)
    .bind(per_error_stats_json)
    .bind(failure_patterns_json)
    .bind(weak_tools_json)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

/// Query patterns for all projects except the given one.
pub async fn query_patterns_excluding_pool(
    pool: &AnyPool,
    exclude_project: &str,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT timestamp, project, success_rate, avg_score,
                per_error_stats, failure_patterns, weak_tools
         FROM global_patterns
         WHERE project != ?
         ORDER BY timestamp DESC LIMIT ?",
    )
    .bind(exclude_project)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    Ok(rows.iter().map(row_to_pattern).collect())
}

/// Query all patterns (regardless of project).
pub async fn query_all_patterns_pool(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT timestamp, project, success_rate, avg_score,
                per_error_stats, failure_patterns, weak_tools
         FROM global_patterns ORDER BY timestamp DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    Ok(rows.iter().map(row_to_pattern).collect())
}

/// Convert a row to a JSON value.
fn row_to_pattern(r: &sqlx::any::AnyRow) -> serde_json::Value {
    let ts: String = r.try_get(0).unwrap_or_default();
    let project: String = r.try_get(1).unwrap_or_default();
    let success_rate: f64 = r.try_get(2).unwrap_or(0.0);
    let avg_score: f64 = r.try_get(3).unwrap_or(0.0);
    let per_error_raw: String = r.try_get(4).unwrap_or_else(|_| "{}".into());
    let failure_raw: String = r.try_get(5).unwrap_or_else(|_| "[]".into());
    let weak_raw: String = r.try_get(6).unwrap_or_else(|_| "[]".into());

    let per_error_stats = parse_json_field(&per_error_raw, serde_json::json!({}));
    let failure_patterns = parse_json_field(&failure_raw, serde_json::json!([]));
    let weak_tools = parse_json_field(&weak_raw, serde_json::json!([]));

    serde_json::json!({
        "timestamp": ts,
        "project": project,
        "success_rate": success_rate,
        "avg_score": avg_score,
        "per_error_stats": per_error_stats,
        "failure_patterns": failure_patterns,
        "weak_tools": weak_tools,
    })
}

/// Parse a JSON field from a DB column, logging a warning on failure.
fn parse_json_field(raw: &str, fallback: serde_json::Value) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[store/global] JSON parse failed ({}): '{}' — using fallback",
                e,
                crate::shared::sanitize::truncate_utf8(raw, 100)
            );
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_query() {
        let pool = test_pool().await;
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
    async fn session_scoped_insert_is_exactly_once_and_rejects_empty_identity() {
        let pool = test_pool().await;
        assert!(
            insert_pattern_once_pool(
                &pool,
                "session-a",
                "2026-06-02T10:00:00Z",
                "project-a",
                0.9,
                0.85,
                "{}",
                "[]",
                "[]",
            )
            .await
            .unwrap()
        );
        assert!(
            !insert_pattern_once_pool(
                &pool,
                "session-a",
                "2026-06-02T10:00:01Z",
                "project-a",
                0.8,
                0.75,
                "{}",
                "[]",
                "[]",
            )
            .await
            .unwrap()
        );
        assert!(
            insert_pattern_once_pool(
                &pool,
                "",
                "2026-06-02T10:00:02Z",
                "project-a",
                0.8,
                0.75,
                "{}",
                "[]",
                "[]",
            )
            .await
            .is_err()
        );

        assert_eq!(query_all_patterns_pool(&pool, 10).await.unwrap().len(), 1);
    }

    #[test]
    fn malformed_json_diagnostic_is_utf8_safe() {
        let raw = format!("{}日", "x".repeat(99));
        assert_eq!(
            parse_json_field(&raw, serde_json::json!({"fallback": true})),
            serde_json::json!({"fallback": true})
        );
    }

    #[tokio::test]
    async fn query_excluding_project() {
        let pool = test_pool().await;
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
