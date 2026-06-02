//! Integration tests for the store module

use rusqlite::Connection;

fn in_memory_db() -> Connection {
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
        "_meta",
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
            "SELECT value FROM _meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, "1");
}
