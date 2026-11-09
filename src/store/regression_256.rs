use super::*;
use sqlx::Connection;

async fn database(path: &std::path::Path) -> sqlx::SqliteConnection {
    let mut c = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(crate::store::schema::DDL_SQLITE)
        .execute(&mut c)
        .await
        .unwrap();
    c
}

#[tokio::test]
async fn issue_256_retries_keep_distinct_source_rows_without_reinserting_them() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    let mut s = database(&source).await;
    sqlx::raw_sql("INSERT INTO sessions(timestamp,summary,created_at_millis) VALUES('2026-11-09','identical legitimate snapshot',1),('2026-11-09','identical legitimate snapshot',1);
      INSERT INTO evolution_records(timestamp) VALUES('2026-11-09');
      INSERT INTO score_history(timestamp,success_rate,avg_score) VALUES('2026-11-09',0.7,0.8);").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    assert_eq!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .unwrap()
            .sessions,
        2
    );
    let retried = merge_attached_db_async(&mut target, "target", "src")
        .await
        .unwrap();
    assert_eq!(retried.sessions, 0);
    for (table, expected) in [
        ("sessions", 2),
        ("evolution_records", 1),
        ("score_history", 1),
    ] {
        let count: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
                .fetch_one(&mut target)
                .await
                .unwrap();
        assert_eq!(count, expected, "{table} retry must not duplicate history");
    }
    let mut s = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&source)
        .connect()
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(timestamp,summary,created_at_millis) VALUES('2026-11-10','new snapshot',2)").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    assert_eq!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .unwrap()
            .sessions,
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(count, 3);
    sqlx::query("DETACH DATABASE src")
        .execute(&mut target)
        .await
        .unwrap();
    let moved = temp.path().join("moved.db");
    std::fs::rename(&source, &moved).unwrap();
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(moved.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    assert_eq!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .unwrap()
            .sessions,
        0,
        "renaming a migrated source must not create a fresh import identity"
    );
}

#[tokio::test]
async fn issue_256_read_only_source_retries_do_not_need_source_metadata_writes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("readonly.db");
    let mut s = database(&source).await;
    sqlx::query("INSERT INTO sessions(timestamp,summary,created_at_millis) VALUES('2026-11-09','readonly',1)").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    let mut uri = url::Url::from_file_path(&source).unwrap();
    uri.set_query(Some("mode=ro"));
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(uri.as_str())
        .execute(&mut target)
        .await
        .unwrap();
    assert_eq!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .unwrap()
            .sessions,
        1
    );
    assert_eq!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .unwrap()
            .sessions,
        0
    );
    let source_ids: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM src._harness_meta WHERE key='merge_source_id'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(source_ids, 0);
}
