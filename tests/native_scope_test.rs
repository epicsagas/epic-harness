#[path = "../src-tauri/src/commands/native_store.rs"]
mod native_store;

use sqlx::{AnyPool, any::AnyPoolOptions};

async fn pool() -> AnyPool {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    // Build the fixture from canonical DDL without changing process-global
    // harness paths or connecting to a user's operational database.
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
    for (project, count) in [("alpha", "2"), ("beta", "9")] {
        sqlx::query("INSERT INTO metrics_state (key,value,project) VALUES ('total_sessions',?,?)")
            .bind(count)
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO observations (timestamp,session_id,tool,tool_category,project,score) VALUES ('2026-11-09T10:00:00Z',?,'Read','read',?,1.0)")
            .bind(project).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO evolution_records (timestamp,observations,analysis_summary,project) VALUES ('2026-11-09T10:00:00Z',1,?,?)")
            .bind(project).bind(project).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO sessions (timestamp,summary,created_at_millis,project) VALUES ('2026-11-09T10:00:00Z',?,1,?)")
            .bind(project).bind(project).execute(&pool).await.unwrap();
    }
    pool
}

#[tokio::test]
async fn native_project_metrics_and_all_project_aggregate_are_distinct() {
    let db = pool().await;
    assert_eq!(
        native_store::metrics(&db, Some("alpha"))
            .await
            .unwrap()
            .total_sessions,
        2
    );
    assert_eq!(
        native_store::metrics(&db, Some("beta"))
            .await
            .unwrap()
            .total_sessions,
        9
    );
    assert_eq!(
        native_store::metrics(&db, None)
            .await
            .unwrap()
            .total_sessions,
        11
    );
}

#[tokio::test]
async fn native_project_observations_exclude_other_projects() {
    let db = pool().await;
    assert_eq!(
        native_store::observations(&db, Some("alpha"))
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        native_store::observations(&db, None).await.unwrap().total,
        2
    );
}

#[tokio::test]
async fn native_project_history_is_filtered_before_limit() {
    let db = pool().await;
    let rows = native_store::history(&db, 1, Some("alpha")).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].analysis_summary, "alpha");
}

#[tokio::test]
async fn native_project_snapshots_are_filtered_before_limit() {
    let db = pool().await;
    let rows = native_store::snapshots(&db, 1, Some("alpha"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].summary, "alpha");
    assert_eq!(
        native_store::snapshots(&db, 10, None).await.unwrap().len(),
        2
    );
}
