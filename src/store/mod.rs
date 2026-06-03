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

#[cfg(test)]
pub(crate) use tests::in_memory_db;

// ── Store error helpers ──────────────────────────────

/// Convert a rusqlite result to io::Result, preserving context.
#[inline]
pub(crate) fn store_err<T>(result: Result<T, rusqlite::Error>) -> io::Result<T> {
    result.map_err(io::Error::other)
}

/// Execute a closure with an open harness DB connection.
///
/// Returns `None` if the DB cannot be opened (logged to stderr).
/// Use for SQLite-first queries where callers provide their own fallback.
pub fn with_harness_db<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> io::Result<T>,
{
    match open_harness_db() {
        Ok(conn) => match f(&conn) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("[store] query failed: {e}");
                None
            }
        },
        Err(e) => {
            eprintln!("[store] harness.db unavailable: {e}");
            None
        }
    }
}

// ── DB connection ────────────────────────────────────

use rusqlite::Connection;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::shared::paths;

/// Convert u64 to i64 for SQLite storage.
///
/// SQLite has no native u64 type, so counters are stored as i64.
/// Values exceeding `i64::MAX` saturate — this is acceptable because all u64 fields
/// in the schema (session counters, observation counts, skill attribution) are
/// monotonically increasing counters that will never approach `i64::MAX` (~9.2e18)
/// in practice. The saturation preserves ordering and prevents silent data loss.
///
/// When reading back, `i64 as u64` is always safe for values that originated here
/// (either the original value or `i64::MAX`, both non-negative).
#[inline]
pub(crate) fn u64_to_i64(v: u64) -> i64 {
    v.try_into().unwrap_or_else(|_| {
        eprintln!(
            "[store] u64_to_i64: value {v} exceeds i64::MAX, saturating — \
             acceptable for counters but would break if used as an identifier"
        );
        i64::MAX
    })
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

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // Restrict DB file to owner-only (observation data may contain file paths,
    // command text, etc.). Only applies on Unix; no-op on other platforms.
    #[cfg(unix)]
    {
        let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o600));
    }

    // Apply schema (WAL + FK pragma are set inside init_schema as the first operation).
    // Uses IF NOT EXISTS throughout, so safe to call on existing DBs.
    schema::init_schema(&conn)?;

    // Run legacy migration when needed. migrate::run() is idempotent — it checks
    // the 'legacy_migrated' flag and exits immediately if already done.
    // Runs for both new and existing DBs to handle the first open after an upgrade.
    migrate::run(&conn);

    Ok(conn)
}
