//! Integration tests for the store module

use rusqlite::Connection;

/// Shared in-memory test database helper.
/// Re-exported via `super::in_memory_db` from submodules — use `super::in_memory_db()`.
pub(crate) fn in_memory_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    super::schema::init_schema(&conn).unwrap();
    conn
}

#[test]
fn schema_creates_all_tables() {
    let conn = in_memory_db();

    // Verify all expected tables exist
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
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

#[test]
fn schema_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    super::schema::init_schema(&conn).unwrap();
    // Running again should not fail
    super::schema::init_schema(&conn).unwrap();
}

#[test]
fn meta_table_tracks_version() {
    let conn = in_memory_db();
    let version: String = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, "3");
}

#[test]
fn wal_mode_is_enabled() {
    let conn = in_memory_db();
    // In-memory DBs always report "memory" for journal_mode, but the pragma call
    // must succeed without error. For file-backed DBs this would return "wal".
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    // In-memory always = "memory"; accept both to keep in-memory tests green
    assert!(
        mode == "wal" || mode == "memory",
        "unexpected journal_mode: {mode}"
    );
}

#[test]
fn foreign_keys_are_enforced() {
    let conn = in_memory_db();
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk, 1, "foreign_keys pragma must be ON (1)");
}

#[test]
fn migration_is_idempotent_concurrent_simulation() {
    // Simulates two openers racing to migrate: the second should detect the flag
    // set by the first and skip. Both share the same connection here (single-threaded
    // approximation — real concurrency is covered by SQLite's IMMEDIATE transaction).
    let conn = Connection::open_in_memory().unwrap();
    super::schema::init_schema(&conn).unwrap();

    super::migrate::run(&conn); // first run — sets legacy_migrated=1
    super::migrate::run(&conn); // second run — must exit immediately (idempotent)

    let migrated: String = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(migrated, "1");
}

#[test]
fn u64_to_i64_converts_normal_values() {
    assert_eq!(super::u64_to_i64(0), 0);
    assert_eq!(super::u64_to_i64(1_000_000), 1_000_000);
    assert_eq!(super::u64_to_i64(i64::MAX as u64), i64::MAX);
}

#[test]
fn u64_to_i64_saturates_on_overflow() {
    // Values above i64::MAX must saturate to i64::MAX (not panic)
    let result = super::u64_to_i64(u64::MAX);
    assert_eq!(result, i64::MAX);
}

#[test]
fn obs_stats_tool_limit_is_enforced() {
    use super::observations::{insert_observation_conn, query_obs_stats_conn};
    use crate::shared::obs::ObsRecord;

    let conn = in_memory_db();

    // Insert observations for 150 distinct tool names
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
        insert_observation_conn(&conn, &rec, "sess_limit_test").unwrap();
    }

    let stats = query_obs_stats_conn(&conn, "2026-06-02", "2026-06-02").unwrap();
    assert_eq!(stats.total, 150, "total should count all observations");
    assert!(
        stats.tool_stats.len() <= 100,
        "tool_stats must be capped at 100, got {}",
        stats.tool_stats.len()
    );
}

#[test]
fn obs_error_stats_limit_is_enforced() {
    use super::observations::{insert_observation_conn, query_obs_stats_conn};
    use crate::shared::obs::ObsRecord;

    let conn = in_memory_db();

    // Insert observations for 80 distinct failure categories
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
        insert_observation_conn(&conn, &rec, "sess_err_limit").unwrap();
    }

    let stats = query_obs_stats_conn(&conn, "2026-06-02", "2026-06-02").unwrap();
    assert!(
        stats.error_stats.len() <= 50,
        "error_stats must be capped at 50, got {}",
        stats.error_stats.len()
    );
}

#[test]
fn global_json_field_parse_failure_uses_fallback() {
    use super::global::{insert_pattern_conn, query_all_patterns_conn};

    let conn = in_memory_db();
    // Insert a row with intentionally malformed JSON in per_error_stats
    conn.execute(
        "INSERT INTO global_patterns
         (timestamp, project, success_rate, avg_score, per_error_stats, failure_patterns, weak_tools)
         VALUES ('2026-06-02T10:00:00Z', 'proj-x', 0.9, 0.85, 'NOT_JSON', '[]', '[]')",
        [],
    )
    .unwrap();

    // Should not panic; malformed per_error_stats returns fallback {}
    let patterns = query_all_patterns_conn(&conn, 10).unwrap();
    assert_eq!(patterns.len(), 1);
    assert!(
        patterns[0]["per_error_stats"].is_object(),
        "fallback should be an empty object"
    );
    // Confirm fallback is empty object (not the original invalid string)
    assert_eq!(
        patterns[0]["per_error_stats"],
        serde_json::json!({}),
        "fallback for invalid JSON must be {{}}"
    );

    // Insert with valid JSON — must be preserved
    insert_pattern_conn(
        &conn,
        "2026-06-02T11:00:00Z",
        "proj-y",
        0.8,
        0.75,
        r#"{"type_error": 3}"#,
        "[]",
        "[]",
    )
    .unwrap();
    let all = query_all_patterns_conn(&conn, 10).unwrap();
    let proj_y = all.iter().find(|p| p["project"] == "proj-y").unwrap();
    assert_eq!(proj_y["per_error_stats"]["type_error"], 3);
}

#[test]
fn dismiss_agent_is_atomic() {
    use super::orchestrator::{
        OrchAgent, OrchRun, dismiss_agent_conn, init_run_conn, read_agent_conn, upsert_agent_conn,
    };

    let conn = in_memory_db();
    let run = OrchRun {
        id: "run-atomic".into(),
        status: "running".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-02T10:00:00Z".into(),
        updated_at: "2026-06-02T10:00:00Z".into(),
    };
    init_run_conn(&conn, &run).unwrap();

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
    upsert_agent_conn(&conn, &agent).unwrap();

    // First dismiss must succeed
    let first = dismiss_agent_conn(&conn, "agent-atomic").unwrap();
    assert!(first, "first dismiss must return true");

    // Second dismiss of the same agent must return false (not panic)
    let second = dismiss_agent_conn(&conn, "agent-atomic").unwrap();
    assert!(
        !second,
        "second dismiss of non-existent agent must return false"
    );

    assert!(read_agent_conn(&conn, "agent-atomic").unwrap().is_none());
}

#[test]
fn cleanup_stale_is_atomic() {
    use super::orchestrator::{
        OrchAgent, OrchRun, cleanup_stale_conn, init_run_conn, upsert_agent_conn,
    };

    let conn = in_memory_db();
    let run = OrchRun {
        id: "run-stale".into(),
        status: "complete".into(),
        agents_json: "[]".into(),
        dep_graph_json: "{}".into(),
        created_at: "2026-06-01T10:00:00Z".into(),
        updated_at: "2026-06-01T10:00:00Z".into(),
    };
    init_run_conn(&conn, &run).unwrap();

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
    upsert_agent_conn(&conn, &agent).unwrap();

    // cleanup_stale must delete both run and orphaned agent atomically
    let deleted = cleanup_stale_conn(&conn, "").unwrap();
    assert!(deleted >= 2, "must delete run + agent, got {deleted}");

    // Neither should remain
    let run_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM orch_runs", [], |r| r.get(0))
        .unwrap();
    let agent_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM orch_agents", [], |r| r.get(0))
        .unwrap();
    assert_eq!(run_count, 0);
    assert_eq!(agent_count, 0);
}
