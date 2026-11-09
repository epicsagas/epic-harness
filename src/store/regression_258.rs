use super::*;
use std::path::Path;

fn child(root: &Path, home: &Path, project: &Path) -> std::process::Output {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store::migrate::regression_258::issue_258_child_import",
        ])
        .current_dir(project)
        .env("EPIC_STORAGE_CHILD_CASE", "258")
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("HARNESS_DIR", root)
        .env("HARNESS_ROOT", root)
        .env_remove("HARNESS_DB_URL")
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap()
}

#[tokio::test]
async fn issue_258_each_project_migrates_once_into_the_shared_database() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    let home = temp.path().join("home");
    std::fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    for project in ["alpha", "beta"] {
        let dir = root.join("projects").join(project);
        std::fs::create_dir_all(dir.join("obs")).unwrap();
        let record = serde_json::json!({"timestamp":"2026-11-09T00:00:00Z","tool":"Bash","tool_category":"bash","action":format!("echo {project} marker"),"result":"success","score":1.0});
        std::fs::write(
            dir.join("obs/session_20261109_fixture.jsonl"),
            format!("{record}\n"),
        )
        .unwrap();
    }
    std::fs::write(
        root.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    for project in ["alpha", "beta", "alpha", "beta"] {
        let output = child(&root, &home, &root.join("projects").join(project));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(root.join("harness.db"))
        .connect()
        .await
        .unwrap();
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM observations ORDER BY action")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(
        actions,
        vec!["echo alpha marker", "echo beta marker"],
        "one project's completion cannot suppress another project's source"
    );
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM _harness_meta WHERE key IN ('legacy_migrated:alpha','legacy_migrated:beta') AND value='1'").fetch_one(&mut conn).await.unwrap();
    assert_eq!(count, 2);
}

#[test]
fn issue_258_child_import() {
    if std::env::var("EPIC_STORAGE_CHILD_CASE").as_deref() == Ok("258") {
        super::super::runtime::block_on(async {
            let path = crate::shared::paths::global_harness_db_path();
            let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .connect()
                .await
                .unwrap();
            sqlx::raw_sql(crate::store::schema::DDL_SQLITE)
                .execute(&mut conn)
                .await
                .unwrap();
        });
        assert_eq!(run_subcommand(false, false), 0);
    }
}
