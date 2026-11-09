use std::ffi::OsString;

struct Environment(Vec<(&'static str, Option<OsString>)>);

impl Drop for Environment {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            // This test owns its process and restores variables after dropping
            // its only runtime. No other test shares this test binary.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[test]
fn issue_279_shared_listing_preserves_database_errors_without_legacy_rows() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let project = data.join("projects/project-a/orbit");
    std::fs::create_dir_all(&project).unwrap();
    let _environment = Environment(
        ["HARNESS_DIR", "HARNESS_ROOT"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    // Set fixture paths before starting runtime worker threads.
    unsafe {
        std::env::set_var("HARNESS_DIR", &data);
        std::env::set_var("HARNESS_ROOT", temp.path());
    }
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // Deliberately omit the primary schema to trigger a real query error.
        let unavailable =
            epic_harness::serve::list_dashboard_pipelines_pool(&pool, Some("project-a"), Some(1))
                .await;
        assert!(
            unavailable.is_err(),
            "absence of rows must not disguise a query error"
        );
        std::fs::write(
            project.join("PIPELINE-legacy.json"),
            r#"{"id":"legacy","status":"running"}"#,
        )
        .unwrap();
        let legacy =
            epic_harness::serve::list_dashboard_pipelines_pool(&pool, Some("project-a"), Some(1))
                .await
                .unwrap();
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0]["id"], "legacy");
        pool.close().await;
    });
}
