//! pool/ — Async connection pool factory (sqlx)
//!
//! Creates lazily-initialized `SqlitePool` instances for `harness.db` and `memory.db`.
//! Each pool is stored in a `TokioMutex<Option<(url, pool)>>` — on every call the
//! resolved URL is compared to the cached URL; if they differ the old pool is closed
//! and a new one is opened.  This allows integration tests to redirect pool paths via
//! `HARNESS_ROOT` without leaking state across tests.

use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::config::CONFIG;
use crate::shared::paths;

// ── Default paths ──────────────────────────────────

fn default_harness_db_path() -> PathBuf {
    paths::global_harness_db_path()
}

fn default_memory_db_path() -> PathBuf {
    // Honour HARNESS_ROOT if set (used by integration tests to redirect to a temp dir).
    if let Ok(root) = std::env::var("HARNESS_ROOT") {
        return PathBuf::from(root).join(".harness").join("memory.db");
    }
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
//
// In production (and in unit tests within the library crate), `HARNESS_ROOT` is
// fixed for the lifetime of the process, so `OnceCell` is the right primitive.
//
// Integration tests (`tests/`) set `HARNESS_ROOT` to a fresh temp dir before each
// test.  To support that pattern without forcing every integration test to call into
// async pool machinery directly, each pool is stored as
// `Mutex<Option<(url, pool)>>`.  On every call we check whether the resolved URL
// still matches the cached URL; if not, the old pool is closed and a new one is
// opened.  The mutex guarantees that concurrent async callers never see a torn
// state.

use tokio::sync::Mutex as TokioMutex;

static HARNESS_POOL: TokioMutex<Option<(String, SqlitePool)>> = TokioMutex::const_new(None);
static MEMORY_POOL: TokioMutex<Option<(String, SqlitePool)>> = TokioMutex::const_new(None);

/// Returns a shared `SqlitePool` for `harness.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Uses `CONFIG.db` for URL and max_connections.
pub async fn harness_pool() -> io::Result<SqlitePool> {
    let url = harness_url();
    let mut guard = HARNESS_POOL.lock().await;
    if guard.as_ref().map(|(u, _)| u == &url).unwrap_or(false) {
        return Ok(guard.as_ref().unwrap().1.clone());
    }
    // URL changed (or first call) — close old pool and open a new one.
    if let Some((_, old)) = guard.take() {
        old.close().await;
    }
    let pool = build_pool(&url, CONFIG.db.max_connections).await?;
    super::schema::init_schema_pool(&pool).await?;
    *guard = Some((url, pool.clone()));
    Ok(pool)
}

/// Returns a shared `SqlitePool` for `memory.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Initializes the memory schema (nodes, edges, FTS5) on first creation.
pub async fn memory_pool() -> io::Result<SqlitePool> {
    let url = memory_url();
    let mut guard = MEMORY_POOL.lock().await;
    if guard.as_ref().map(|(u, _)| u == &url).unwrap_or(false) {
        return Ok(guard.as_ref().unwrap().1.clone());
    }
    // URL changed (or first call) — close old pool and open a new one.
    if let Some((_, old)) = guard.take() {
        old.close().await;
    }
    let pool = build_pool(&url, CONFIG.db.max_connections).await?;
    crate::mem::store::init_schema_pool(&pool).await?;
    crate::mem::store::auto_migrate_legacy(&pool).await;
    *guard = Some((url, pool.clone()));
    Ok(pool)
}

/// Gracefully close all pools. Call on process shutdown to flush WAL.
#[allow(dead_code)]
pub async fn shutdown() {
    if let Some((_, pool)) = HARNESS_POOL.lock().await.take() {
        pool.close().await;
    }
    if let Some((_, pool)) = MEMORY_POOL.lock().await.take() {
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
