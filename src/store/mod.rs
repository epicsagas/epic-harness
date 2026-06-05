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
pub mod pool;
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

/// Convert a sqlx error to io::Result. Used by all `*_pool` async functions.
#[inline]
pub(crate) fn sqlx_err(e: sqlx::Error) -> io::Error {
    io::Error::other(e.to_string())
}

/// Convert a single-row query result to `Option<T>`, mapping "no rows" to `None`.
///
/// Used across all store submodules to handle the common pattern:
/// `query_row()` → `Ok(v)` / `QueryReturnedNoRows` → `None` / other errors propagated.
#[inline]
pub(crate) fn query_row_optional<T>(result: Result<T, rusqlite::Error>) -> io::Result<Option<T>> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

// ── RAII transaction guard ───────────────────────────

/// RAII guard for `BEGIN IMMEDIATE` transactions.
///
/// Calls `ROLLBACK` on drop if not explicitly committed, preventing
/// connection state corruption when errors occur mid-transaction.
pub(crate) struct ImmediateTx<'a> {
    conn: &'a Connection,
    committed: bool,
}

impl<'a> ImmediateTx<'a> {
    pub(crate) fn begin(conn: &'a Connection) -> io::Result<Self> {
        store_err(conn.execute_batch("BEGIN IMMEDIATE"))?;
        Ok(Self {
            conn,
            committed: false,
        })
    }

    pub(crate) fn commit(mut self) -> io::Result<()> {
        self.committed = true;
        store_err(self.conn.execute_batch("COMMIT"))
    }
}

impl Drop for ImmediateTx<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.execute_batch("ROLLBACK");
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

/// Convert i64 to u64 for reading SQLite-stored counters.
///
/// Companion to [`u64_to_i64`]: negative values (which should never exist for
/// counters that originated as u64) clamp to 0 with a diagnostic log.
#[inline]
pub(crate) fn i64_to_u64(v: i64) -> u64 {
    v.try_into().unwrap_or_else(|_| {
        eprintln!(
            "[store] i64_to_u64: value {v} is negative, clamping to 0 — \
             indicates data corruption or incorrect column read"
        );
        0
    })
}

/// Path to the global operational database: `~/.harness/harness.db`
///
/// Shared across all projects alongside `memory.db`. Project scoping is handled
/// via the `project` column in each table rather than separate DB files.
pub fn harness_db_path() -> std::path::PathBuf {
    paths::global_harness_db_path()
}

/// Open the harness operational database.
///
/// Creates the file if it doesn't exist. Applies schema (tables, indexes, WAL mode),
/// and runs pending schema version migrations. Legacy JSONL/JSON data import is NOT
/// automatic — run `epic-harness migrate` explicitly to import legacy data.
pub fn open_harness_db() -> io::Result<Connection> {
    let path = harness_db_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // Restrict DB file to owner-only (observation data may contain file paths,
    // command text, etc.). Only applies on Unix; no-op on other platforms.
    //
    // TOCTOU note: there is a small window between Connection::open (which creates
    // the file with default umask permissions) and set_permissions here. rusqlite
    // does not expose an fd-level fchmod API, so this window cannot be eliminated
    // without a custom VFS. The risk is low for a local single-user tool — the
    // threat actor would need to read the file within milliseconds of first creation.
    #[cfg(unix)]
    {
        let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o600));
    }

    // Apply schema (WAL + FK pragma are set inside init_schema as the first operation).
    // Uses IF NOT EXISTS throughout, so safe to call on existing DBs.
    schema::init_schema(&conn)?;

    Ok(conn)
}
