use sqlx::AnyPool;

pub struct AppState {
    /// Knowledge graph DB (`~/.harness/memory.db`).
    pub db: AnyPool,
    /// Operational data DB (`~/.harness/projects/{slug}/harness.db`).
    pub harness_db: AnyPool,
}

impl AppState {
    /// Initialize database connection pools.
    ///
    /// Creates a short-lived tokio Runtime to drive the async pool setup,
    /// then drops it. The `AnyPool` handles maintain their own internal
    /// runtime for subsequent queries, so the ephemeral Runtime is only
    /// needed for the initial `connect()` calls.
    ///
    /// Must NOT use `Handle::current()` because `.manage()` is evaluated
    /// before Tauri's `.run()` sets up its own runtime.
    pub fn new() -> Result<Self, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

        let db = rt.block_on(async {
            epic_harness::store::pool::memory_pool()
                .await
                .map_err(|e| format!("Failed to open memory DB: {e}"))
        })?;

        let harness_db = rt.block_on(async {
            epic_harness::store::pool::harness_pool()
                .await
                .map_err(|e| format!("Failed to open harness DB: {e}"))
        })?;

        // rt drops here — pools keep their own connections alive internally.
        drop(rt);

        Ok(Self { db, harness_db })
    }
}
