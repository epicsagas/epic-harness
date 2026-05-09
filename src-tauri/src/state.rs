use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let conn =
            epic_harness::mem::store::open_db().map_err(|e| format!("Failed to open DB: {e}"))?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }
}
