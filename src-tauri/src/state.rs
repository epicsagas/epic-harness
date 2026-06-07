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
    /// # Why an ephemeral runtime?
    ///
    /// This is a known Tauri limitation: `.manage()` is evaluated during
    /// `Builder::build()`, which runs *before* `tauri::run()` sets up its
    /// own tokio runtime. Therefore `Handle::current()` is unavailable and
    /// we must create our own temporary runtime for the async pool setup.
    /// See: https://github.com/tauri-apps/tauri/issues/3422
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
