//! sqlite_version_test.rs — bundled SQLite must be >= 3.54.0 (RUSTSEC-2026-0459).
//! Uses HARNESS_ROOT so the real ~/.harness is never touched.

use sqlx::Row;

#[tokio::test]
async fn bundled_sqlite_is_at_least_3_54_0() {
    let root = tempfile::tempdir().unwrap();
    // SAFETY: this file holds a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("HARNESS_ROOT", root.path());
        std::env::remove_var("HARNESS_DIR");
        std::env::remove_var("HARNESS_MEM_URL");
        std::env::remove_var("DATABASE_URL");
    }

    let pool = epic_harness::store::pool::memory_pool().await.unwrap();
    let version: String = sqlx::query("select sqlite_version()")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    println!("sqlite_version() = {version}");

    let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
    assert!(
        parts >= vec![3, 54, 0],
        "SQLite {version} is older than 3.54.0"
    );
    pool.close().await;
}
