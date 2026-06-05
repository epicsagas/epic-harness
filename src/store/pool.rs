//! pool/ — Async connection pool factory (sqlx)
//!
//! Creates lazily-initialized `SqlitePool` instances for `harness.db` and `memory.db`.
//! Pools are stored in global `OnceLock` singletons — the first call creates the
//! pool, subsequent calls return the existing one.
//!
//! This module lives alongside the existing rusqlite sync code in `store/mod.rs`.
//! No existing code is modified; the async pools are additive for the migration.

use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::config::CONFIG;
use crate::shared::paths;

// ── Default paths ──────────────────────────────────

fn default_harness_db_path() -> PathBuf {
    paths::global_harness_db_path()
}

fn default_memory_db_path() -> PathBuf {
    paths::dirs_home().join(".harness").join("memory.db")
}

// ── Pool construction ──────────────────────────────

/// Resolve the effective connection URL for harness.db.
/// If the config value is empty, falls back to the default path.
fn harness_url() -> String {
    let configured = &CONFIG.db.harness_url;
    if configured.is_empty() {
        format!("sqlite:{}", default_harness_db_path().display())
    } else {
        configured.clone()
    }
}

/// Resolve the effective connection URL for memory.db.
fn memory_url() -> String {
    let configured = &CONFIG.db.memory_url;
    if configured.is_empty() {
        format!("sqlite:{}", default_memory_db_path().display())
    } else {
        configured.clone()
    }
}

/// Build a `SqlitePool` from a `sqlite:` URL string.
///
/// - Validates the URL scheme.
/// - Creates parent directories if needed.
/// - Sets WAL mode, foreign_keys=ON, busy_timeout=5000ms.
/// - Restricts file permissions to 0o600 on Unix.
async fn build_pool(url: &str, max_connections: u32) -> io::Result<SqlitePool> {
    // URL validation: must use sqlite: scheme.
    if !url.starts_with("sqlite:") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("database URL must start with 'sqlite:': {url}"),
        ));
    }

    // Extract filesystem path from "sqlite:/path/to/db" for directory/permission setup.
    let db_path = url
        .strip_prefix("sqlite:")
        .unwrap_or(url)
        .trim_start_matches("//");
    let path = PathBuf::from(db_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(url)
        .map_err(io::Error::other)?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_millis(5000));

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(io::Error::other)?;

    // Restrict file permissions (same pattern as the rusqlite code in store/mod.rs).
    #[cfg(unix)]
    {
        let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o600));
    }

    Ok(pool)
}

// ── Global pool singletons ─────────────────────────

static HARNESS_POOL: OnceLock<SqlitePool> = OnceLock::new();
static MEMORY_POOL: OnceLock<SqlitePool> = OnceLock::new();

/// Returns a shared `SqlitePool` for `harness.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Uses `CONFIG.db` for URL and max_connections.
pub async fn harness_pool() -> io::Result<SqlitePool> {
    if let Some(pool) = HARNESS_POOL.get() {
        return Ok(pool.clone());
    }
    let pool = build_pool(&harness_url(), CONFIG.db.max_connections).await?;
    // Initialize schema on first connection.
    super::schema::init_schema_pool(&pool).await?;
    // If another thread beat us, that's fine — both pools are identical.
    let _ = HARNESS_POOL.set(pool.clone());
    Ok(pool)
}

/// Returns a shared `SqlitePool` for `memory.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
pub async fn memory_pool() -> io::Result<SqlitePool> {
    if let Some(pool) = MEMORY_POOL.get() {
        return Ok(pool.clone());
    }
    let pool = build_pool(&memory_url(), CONFIG.db.max_connections).await?;
    let _ = MEMORY_POOL.set(pool.clone());
    Ok(pool)
}

/// Gracefully close all pools. Call on process shutdown to flush WAL.
///
/// Uses `get()` rather than `take()` because `OnceLock::take()` requires `&mut self`,
/// which is unavailable on a `static`. After `close()`, any further pool operations
/// will return an error — the desired behavior for a shutdown path.
#[cfg(test)]
pub async fn shutdown() {
    if let Some(pool) = HARNESS_POOL.get() {
        pool.close().await;
    }
    if let Some(pool) = MEMORY_POOL.get() {
        pool.close().await;
    }
}

/// Create a transient in-memory `SqlitePool` for tests.
/// Each call creates a fresh pool (no singleton reuse).
#[cfg(test)]
pub async fn test_memory_pool() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(io::Error::other)
        .unwrap()
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap()
}
