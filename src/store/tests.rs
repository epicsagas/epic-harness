//! Integration tests for the store module

use sqlx::AnyPool;
use sqlx::Row;

async fn test_pool() -> AnyPool {
    let pool = super::pool::test_memory_pool().await;
    super::schema::init_schema_pool(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn schema_creates_all_tables() {
    let pool = test_pool().await;

    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(&pool)
        .await
        .unwrap();

    let tables: Vec<String> = rows.iter().filter_map(|r| r.try_get(0).ok()).collect();

    let expected = [
        "_harness_meta",
        "evolution_records",
        "global_patterns",
        "metrics_state",
        "observations",
        "orch_agent_events",
        "orch_agent_inbox",
        "orch_agents",
        "orch_control",
        "orch_runs",
        "orbit_pipelines",
        "promotion_counters",
        "score_history",
        "sessions",
        "skill_attribution",
        "workspace_manifest",
    ];

    for table in &expected {
        assert!(
            tables.iter().any(|t| t == *table),
            "Missing table: {}",
            table
        );
    }
    assert!(
        !tables.iter().any(|table| table == "evolved_skills"),
        "evolved skills are file-owned and must not get a new SQLite table"
    );
}

#[tokio::test]
async fn schema_is_idempotent() {
    let pool = super::pool::test_memory_pool().await;
    super::schema::init_schema_pool(&pool).await.unwrap();
    // Running again should not fail
    super::schema::init_schema_pool(&pool).await.unwrap();
}

#[tokio::test]
async fn meta_table_tracks_version() {
    let pool = test_pool().await;
    let version: String =
        sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'schema_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, super::schema::SCHEMA_VERSION.to_string());
}

#[tokio::test]
async fn u64_to_i64_converts_normal_values() {
    assert_eq!(super::u64_to_i64(0), 0);
    assert_eq!(super::u64_to_i64(1_000_000), 1_000_000);
    assert_eq!(super::u64_to_i64(i64::MAX as u64), i64::MAX);
}

#[tokio::test]
async fn u64_to_i64_saturates_on_overflow() {
    let result = super::u64_to_i64(u64::MAX);
    assert_eq!(result, i64::MAX);
}

#[tokio::test]
async fn obs_stats_tool_limit_is_enforced() {
    use super::observations::{insert_observation_pool, query_obs_stats_pool};
    use crate::shared::obs::ObsRecord;

    let pool = test_pool().await;

    for i in 0..150usize {
        let rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: format!("tool_{i:03}"),
            tool_category: "bash".into(),
            action: None,
            result: Some("success".into()),
            score: Some(0.9),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            tool_use_id: None,
            pipeline_id: None,
        };
        insert_observation_pool(&pool, &rec, "sess_limit_test", "test-project")
            .await
            .unwrap();
    }

    let stats = query_obs_stats_pool(&pool, "2026-06-02", "2026-06-02")
        .await
        .unwrap();
    assert_eq!(stats.total, 150, "total should count all observations");
    assert!(
        stats.tool_stats.len() <= 100,
        "tool_stats must be capped at 100, got {}",
        stats.tool_stats.len()
    );
}

#[tokio::test]
async fn obs_error_stats_limit_is_enforced() {
    use super::observations::{insert_observation_pool, query_obs_stats_pool};
    use crate::shared::obs::ObsRecord;

    let pool = test_pool().await;

    for i in 0..80usize {
        let rec = ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: "Bash".into(),
            tool_category: "bash".into(),
            action: None,
            result: Some("error".into()),
            score: Some(0.3),
            dimensions: None,
            failure_category: Some(format!("error_cat_{i:03}")),
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            tool_use_id: None,
            pipeline_id: None,
        };
        insert_observation_pool(&pool, &rec, "sess_err_limit", "test-project")
            .await
            .unwrap();
    }

    let stats = query_obs_stats_pool(&pool, "2026-06-02", "2026-06-02")
        .await
        .unwrap();
    assert!(
        stats.error_stats.len() <= 50,
        "error_stats must be capped at 50, got {}",
        stats.error_stats.len()
    );
}

#[tokio::test]
async fn global_json_field_parse_failure_uses_fallback() {
    use super::global::{insert_pattern_pool, query_all_patterns_pool};

    let pool = test_pool().await;
    // Insert a row with intentionally malformed JSON via raw SQL
    sqlx::query(
        "INSERT INTO global_patterns
         (timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools)
         VALUES ('2026-06-02T10:00:00Z', 'proj-x', 0.9, 0.85, 'NOT_JSON', '[]', '[]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let patterns = query_all_patterns_pool(&pool, 10).await.unwrap();
    assert_eq!(patterns.len(), 1);
    assert!(
        patterns[0]["per_error_stats"].is_object(),
        "fallback should be an empty object"
    );
    assert_eq!(
        patterns[0]["per_error_stats"],
        serde_json::json!({}),
        "fallback for invalid JSON must be {{}}"
    );

    insert_pattern_pool(
        &pool,
        "2026-06-02T11:00:00Z",
        "proj-y",
        0.8,
        0.75,
        r#"{"type_error": 3}"#,
        "[]",
        "[]",
    )
    .await
    .unwrap();
    let all = query_all_patterns_pool(&pool, 10).await.unwrap();
    let proj_y = all.iter().find(|p| p["project"] == "proj-y").unwrap();
    assert_eq!(proj_y["per_error_stats"]["type_error"], 3);
}

#[tokio::test]
async fn dismiss_agent_is_atomic() {
    use super::orchestrator::{
        OrchAgent, OrchRun, dismiss_agent_pool, init_run_pool, read_agent_pool, upsert_agent_pool,
    };

    let pool = test_pool().await;
    let run = OrchRun {
        id: "run-atomic".into(),
        status: "running".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-02T10:00:00Z".into(),
        updated_at: "2026-06-02T10:00:00Z".into(),
    };
    init_run_pool(&pool, &run).await.unwrap();

    let agent = OrchAgent {
        id: "agent-atomic".into(),
        run_id: "run-atomic".into(),
        role: "worker".into(),
        task: "do work".into(),
        satisfies_json: "[]".into(),
        status: "running".into(),
        phase: "exec".into(),
        progress: 0.0,
        last_heartbeat: "2026-06-02T10:00:00Z".into(),
        started_at: None,
        completed_at: None,
    };
    upsert_agent_pool(&pool, &agent).await.unwrap();

    let first = dismiss_agent_pool(&pool, "agent-atomic").await.unwrap();
    assert!(first, "first dismiss must return true");

    let second = dismiss_agent_pool(&pool, "agent-atomic").await.unwrap();
    assert!(
        !second,
        "second dismiss of non-existent agent must return false"
    );

    assert!(
        read_agent_pool(&pool, "agent-atomic")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn cleanup_stale_is_atomic() {
    use super::orchestrator::{
        OrchAgent, OrchRun, cleanup_stale_pool, init_run_pool, upsert_agent_pool,
    };

    let pool = test_pool().await;
    let run = OrchRun {
        id: "run-stale".into(),
        status: "complete".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-01T10:00:00Z".into(),
        updated_at: "2026-06-01T10:00:00Z".into(),
    };
    init_run_pool(&pool, &run).await.unwrap();

    let agent = OrchAgent {
        id: "agent-stale".into(),
        run_id: "run-stale".into(),
        role: "worker".into(),
        task: "done".into(),
        satisfies_json: "[]".into(),
        status: "complete".into(),
        phase: "done".into(),
        progress: 1.0,
        last_heartbeat: "2026-06-01T10:00:00Z".into(),
        started_at: None,
        completed_at: None,
    };
    upsert_agent_pool(&pool, &agent).await.unwrap();

    let deleted = cleanup_stale_pool(&pool, "").await.unwrap();
    assert!(deleted >= 2, "must delete run + agent, got {deleted}");

    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orch_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let agent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orch_agents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run_count, 0);
    assert_eq!(agent_count, 0);
}
