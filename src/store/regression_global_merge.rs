use super::*;
use sqlx::Connection;

#[tokio::test]
async fn issue_183_global_transfer_error_rolls_back_and_reports_failure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("global.db");
    let mut conn = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(super::super::schema::DDL_SQLITE)
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO sessions(timestamp,summary,created_at_millis,project) VALUES('fixture','preserve',1,'source'); CREATE TRIGGER stop_transfer BEFORE UPDATE ON evolution_records BEGIN SELECT RAISE(ABORT,'synthetic transfer failure'); END; INSERT INTO evolution_records(timestamp,project) VALUES('fixture','source');").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    assert!(
        merge_global_db(&path, "source", "target", false)
            .await
            .is_err()
    );
    let mut conn = SqliteConnectOptions::new()
        .filename(&path)
        .connect()
        .await
        .unwrap();
    let project: String = sqlx::query_scalar("SELECT project FROM sessions")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        project, "source",
        "earlier table transfers must roll back after an error"
    );
    let cursor: Option<String> = sqlx::query_scalar(
        "SELECT value FROM metrics_state WHERE key='reflection_cursor' AND project='target'",
    )
    .fetch_optional(&mut conn)
    .await
    .unwrap();
    assert!(
        cursor.is_none(),
        "new cursor creation rolls back with a later table error"
    );
}

async fn legacy_project_database(path: &Path) {
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(super::super::schema::DDL_SQLITE)
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO observations(id,timestamp,session_id,tool,tool_category,action,result,score,project) VALUES(900,'2024-01-01T00:00:00Z','fresh-import','Read','read','fresh-source','success',1,'source'),(1,'2023-01-01T00:00:00Z','pending-target','Read','read','pending-target','success',1,'target');").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
}

#[tokio::test]
async fn issue_218_global_merge_initializes_absent_cursor_without_changing_legacy_dates() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("global.db");
    legacy_project_database(&path).await;
    merge_global_db(&path, "source", "target", false)
        .await
        .unwrap();
    let mut conn = SqliteConnectOptions::new()
        .filename(&path)
        .connect()
        .await
        .unwrap();
    let cursor: Option<String> = sqlx::query_scalar(
        "SELECT value FROM metrics_state WHERE key='reflection_cursor' AND project='target'",
    )
    .fetch_optional(&mut conn)
    .await
    .unwrap();
    assert_eq!(
        cursor.as_deref(),
        Some("0"),
        "combined streams must not fall back to an older project's timestamp cutoff"
    );
    let timestamps: Vec<String> = sqlx::query_scalar(
        "SELECT timestamp FROM observations WHERE project='target' ORDER BY timestamp",
    )
    .fetch_all(&mut conn)
    .await
    .unwrap();
    assert_eq!(
        timestamps,
        vec!["2023-01-01T00:00:00Z", "2024-01-01T00:00:00Z"]
    );
    let source_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM observations WHERE project='source'")
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert_eq!(source_rows, 0);
}

#[tokio::test]
async fn issue_218_global_merge_cursor_creation_failure_retains_both_streams() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("global.db");
    legacy_project_database(&path).await;
    let mut conn = SqliteConnectOptions::new()
        .filename(&path)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_cursor BEFORE INSERT ON metrics_state WHEN NEW.key='reflection_cursor' BEGIN SELECT RAISE(ABORT,'synthetic cursor creation failure'); END;").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    assert!(
        merge_global_db(&path, "source", "target", false)
            .await
            .is_err(),
        "cursor creation failure must fail the merge"
    );
    let mut conn = SqliteConnectOptions::new()
        .filename(&path)
        .connect()
        .await
        .unwrap();
    let projects: Vec<String> =
        sqlx::query_scalar("SELECT project FROM observations ORDER BY project")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(
        projects,
        vec!["source", "target"],
        "both original streams remain intact"
    );
}
