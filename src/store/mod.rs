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

/// Convert u64 to i64 for SQLite storage.
/// Saturates at i64::MAX on overflow (extremely unlikely for session/metric counters).
#[inline]
pub(crate) fn u64_to_i64(v: u64) -> i64 {
    v.try_into().unwrap_or(i64::MAX)
}

/// Path to the operational database: `~/.harness/projects/{slug}/harness.db`
pub fn harness_db_path() -> std::path::PathBuf {
    paths::harness_dir().join("harness.db")
}

/// Open the harness operational database.
///
/// Creates the file if it doesn't exist. Applies schema (tables, indexes),
/// runs pending migrations, and imports legacy JSONL/JSON data on first run.
///
/// For existing databases, schema init is skipped (checked via _harness_meta).
/// For new databases, schema is applied and legacy migration runs if needed.
pub fn open_harness_db() -> io::Result<Connection> {
    let path = harness_db_path();
    let is_new = !path.exists();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // WAL mode for concurrent readers
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(io::Error::other)?;

    // Always ensure schema exists (uses IF NOT EXISTS internally).
    // For existing DBs this is a no-op since all tables already exist.
    schema::init_schema(&conn)?;

    // Migration check is cheap: reads _harness_meta 'legacy_migrated'.
    // If already migrated, returns immediately (single SELECT).
    if is_new {
        migrate::run(&conn);
    } else {
        // Even for existing DBs, check if migration was done
        // (handles first open after upgrade on existing DB)
        let migrated: bool = conn
            .query_row(
                "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .is_some_and(|v| v == "1");
        if !migrated {
            migrate::run(&conn);
        }
    }

    Ok(conn)
}
