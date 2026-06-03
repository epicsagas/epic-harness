use std::sync::{Arc, Mutex};

use rusqlite::Connection;

pub struct AppState {
    /// Knowledge graph DB (`~/.harness/memory.db`).
    pub db: Arc<Mutex<Connection>>,
    /// Operational data DB (`~/.harness/projects/{slug}/harness.db`).
    pub harness_db: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let mem_conn =
            epic_harness::mem::store::open_db().map_err(|e| format!("Failed to open memory DB: {e}"))?;
        let harness_conn = epic_harness::store::open_harness_db()
            .map_err(|e| format!("Failed to open harness DB: {e}"))?;
        Ok(Self {
            db: Arc::new(Mutex::new(mem_conn)),
            harness_db: Arc::new(Mutex::new(harness_conn)),
        })
    }
}
