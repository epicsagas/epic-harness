//! pool/ — Async connection pool factory (sqlx AnyPool)
//!
//! Creates lazily-initialized `AnyPool` instances for `harness.db` and `memory.db`.
//! Supports SQLite (default), PostgreSQL, and MySQL via URL scheme detection.
//!
//! Each pool is stored in a `TokioMutex<Option<(url, pool)>>` — on every call the
//! resolved URL is compared to the cached URL; if they differ the old pool is closed
//! and a new one is opened.  This allows integration tests to redirect pool paths via
//! `HARNESS_ROOT` without leaking state across tests.

use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use sqlx::AnyPool;
use sqlx::any::AnyPoolOptions;

use crate::config::CONFIG;
use crate::shared::paths;

// ── Database type detection ───────────────────────────

/// Detected database backend from connection URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbType {
    Sqlite,
    Postgres,
    Mysql,
}

impl DbType {
    /// Detect database type from connection URL scheme.
    pub fn from_url(url: &str) -> io::Result<Self> {
        if url.starts_with("sqlite:") {
            Ok(Self::Sqlite)
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            Ok(Self::Postgres)
        } else if url.starts_with("mysql:") {
            Ok(Self::Mysql)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unsupported database URL scheme (expected sqlite:, postgres:, or mysql:): {url}"
                ),
            ))
        }
    }

    /// Return the config-style driver name for this backend.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
        }
    }
}

// ── Default paths ──────────────────────────────────────

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

// ── Pool construction ──────────────────────────────────

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

/// Build an `AnyPool` from a connection URL.
///
/// Dispatches to the appropriate driver based on URL scheme:
/// - `sqlite:` — WAL mode, foreign_keys=ON, busy_timeout=5s, file permissions 0o600
/// - `postgres:` — TLS required for non-local connections
/// - `mysql:` — TLS required for non-local connections
async fn build_pool(url: &str, max_connections: u32) -> io::Result<AnyPool> {
    // Ensure compiled-in drivers are registered for AnyPool.
    // Idempotent — safe to call on every build_pool invocation.
    sqlx::any::install_default_drivers();

    let db_type = DbType::from_url(url)?;

    match db_type {
        DbType::Sqlite => build_sqlite_pool(url, max_connections).await,
        DbType::Postgres => build_postgres_pool(url, max_connections).await,
        DbType::Mysql => build_mysql_pool(url, max_connections).await,
    }
}

/// Build a SQLite pool via AnyPoolOptions.
///
/// Pre-creates the database file (AnyConnectOptions doesn't expose
/// `create_if_missing`), then connects by URL. PRAGMAs are applied in
/// `init_schema_pool()`.
async fn build_sqlite_pool(url: &str, max_connections: u32) -> io::Result<AnyPool> {
    // Extract filesystem path for directory/permission setup.
    let db_path = url
        .strip_prefix("sqlite:")
        .unwrap_or(url)
        .trim_start_matches("//");

    // In-memory databases have no filesystem path.
    if db_path != ":memory:" && !db_path.is_empty() {
        let path = PathBuf::from(db_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Pre-create file so AnyPoolOptions::connect() can open it.
        if !path.exists() {
            fs::File::create(&path)?;
        }

        #[cfg(unix)]
        {
            if path.exists() {
                let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o600));
            }
        }
    }

    let pool = AnyPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .map_err(io::Error::other)?;

    // Set file permissions after pool opens the file.
    #[cfg(unix)]
    {
        if db_path != ":memory:" && !db_path.is_empty() {
            let path = PathBuf::from(db_path);
            let _ = fs::set_permissions(&path, PermissionsExt::from_mode(0o600));
        }
    }

    Ok(pool)
}

/// Build a PostgreSQL pool with TLS enforced per `CONFIG.db.tls_mode`.
async fn build_postgres_pool(url: &str, max_connections: u32) -> io::Result<AnyPool> {
    let url = apply_tls_param(url, "sslmode", &CONFIG.db.tls_mode)?;
    let pool = AnyPoolOptions::new()
        .max_connections(max_connections)
        .connect(&url)
        .await
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("PostgreSQL connection failed: {e}"),
            )
        })?;

    Ok(pool)
}

/// Build a MySQL pool — currently **unsupported** (DDL and FTS not yet implemented).
///
/// Returns an error directing users to SQLite or PostgreSQL backends.
async fn build_mysql_pool(_url: &str, _max_connections: u32) -> io::Result<AnyPool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "MySQL driver is not yet supported — use 'sqlite' (default) or 'postgres'. \
         MySQL support will be added in a future release.",
    ))
}

/// Inject or replace a TLS query parameter in the connection URL.
///
/// Maps config values: prefer → `prefer`, require → `require`, disable → `disable`.
/// For MySQL, `prefer` maps to `PREFERRED` and `require` to `REQUIRED`.
/// Note: The MySQL `ssl-mode` branch is currently dead code because `build_mysql_pool()`
/// returns `Unsupported`. It will be used when MySQL support is implemented.
fn apply_tls_param(url: &str, param: &str, tls_mode: &str) -> io::Result<String> {
    let mut parsed = url
        .parse::<url::Url>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let value = match tls_mode {
        "disable" => {
            if param == "ssl-mode" {
                "DISABLED"
            } else {
                "disable"
            }
        }
        "require" => {
            if param == "ssl-mode" {
                "REQUIRED"
            } else {
                "require"
            }
        }
        _ => {
            // "prefer"
            if param == "ssl-mode" {
                "PREFERRED"
            } else {
                "prefer"
            }
        }
    };

    // Replace existing param or append using url crate's proper percent-encoding.
    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if let Some(pair) = pairs.iter_mut().find(|(k, _)| k == param) {
        pair.1 = value.to_string();
    } else {
        pairs.push((param.to_string(), value.to_string()));
    }

    // Rebuild query using query_pairs_mut for correct percent-encoding.
    parsed.query_pairs_mut().clear().extend_pairs(pairs);

    Ok(parsed.to_string())
}

// ── Global pool singletons ─────────────────────────────
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

static HARNESS_POOL: TokioMutex<Option<(String, AnyPool)>> = TokioMutex::const_new(None);
static MEMORY_POOL: TokioMutex<Option<(String, AnyPool)>> = TokioMutex::const_new(None);

/// Returns a shared `AnyPool` for `harness.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Uses `CONFIG.db` for URL and max_connections.
pub async fn harness_pool() -> io::Result<AnyPool> {
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

/// Returns a shared `AnyPool` for `memory.db`.
///
/// Creates the pool on first call; subsequent calls return the same instance.
/// Initializes the memory schema (nodes, edges, FTS5) on first creation.
pub async fn memory_pool() -> io::Result<AnyPool> {
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
    let db_type = DbType::from_url(&url).unwrap_or(DbType::Sqlite);
    if db_type == DbType::Sqlite {
        crate::mem::store::auto_migrate_legacy(&pool).await;
    }
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

/// Detect the database type for the harness.db pool.
pub fn harness_db_type() -> DbType {
    DbType::from_url(&harness_url()).unwrap_or(DbType::Sqlite)
}

/// Detect the database type for the memory.db pool.
pub fn memory_db_type() -> DbType {
    DbType::from_url(&memory_url()).unwrap_or(DbType::Sqlite)
}

/// Create a transient in-memory `AnyPool` (SQLite) for tests.
/// Each call creates a fresh pool (no singleton reuse).
#[cfg(test)]
pub async fn test_memory_pool() -> AnyPool {
    sqlx::any::install_default_drivers();
    AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[test]
    fn db_type_detection() {
        assert_eq!(DbType::from_url("sqlite:test.db").unwrap(), DbType::Sqlite);
        assert_eq!(
            DbType::from_url("postgres://u:p@localhost/db").unwrap(),
            DbType::Postgres
        );
        assert_eq!(
            DbType::from_url("postgresql://u:p@localhost/db").unwrap(),
            DbType::Postgres
        );
        assert_eq!(
            DbType::from_url("mysql://u:p@localhost/db").unwrap(),
            DbType::Mysql
        );
        assert!(DbType::from_url("oracle://bad").is_err());
    }

    #[tokio::test]
    async fn build_pool_rejects_unknown_schemes() {
        let err = build_pool("oracle://localhost/harness", 1)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("unsupported database URL scheme"));
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
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind("2026-06-02T10:00:00Z")
        .bind("session-1")
        .bind("Bash")
        .bind("bash")
        .bind("test-project")
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query("SELECT COUNT(*) FROM observations WHERE project = $1")
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
