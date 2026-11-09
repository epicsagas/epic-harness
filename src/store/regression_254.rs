use super::*;
use sqlx::{Connection, Row};

#[tokio::test]
async fn issue_254_preserves_unique_and_shared_promotion_counts() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("harness.db");
    let mut conn = SqliteConnectOptions::new()
        .filename(&db)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(crate::store::schema::DDL_SQLITE)
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO promotion_counters(pattern_key,project,count) VALUES('unique','source',7),('shared','source',2),('shared','target',3),('unrelated','other',11)")
        .execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    merge_global_db(&db, "source", "target", false)
        .await
        .unwrap();
    let mut conn = SqliteConnectOptions::new()
        .filename(&db)
        .connect()
        .await
        .unwrap();
    let rows = sqlx::query(
        "SELECT pattern_key, project, count FROM promotion_counters ORDER BY pattern_key",
    )
    .fetch_all(&mut conn)
    .await
    .unwrap();
    let rows: Vec<(String, String, i64)> = rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("shared".into(), "target".into(), 5),
            ("unique".into(), "target".into(), 7),
            ("unrelated".into(), "other".into(), 11)
        ]
    );
    merge_global_db(&db, "source", "target", false)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count FROM promotion_counters WHERE pattern_key='shared' AND project='target'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(
        count, 5,
        "retry must not add the already merged source again"
    );
}
