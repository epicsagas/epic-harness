use std::sync::{Arc, Mutex};

use rusqlite::Connection;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let conn =
            epic_harness::mem::store::open_db().map_err(|e| format!("Failed to open DB: {e}"))?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }
}
