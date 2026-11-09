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
async fn issue_185_copies_all_operational_tables_and_scalar_fields() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    let mut s = database(&source).await;
    sqlx::raw_sql("INSERT INTO sessions(timestamp,summary,created_at_millis) VALUES('2026-11-09','control',1);
      INSERT INTO evolution_records(timestamp,edit_type) VALUES('2026-11-09','modify_skill');
      INSERT INTO skill_attribution(skill_name,first_seen,sessions_holdout) VALUES('synthetic','2026-11-09',6);
      INSERT INTO orbit_pipelines(id,project,state_json,created_at,updated_at) VALUES('pipeline','old','{\"value\":7}','2026-11-09','2026-11-09');
      INSERT INTO evolved_skills(name,project,skill_md,created,updated) VALUES('synthetic','old','preserved guidance','2026-11-09','2026-11-09');
      INSERT INTO global_patterns(timestamp,project,weak_tools) VALUES('2026-11-09','old','[\"synthetic\"]');
      INSERT INTO workspace_manifest(id,updated,skills_json) VALUES(1,'2026-11-09','[\"synthetic\"]');")
      .execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    merge_attached_db_async(&mut target, "target", "src")
        .await
        .unwrap();
    let pipeline: String =
        sqlx::query_scalar("SELECT state_json FROM orbit_pipelines WHERE project='target'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(pipeline, "{\"value\":7}");
    let skill: String =
        sqlx::query_scalar("SELECT skill_md FROM evolved_skills WHERE project='target'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(skill, "preserved guidance");
    let global: String =
        sqlx::query_scalar("SELECT weak_tools FROM global_patterns WHERE project='target'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(global, "[\"synthetic\"]");
    let workspace: String = sqlx::query_scalar("SELECT skills_json FROM workspace_manifest")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(workspace, "[\"synthetic\"]");
    let edit: String =
        sqlx::query_scalar("SELECT edit_type FROM evolution_records WHERE project='target'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(edit, "modify_skill");
    let holdout: i64 =
        sqlx::query_scalar("SELECT sessions_holdout FROM skill_attribution WHERE project='target'")
            .fetch_one(&mut target)
            .await
            .unwrap();
    assert_eq!(holdout, 6);
}

#[tokio::test]
async fn issue_185_copy_error_rolls_back_earlier_tables() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    let mut s = database(&source).await;
    sqlx::raw_sql("INSERT INTO sessions(timestamp,summary,created_at_millis) VALUES('2026-11-09','must roll back',1); DROP TABLE evolution_records; CREATE TABLE evolution_records(analysis_summary TEXT); INSERT INTO evolution_records VALUES('untransferable record');").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    assert!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .is_err(),
        "unsupported source shape must not report success"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn issue_185_legacy_optional_columns_keep_documented_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    let mut s = database(&source).await;
    sqlx::raw_sql("ALTER TABLE evolution_records DROP COLUMN edit_type; ALTER TABLE skill_attribution DROP COLUMN sessions_holdout; INSERT INTO evolution_records(timestamp) VALUES('2026-11-09'); INSERT INTO skill_attribution(skill_name,first_seen) VALUES('legacy','2026-11-09');").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    merge_attached_db_async(&mut target, "target", "src")
        .await
        .unwrap();
    let edit: String = sqlx::query_scalar("SELECT edit_type FROM evolution_records")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(edit, "add_skill");
    let holdout: i64 = sqlx::query_scalar("SELECT sessions_holdout FROM skill_attribution")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(holdout, 0);
}

#[tokio::test]
async fn issue_185_preserves_invocation_identity_and_resets_remapped_reflection_cursor() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    let mut s = database(&source).await;
    let has_column: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('observations') WHERE name='invocation_id'",
    )
    .fetch_one(&mut s)
    .await
    .unwrap();
    if has_column == 0 {
        sqlx::query("ALTER TABLE observations ADD COLUMN invocation_id TEXT")
            .execute(&mut s)
            .await
            .unwrap();
    }
    sqlx::raw_sql("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,invocation_id) VALUES('fixture','session','Bash','bash','echo fixture','success',1,'stable-fixture'); INSERT INTO metrics_state(key,value,project) VALUES('reflection_cursor','999','source');").execute(&mut s).await.unwrap();
    s.close().await.unwrap();
    let mut target = database(&temp.path().join("target.db")).await;
    if has_column == 0 {
        sqlx::query("ALTER TABLE observations ADD COLUMN invocation_id TEXT")
            .execute(&mut target)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO metrics_state(key,value,project) VALUES('reflection_cursor','555','target')",
    )
    .execute(&mut target)
    .await
    .unwrap();
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    merge_attached_db_async(&mut target, "target", "src")
        .await
        .unwrap();
    let identity: String = sqlx::query_scalar("SELECT invocation_id FROM observations")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(identity, "stable-fixture");
    let cursor: String = sqlx::query_scalar(
        "SELECT value FROM metrics_state WHERE key='reflection_cursor' AND project='target'",
    )
    .fetch_one(&mut target)
    .await
    .unwrap();
    assert_eq!(cursor, "0");
}

async fn legacy_observation_source(path: &std::path::Path) {
    let mut source = database(path).await;
    sqlx::raw_sql("INSERT INTO observations(id,timestamp,session_id,tool,tool_category,action,result,score,project) VALUES(900,'2024-01-01T00:00:00Z','fresh-import','Read','read','fresh-a','success',1,'source'),(901,'2024-01-01T00:00:00Z','fresh-import','Read','read','fresh-b','success',1,'source');").execute(&mut source).await.unwrap();
    let metrics: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metrics_state")
        .fetch_one(&mut source)
        .await
        .unwrap();
    assert_eq!(metrics, 0, "fresh source has no reflection provenance");
    source.close().await.unwrap();
}

#[tokio::test]
async fn issue_218_consolidation_initializes_absent_cursor_without_changing_legacy_dates() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    legacy_observation_source(&source).await;
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    let stats = merge_attached_db_async(&mut target, "target", "src")
        .await
        .unwrap();
    assert_eq!(stats.obs, 2);
    let cursor: Option<String> = sqlx::query_scalar(
        "SELECT value FROM metrics_state WHERE key='reflection_cursor' AND project='target'",
    )
    .fetch_optional(&mut target)
    .await
    .unwrap();
    assert_eq!(
        cursor.as_deref(),
        Some("0"),
        "an absent cursor must not invoke a legacy timestamp cutoff for fresh imports"
    );
    let timestamps: Vec<String> =
        sqlx::query_scalar("SELECT timestamp FROM observations WHERE project='target' ORDER BY id")
            .fetch_all(&mut target)
            .await
            .unwrap();
    assert_eq!(timestamps, vec!["2024-01-01T00:00:00Z"; 2]);
    let source_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM src.observations")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(source_rows, 2, "source history remains intact");
}

#[tokio::test]
async fn issue_218_consolidation_cursor_creation_failure_rolls_back_import() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.db");
    legacy_observation_source(&source).await;
    let mut target = database(&temp.path().join("target.db")).await;
    sqlx::raw_sql("CREATE TRIGGER reject_cursor BEFORE INSERT ON metrics_state WHEN NEW.key='reflection_cursor' BEGIN SELECT RAISE(ABORT,'synthetic cursor creation failure'); END;").execute(&mut target).await.unwrap();
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut target)
        .await
        .unwrap();
    assert!(
        merge_attached_db_async(&mut target, "target", "src")
            .await
            .is_err(),
        "a failed cursor creation cannot report successful import"
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM observations")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(
        rows, 0,
        "observation transfer rolls back with cursor creation"
    );
    let source_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM src.observations")
        .fetch_one(&mut target)
        .await
        .unwrap();
    assert_eq!(source_rows, 2);
}
