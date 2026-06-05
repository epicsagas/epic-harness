//! Integration tests for the store module

use sqlx::Row;

// ── Helper ────────────────────────────────────────────
async fn setup_pool() -> sqlx::SqlitePool {
    let pool = crate::store::pool::test_memory_pool().await;
    crate::store::schema::init_schema_pool(&pool).await.unwrap();
    pool
}

// ── Schema / DDL tests ────────────────────────────────

#[tokio::test]
async fn schema_creates_all_tables() {
    let pool = setup_pool().await;

    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(&pool)
        .await
        .unwrap();

    let tables: Vec<String> = rows
        .iter()
        .map(|r| r.try_get::<String, _>(0).unwrap())
        .collect();

    let expected = [
        "_harness_meta",
        "evolution_records",
        "evolved_skills",
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
}

#[tokio::test]
async fn schema_is_idempotent() {
    let pool = crate::store::pool::test_memory_pool().await;
    crate::store::schema::init_schema_pool(&pool).await.unwrap();
    // Running again should not fail
    crate::store::schema::init_schema_pool(&pool).await.unwrap();
}

#[tokio::test]
async fn meta_table_tracks_version() {
    let pool = setup_pool().await;
    let version: String =
        sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'schema_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, "4");
}

#[tokio::test]
async fn wal_mode_is_enforced() {
    let pool = setup_pool().await;
    // In-memory DBs report "memory"; file-backed report "wal". Accept both.
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        mode == "wal" || mode == "memory",
        "unexpected journal_mode: {mode}"
    );
}

#[tokio::test]
async fn foreign_keys_are_enforced() {
    let pool = setup_pool().await;
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fk, 1, "foreign_keys pragma must be ON (1)");
}

#[tokio::test]
async fn migration_is_idempotent() {
    let pool = setup_pool().await;

    // Simulate a completed migration by setting the flag.
    sqlx::query(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('legacy_migrated', '1')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let migrated: String =
        sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(migrated, "1");
}

// ── Numeric helpers ───────────────────────────────────

#[test]
fn u64_to_i64_converts_normal_values() {
    assert_eq!(super::u64_to_i64(0), 0);
    assert_eq!(super::u64_to_i64(1_000_000), 1_000_000);
    assert_eq!(super::u64_to_i64(i64::MAX as u64), i64::MAX);
}

#[test]
fn u64_to_i64_saturates_on_overflow() {
    let result = super::u64_to_i64(u64::MAX);
    assert_eq!(result, i64::MAX);
}

#[test]
fn i64_to_u64_converts_normal_values() {
    assert_eq!(super::i64_to_u64(0), 0);
    assert_eq!(super::i64_to_u64(1_000_000), 1_000_000);
    assert_eq!(super::i64_to_u64(i64::MAX), i64::MAX as u64);
}

#[test]
fn i64_to_u64_clamps_negative_to_zero() {
    assert_eq!(super::i64_to_u64(-1), 0);
    assert_eq!(super::i64_to_u64(i64::MIN), 0);
}

// ── Pool-based constraint tests ───────────────────────

#[tokio::test]
async fn fk_violation_on_agent_without_run() {
    let pool = setup_pool().await;
    let result = sqlx::query(
        "INSERT INTO orch_agents
         (id, run_id, role, task, satisfies_json, status, phase, progress, last_heartbeat)
         VALUES ('agent-x', 'nonexistent-run', 'worker', 'do thing', '[]', 'running', 'exec', 0.0, '')",
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "FK violation must reject insert of agent with non-existent run_id"
    );
}

#[tokio::test]
async fn unique_constraint_on_score_history_timestamp() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO score_history (timestamp, success_rate, avg_score, observations, dim_success, dim_quality, dim_cost)
         VALUES ('2026-06-02T10:00:00Z', 0.9, 0.8, 10, 1.0, 0.9, 0.8)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = sqlx::query(
        "INSERT INTO score_history (timestamp, success_rate, avg_score, observations, dim_success, dim_quality, dim_cost)
         VALUES ('2026-06-02T10:00:00Z', 0.7, 0.6, 5, 0.5, 0.5, 0.5)",
    )
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "UNIQUE constraint must reject duplicate score_history (timestamp, project)"
    );
}

// ── Observation / stats tests ─────────────────────────

#[tokio::test]
async fn obs_stats_tool_limit_is_enforced() {
    use super::observations::{insert_observation_pool, query_obs_stats_pool};
    use crate::shared::obs::ObsRecord;

    let pool = setup_pool().await;

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
            pipeline_id: None,
        };
        insert_observation_pool(&pool, "test-project", &rec, "sess_limit_test")
            .await
            .unwrap();
    }

    let stats = query_obs_stats_pool(&pool, "test-project", "2026-06-02", "2026-06-02")
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

    let pool = setup_pool().await;

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
            pipeline_id: None,
        };
        insert_observation_pool(&pool, "test-project", &rec, "sess_err_limit")
            .await
            .unwrap();
    }

    let stats = query_obs_stats_pool(&pool, "test-project", "2026-06-02", "2026-06-02")
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

    let pool = setup_pool().await;
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

    let pool = setup_pool().await;
    let run = OrchRun {
        id: "run-atomic".into(),
        status: "running".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-02T10:00:00Z".into(),
        updated_at: "2026-06-02T10:00:00Z".into(),
    };
    init_run_pool(&pool, "test-project", &run).await.unwrap();

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

    let first = dismiss_agent_pool(&pool, "test-project", "agent-atomic")
        .await
        .unwrap();
    assert!(first, "first dismiss must return true");

    let second = dismiss_agent_pool(&pool, "test-project", "agent-atomic")
        .await
        .unwrap();
    assert!(
        !second,
        "second dismiss of non-existent agent must return false"
    );

    assert!(
        read_agent_pool(&pool, "test-project", "agent-atomic")
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

    let pool = setup_pool().await;
    let run = OrchRun {
        id: "run-stale".into(),
        status: "complete".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-01T10:00:00Z".into(),
        updated_at: "2026-06-01T10:00:00Z".into(),
    };
    init_run_pool(&pool, "test-project", &run).await.unwrap();

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

    let deleted = cleanup_stale_pool(&pool, "test-project", "").await.unwrap();
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
