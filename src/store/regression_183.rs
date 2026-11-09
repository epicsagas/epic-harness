use super::*;
use sqlx::Connection;

async fn test_db(path: &Path) -> sqlx::SqliteConnection {
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(crate::store::schema::DDL_SQLITE)
        .execute(&mut conn)
        .await
        .unwrap();
    conn
}

#[tokio::test]
async fn issue_183_failed_database_merge_retains_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    let home = temp.path().join("home");
    let source = root.join("projects/source");
    let target = root.join("projects/target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(target.join("harness.db")).unwrap();
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    std::fs::write(
        root.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let mut conn = test_db(&source.join("harness.db")).await;
    sqlx::query("INSERT INTO sessions(timestamp, summary, created_at_millis, project) VALUES('2026-01-01','retained sentinel',1,'source')")
            .execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store::merge_project::regression_183::issue_183_child_merge",
        ])
        .current_dir(&source)
        .env("EPIC_STORAGE_CHILD_CASE", "183")
        .env("HARNESS_DIR", &root)
        .env("HARNESS_ROOT", temp.path())
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        source.join("harness.db").is_file(),
        "a failed transfer must preserve its only source"
    );
    let mut conn = test_db(&source.join("harness.db")).await;
    let summary: String = sqlx::query_scalar("SELECT summary FROM sessions")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(summary, "retained sentinel");
}

#[test]
fn issue_183_child_merge() {
    if std::env::var("EPIC_STORAGE_CHILD_CASE").as_deref() == Ok("183") {
        assert_eq!(run_merge("source", "target", false, true), 1);
    }
}
