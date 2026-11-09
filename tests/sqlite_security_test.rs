//! Exercise the SQLx engine actually linked into Epic, using synthetic data only.
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;

async fn open(path: &Path, page_size: u32) -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .page_size(page_size)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn bundled_engine_contains_wal_memory_exposure_fix() {
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(version, "3.54.1", "must execute the reviewed fixed engine");
    db.close().await;
}

#[tokio::test]
async fn existing_database_and_matching_wal_remain_readable() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.db");
    let db = open(&original, 4096).await;
    sqlx::query("PRAGMA wal_autocheckpoint=0")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE messages (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO messages VALUES (1, 'existing Epic memory')")
        .execute(&db)
        .await
        .unwrap();
    let copy = dir.path().join("copy.db");
    std::fs::copy(&original, &copy).unwrap();
    std::fs::copy(
        original.with_extension("db-wal"),
        copy.with_extension("db-wal"),
    )
    .unwrap();
    let restored = open(&copy, 4096).await;
    let value: String = sqlx::query_scalar("SELECT body FROM messages WHERE id=1")
        .fetch_one(&restored)
        .await
        .unwrap();
    assert_eq!(value, "existing Epic memory");
    restored.close().await;
    db.close().await;

    let reopened = open(&original, 4096).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM messages")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(count, 1);
    reopened.close().await;
}

#[tokio::test]
async fn legacy_database_remains_readable_and_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    std::fs::write(&path, include_bytes!("fixtures/sqlite-legacy.db")).unwrap();
    let db = open(&path, 4096).await;
    let value: String = sqlx::query_scalar("SELECT body FROM memories WHERE id=1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(value, "synthetic pre-upgrade memory");
    sqlx::query("INSERT INTO memories(body) VALUES ('after upgrade')")
        .execute(&db)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM memories")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 2);
    db.close().await;
}
