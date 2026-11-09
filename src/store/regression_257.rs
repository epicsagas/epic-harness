use super::*;

#[tokio::test]
async fn issue_257_legacy_observations_have_project_provenance() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("source");
    std::fs::create_dir_all(directory.join("obs")).unwrap();
    std::fs::write(directory.join("obs/session_20261109_source.jsonl"),"{\"timestamp\":\"2026-11-09T00:00:00Z\",\"tool\":\"Bash\",\"tool_category\":\"bash\",\"action\":\"echo source marker\",\"result\":\"success\",\"score\":1.0}\n").unwrap();
    let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(":memory:")
        .connect()
        .await
        .unwrap();
    sqlx::raw_sql(crate::store::schema::DDL_SQLITE)
        .execute(&mut conn)
        .await
        .unwrap();
    let mut stats = MigrationStats {
        obs_imported: 0,
        sess_imported: 0,
        evo_imported: 0,
        errors: 0,
        total_lines: 0,
    };
    import_observations(&mut conn, "source", &directory, &mut stats)
        .await
        .unwrap();
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.obs_imported, 1);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM observations WHERE project='source' AND action='echo source marker'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(count, 1);
}
