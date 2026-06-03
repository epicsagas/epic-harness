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

/// Execute a closure with an open harness DB connection.
///
/// Returns `None` if the DB cannot be opened (logged to stderr).
/// Use for SQLite-first queries where callers provide their own fallback.
#[allow(dead_code)]
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

/// Path to the operational database: `~/.harness/projects/{slug}/harness.db`
pub fn harness_db_path() -> std::path::PathBuf {
    paths::harness_dir().join("harness.db")
}

/// Check whether legacy JSONL/JSON data has been migrated into SQLite.
///
/// Returns:
/// - `Ok(true)` when `legacy_migrated = "1"` exists in `_harness_meta`
/// - `Ok(false)` when the flag is missing (migration not yet run)
/// - `Err(...)` when the DB cannot be opened at all
///
/// Use this to decide whether to trust SQLite as the authoritative read source:
/// - `Ok(true)` → SQLite-only (empty results mean "no data", not "migration pending")
/// - `Ok(false)` or `Err` → JSONL fallback (migration hasn't run yet, or DB unavailable)
pub fn is_legacy_migrated() -> io::Result<bool> {
    let conn = open_harness_db()?;
    let migrated: bool = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");
    Ok(migrated)
}

/// Open the harness operational database.
///
/// Creates the file if it doesn't exist. Applies schema (tables, indexes),
/// runs pending migrations, and imports legacy JSONL/JSON data on first run.
///
/// For existing databases, schema init is skipped (checked via _harness_meta).
/// For new databases, schema is applied and legacy migration runs if needed.
///
/// TODO: In serve mode (long-running HTTP server), each request opens a new connection.
/// Consider caching the `Connection` in a `std::sync::OnceLock` or `thread_local!`
/// (initialized on first access, reused across requests) to avoid schema version
/// check overhead on every HTTP request. WAL mode already handles concurrent reads
/// safely, so a single shared connection per thread is sufficient. CLI hook usage
/// (one-shot process per invocation) is fine as-is — no pooling needed.
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
    let migrated = migrate::run(&conn);
    if migrated {
        eprintln!("[store] legacy migration completed on this open");
    }

    Ok(conn)
}
