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
/// Saturates at i64::MAX on overflow (extremely unlikely for session/metric counters,
/// but logs a warning so callers can detect if it ever happens in production).
#[inline]
pub(crate) fn u64_to_i64(v: u64) -> i64 {
    match v.try_into() {
        Ok(n) => n,
        Err(_) => {
            eprintln!(
                "[store] u64_to_i64: value {v} exceeds i64::MAX, saturating — \
                 sequence_id uniqueness may be affected"
            );
            i64::MAX
        }
    }
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
    let _is_new = !path.exists();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // Apply schema (WAL + FK pragma are set inside init_schema as the first operation).
    // Uses IF NOT EXISTS throughout, so safe to call on existing DBs.
    schema::init_schema(&conn)?;

    // Run legacy migration when needed. migrate::run() is idempotent — it checks
    // the 'legacy_migrated' flag and exits immediately if already done.
    // Runs for both new and existing DBs to handle the first open after an upgrade.
    migrate::run(&conn);

    Ok(conn)
}
