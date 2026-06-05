//! pool/ — Async connection pool factory (sqlx)
//!
//! Creates lazily-initialized `SqlitePool` instances for `harness.db` and `memory.db`.
//! Pools are stored in global async `OnceCell` singletons — the first call creates the
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

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::OnceCell;

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
        .expect("database URL scheme was validated above")
        .trim_start_matches("//");
    let path = PathBuf::from(db_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(url)
        .map_err(io::Error::other)?
        .create_if_missing(true)
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

static HARNESS_POOL: OnceCell<SqlitePool> = OnceCell::const_new();
static MEMORY_POOL: OnceCell<SqlitePool> = OnceCell::const_new();

/// Returns a shared `SqlitePool` for `harness.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Uses `CONFIG.db` for URL and max_connections.
pub async fn harness_pool() -> io::Result<SqlitePool> {
    HARNESS_POOL
        .get_or_try_init(|| async {
            let pool = build_pool(&harness_url(), CONFIG.db.max_connections).await?;
            super::schema::init_schema_pool(&pool).await?;
            Ok(pool)
        })
        .await
        .cloned()
}

/// Returns a shared `SqlitePool` for `memory.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
pub async fn memory_pool() -> io::Result<SqlitePool> {
    MEMORY_POOL
        .get_or_try_init(|| async { build_pool(&memory_url(), CONFIG.db.max_connections).await })
        .await
        .cloned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn build_pool_rejects_non_sqlite_urls() {
        let err = build_pool("postgres://localhost/harness", 1)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("sqlite:"));
    }

    #[tokio::test]
    async fn build_pool_creates_file_and_allows_schema_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested").join("harness.db");
        let url = format!("sqlite:{}", db_path.display());

        let pool = build_pool(&url, 1).await.unwrap();
        super::super::schema::init_schema_pool(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO observations
             (timestamp, session_id, tool, tool_category, project)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind("2026-06-02T10:00:00Z")
        .bind("session-1")
        .bind("Bash")
        .bind("bash")
        .bind("test-project")
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query("SELECT COUNT(*) FROM observations WHERE project = ?1")
            .bind("test-project")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.try_get::<i64, _>(0).unwrap(), 1);
        assert!(db_path.exists());

        #[cfg(unix)]
        {
            let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
