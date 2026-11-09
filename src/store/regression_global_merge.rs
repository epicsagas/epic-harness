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
}
