//! store/ — Operational data SQLite I/O (replaces JSONL/JSON files)
//!
//! All project operational data (observations, sessions, evolution, metrics,
//! orchestrator state, orbit pipelines, evolved skills, global patterns) is
//! stored in `harness.db` — separate from the knowledge graph `memory.db`.
//!
//! Follows the same async pool pattern as `src/mem/store/`:
//! all functions use `SqlitePool` for concurrent access.

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
pub mod runtime;
pub(crate) mod schema;
pub mod sessions;

#[cfg(test)]
mod tests;

// ── Store error helpers ──────────────────────────────

/// Convert a sqlx error to io::Result. Used by all `*_pool` async functions.
#[inline]
pub(crate) fn sqlx_err(e: sqlx::Error) -> std::io::Error {
    std::io::Error::other(e)
}

// ── Numeric helpers ──────────────────────────────────

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
    crate::shared::paths::global_harness_db_path()
}
