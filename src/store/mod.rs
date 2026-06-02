//! store/ — Operational data SQLite I/O (replaces JSONL/JSON files)
//!
//! All project operational data (observations, sessions, evolution, metrics,
//! orchestrator state, orbit pipelines, evolved skills, global patterns) is
//! stored in `harness.db` — separate from the knowledge graph `memory.db`.
//!
//! Follows the same dual-API pattern as `src/mem/store/`:
//! standalone functions open their own connection, `_conn()` variants reuse one.

// ── Internal submodules ──────────────────────────────

pub mod evolution;
pub mod evolved;
pub mod global;
pub mod metrics;
pub mod migrate;
pub mod observations;
pub mod orbit_store;
pub mod orchestrator;
pub(crate) mod schema;
pub mod sessions;

#[cfg(test)]
mod tests;

// ── DB connection ────────────────────────────────────

use rusqlite::Connection;
use std::fs;
use std::io;

use crate::shared::paths;

/// Path to the operational database: `~/.harness/projects/{slug}/harness.db`
pub fn harness_db_path() -> std::path::PathBuf {
    paths::harness_dir().join("harness.db")
}

/// Open the harness operational database.
///
/// Creates the file if it doesn't exist. Applies schema (tables, indexes),
/// runs pending migrations, and imports legacy JSONL/JSON data on first run.
pub fn open_harness_db() -> io::Result<Connection> {
    let path = harness_db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // WAL mode for concurrent readers
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(io::Error::other)?;

    schema::init_schema(&conn)?;
    migrate::run(&conn);

    Ok(conn)
}
