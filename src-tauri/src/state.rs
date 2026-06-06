use sqlx::AnyPool;

pub struct AppState {
    /// Knowledge graph DB (`~/.harness/memory.db`).
    pub db: AnyPool,
    /// Operational data DB (`~/.harness/projects/{slug}/harness.db`).
    pub harness_db: AnyPool,
}

impl AppState {
    /// Create a dedicated tokio runtime for pool initialization.
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

        Ok(Self { db, harness_db })
    }
}

/// Determine the default project slug from CWD git repo, matching serve.rs behavior.
pub fn default_project_slug() -> String {
    epic_harness::shared::paths::project_slug()
}
