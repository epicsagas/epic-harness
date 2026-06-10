//! store/ — Operational data SQLite I/O (replaces JSONL/JSON files)
//!
//! All project operational data (observations, sessions, evolution, metrics,
//! orchestrator state, orbit pipelines, evolved skills, global patterns) is
//! stored in `harness.db` — separate from the knowledge graph `memory.db`.
//!
//! Uses `sqlx` async pool via `pool.rs` + `runtime.rs` sync bridge.
//! Functions suffixed `_pool` take `&AnyPool`; standalone helpers use the pool
//! internally via `runtime::block_on`.

// ── Internal submodules ──────────────────────────────

pub mod evolution;
pub mod evolved;
pub mod global;
pub mod merge_project;
pub mod metrics;
pub mod migrate;
pub mod observations;
pub mod orbit_store;
pub mod orchestrator;
pub(crate) mod pool;
pub(crate) mod runtime;
pub(crate) mod schema;
pub mod sessions;

#[cfg(test)]
mod tests;

// ── Helpers ────────────────────────────────────────────

use std::io;

/// Convert `sqlx::Error` to `io::Error`.
#[inline]
pub(crate) fn sqlx_err(e: sqlx::Error) -> io::Error {
    io::Error::other(e.to_string())
}

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

/// Convert i64 back to u64. Clamps negative values to 0.
#[inline]
pub(crate) fn i64_to_u64(v: i64) -> u64 {
    v.try_into().unwrap_or(0)
}

/// Path to the operational database: `~/.harness/projects/{slug}/harness.db`
pub fn harness_db_path() -> std::path::PathBuf {
    crate::shared::paths::harness_dir().join("harness.db")
}
