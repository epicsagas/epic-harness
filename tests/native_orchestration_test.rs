#[test]
fn native_orchestration_exports_have_registered_handlers() {
    let registry = include_str!("../src-tauri/src/lib.rs");
    for command in [
        "get_orchestrator_run",
        "get_orchestrator_agent_status",
        "get_agent_events",
        "get_agent_inbox",
        "dismiss_agent",
        "dismiss_orbit_pipeline",
    ] {
        assert!(
            registry.contains(&format!("commands::harness::{command},")),
            "native invoke {command} must have a registered handler"
        );
    }
}
#[path = "../src-tauri/src/commands/native_files.rs"]
#[allow(dead_code)]
mod native_files;
#[path = "../src-tauri/src/commands/native_orchestration.rs"]
mod native_orchestration;

async fn fixture() -> (sqlx::AnyPool, tempfile::TempDir) {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let source = include_str!("../src/store/schema.rs");
    let ddl = source
        .split("const DDL_SQLITE: &str = r#\"")
        .nth(1)
        .unwrap()
        .split("\"#;")
        .next()
        .unwrap();
    for statement in ddl.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
            .unwrap();
    }
    use epic_harness::store::orchestrator::*;
    init_run_pool(
        &pool,
        &OrchRun {
            id: "run-a".into(),
            status: "running".into(),
            agents_json: "[{\"id\":\"agent-a\"}]".into(),
            dep_graph_json: "{}".into(),
            created_at: "2026-11-09".into(),
            updated_at: "2026-11-09".into(),
        },
    )
    .await
    .unwrap();
    upsert_agent_pool(
        &pool,
        &OrchAgent {
            id: "agent-a".into(),
            run_id: "run-a".into(),
            role: "tester".into(),
            task: "fixture".into(),
            satisfies_json: "[]".into(),
            status: "running".into(),
            phase: "test".into(),
            progress: 0.5,
            last_heartbeat: "2026-11-09".into(),
            started_at: None,
            completed_at: None,
        },
    )
    .await
    .unwrap();
    append_event_pool(
        &pool,
        "agent-a",
        "2026-11-09",
        "tool",
        "{\"tool\":\"Read\"}",
    )
    .await
    .unwrap();
    post_inbox_pool(
        &pool,
        "agent-a",
        "reviewer",
        "2026-11-09",
        "synthetic message",
    )
    .await
    .unwrap();
    (pool, tempfile::tempdir().unwrap())
}

#[tokio::test]
async fn native_orchestration_returns_decoded_persistent_wire_values() {
    let (pool, root) = fixture().await;
    let run = native_orchestration::run(&pool, root.path(), false)
        .await
        .unwrap();
    assert_eq!(run["agents"][0]["id"], "agent-a");
    assert!(run["dependency_graph"].is_object());
    assert_eq!(
        native_orchestration::status(&pool, root.path(), false, "agent-a")
            .await
            .unwrap()["phase"],
        "test"
    );
    assert_eq!(
        native_orchestration::events(&pool, root.path(), false, "agent-a")
            .await
            .unwrap()[0]["data"]["tool"],
        "Read"
    );
    assert_eq!(
        native_orchestration::inbox(&pool, root.path(), false, "agent-a")
            .await
            .unwrap()[0]["from"],
        "reviewer"
    );
}

#[tokio::test]
async fn selected_native_project_does_not_leak_another_runs_agents() {
    let (pool, root) = fixture().await;
    assert!(
        native_orchestration::run(&pool, root.path(), true)
            .await
            .unwrap()
            .is_null()
    );
    assert!(
        native_orchestration::status(&pool, root.path(), true, "agent-a")
            .await
            .unwrap()
            .is_null()
    );
    assert!(
        native_orchestration::events(&pool, root.path(), true, "agent-a")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        native_orchestration::inbox(&pool, root.path(), true, "agent-a")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        native_orchestration::status(&pool, root.path(), false, "../agent-a")
            .await
            .is_err()
    );
    std::fs::create_dir(root.path().join("orchestrator")).unwrap();
    std::fs::write(
        root.path().join("orchestrator/run.json"),
        "{\"id\":\"run-a\",\"agents\":[]}",
    )
    .unwrap();
    assert_eq!(
        native_orchestration::status(&pool, root.path(), true, "agent-a")
            .await
            .unwrap()["agent_id"],
        "agent-a"
    );
}
