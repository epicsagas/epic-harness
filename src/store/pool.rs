//! pool/ — Async connection pool factory (sqlx AnyPool)
//!
//! Creates lazily-initialized `AnyPool` instances for `harness.db` and `memory.db`.
//! Supports SQLite (default), PostgreSQL, and MySQL via URL scheme detection.
//!
//! Each pool is stored in a `RwLock<Option<(url, pool)>>` — on every call the
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
use std::sync::OnceLock;

use crate::config::CONFIG;
use crate::shared::paths;

// ── Driver registration (once) ────────────────────────

static DRIVERS_INSTALLED: OnceLock<()> = OnceLock::new();

/// Ensure compiled-in sqlx drivers are registered. Runs once; subsequent calls are no-ops.
fn ensure_drivers() {
    DRIVERS_INSTALLED.get_or_init(|| {
        sqlx::any::install_default_drivers();
    });
}

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
                    "unsupported database URL scheme (expected sqlite:, postgres:, or mysql:): {}",
                    mask_url(url)
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

// ── Credential masking ─────────────────────────────────

/// Strip password from a connection URL for safe error messages.
/// `postgres://user:secret@host/db` → `postgres://user:***@host/db`
fn mask_url(url: &str) -> String {
    match url.parse::<url::Url>() {
        Ok(mut u) => {
            if u.password().is_some() {
                let _ = u.set_password(Some("***"));
            }
            u.to_string()
        }
        Err(_) => "<invalid-url>".into(),
    }
}

fn default_memory_db_path() -> PathBuf {
    // Must check env var on every call — integration tests change HARNESS_ROOT
    // between tests within the same process.
    if let Ok(root) = std::env::var("HARNESS_ROOT") {
        PathBuf::from(root).join(".harness").join("memory.db")
    } else {
        paths::dirs_home().join(".harness").join("memory.db")
    }
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
    ensure_drivers();

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
///
/// For remote hosts (non-localhost), if `tls_mode` is `"prefer"`, it is
/// upgraded to `"require"` to prevent silent fallback to plaintext.
async fn build_postgres_pool(url: &str, max_connections: u32) -> io::Result<AnyPool> {
    let tls_mode = effective_tls_mode(url, &CONFIG.db.tls_mode);
    let url = apply_tls_param(url, "sslmode", &tls_mode)?;
    let masked = mask_url(&url);
    let pool = AnyPoolOptions::new()
        .max_connections(max_connections)
        .connect(&url)
        .await
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("PostgreSQL connection failed ({masked}): {e}"),
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

/// Determine effective TLS mode. For remote (non-local) hosts,
/// `"prefer"` is upgraded to `"require"` to prevent MITM attacks.
fn effective_tls_mode(url: &str, configured: &str) -> String {
    if configured != "prefer" {
        return configured.to_string();
    }
    // Check if host is local
    match url.parse::<url::Url>() {
        Ok(u) => {
            let host = u.host_str().unwrap_or("");
            if host == "localhost"
                || host == "127.0.0.1"
                || host == "::1"
                || host == "[::1]"
                || host.starts_with('/')
            {
                "prefer".into()
            } else {
                "require".into()
            }
        }
        Err(_) => "require".into(),
    }
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
// Uses `std::sync::RwLock` for double-checked locking:
// - Fast path: read lock for concurrent pool access (no contention)
// - Slow path: write lock only on first call or URL change
// This avoids holding an async mutex across schema initialization.

use std::sync::RwLock;

static HARNESS_POOL: RwLock<Option<(String, AnyPool)>> = RwLock::new(None);
static MEMORY_POOL: RwLock<Option<(String, AnyPool)>> = RwLock::new(None);

/// Recover from a poisoned RwLock — the pool data is still valid even if a
/// previous holder panicked. This is essential for integration tests that
/// share the same process and statics.
fn recover_read(
    lock: &RwLock<Option<(String, AnyPool)>>,
) -> std::sync::RwLockReadGuard<'_, Option<(String, AnyPool)>> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}
fn recover_write(
    lock: &RwLock<Option<(String, AnyPool)>>,
) -> std::sync::RwLockWriteGuard<'_, Option<(String, AnyPool)>> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

/// Returns a shared `AnyPool` for `harness.db`.
///
/// Uses a read lock for the fast path (already initialized); a write lock is
/// acquired only on first call or when the resolved URL changes.
/// The lock is dropped before any `.await` to satisfy clippy's
/// `await_holding_lock` lint.
pub async fn harness_pool() -> io::Result<AnyPool> {
    let url = harness_url();
    // Fast path: read lock — no contention for concurrent readers.
    {
        let guard = recover_read(&HARNESS_POOL);
        if let Some((cached_url, pool)) = guard.as_ref() {
            if cached_url == &url {
                return Ok(pool.clone());
            }
        }
    }
    // Slow path: take old pool under write lock, then drop before async work.
    let old_pool = {
        let mut guard = recover_write(&HARNESS_POOL);
        // Double-check after acquiring write lock.
        if let Some((cached_url, pool)) = guard.as_ref() {
            if cached_url == &url {
                return Ok(pool.clone());
            }
        }
        guard.take() // Take old pool; slot becomes None.
    }; // Write lock dropped here.

    // Async work — no lock held.
    if let Some((_, old)) = old_pool {
        old.close().await;
    }
    let pool = build_pool(&url, CONFIG.db.max_connections).await?;
    super::schema::init_schema_pool(&pool).await?;

    // Re-acquire write lock to store the new pool.
    let duplicate = {
        let mut guard = recover_write(&HARNESS_POOL);
        // Another writer may have stored a pool while we were working.
        if let Some((cached_url, existing)) = guard.as_ref() {
            if cached_url == &url {
                Some(existing.clone())
            } else {
                *guard = Some((url.clone(), pool.clone()));
                None
            }
        } else {
            *guard = Some((url.clone(), pool.clone()));
            None
        }
    }; // Write lock dropped here.
    if let Some(existing) = duplicate {
        pool.close().await;
        return Ok(existing);
    }
    Ok(pool)
}

/// Returns a shared `AnyPool` for `memory.db`.
///
/// Same lock-drop-await pattern as `harness_pool()`.
pub async fn memory_pool() -> io::Result<AnyPool> {
    let url = memory_url();
    // Fast path: read lock.
    {
        let guard = recover_read(&MEMORY_POOL);
        if let Some((cached_url, pool)) = guard.as_ref() {
            if cached_url == &url {
                return Ok(pool.clone());
            }
        }
    }
    // Slow path: take old pool under write lock, then drop before async work.
    let old_pool = {
        let mut guard = recover_write(&MEMORY_POOL);
        if let Some((cached_url, pool)) = guard.as_ref() {
            if cached_url == &url {
                return Ok(pool.clone());
            }
        }
        guard.take()
    }; // Write lock dropped here.

    // Async work — no lock held.
    if let Some((_, old)) = old_pool {
        old.close().await;
    }
    let pool = build_pool(&url, CONFIG.db.max_connections).await?;
    crate::mem::store::init_schema_pool(&pool).await?;
    let db_type = DbType::from_url(&url).unwrap_or(DbType::Sqlite);
    if db_type == DbType::Sqlite {
        crate::mem::store::auto_migrate_legacy(&pool).await;
    }

    // Re-acquire write lock to store the new pool.
    let duplicate = {
        let mut guard = recover_write(&MEMORY_POOL);
        if let Some((cached_url, existing)) = guard.as_ref() {
            if cached_url == &url {
                Some(existing.clone())
            } else {
                *guard = Some((url, pool.clone()));
                None
            }
        } else {
            *guard = Some((url, pool.clone()));
            None
        }
    }; // Write lock dropped here.
    if let Some(existing) = duplicate {
        pool.close().await;
        return Ok(existing);
    }
    Ok(pool)
}

/// Gracefully close all pools. Call on process shutdown to flush WAL.
pub async fn shutdown() {
    // Take pools under write lock, drop lock, then close.
    let harness = {
        let mut guard = recover_write(&HARNESS_POOL);
        guard.take()
    }; // Write lock dropped here, before .await
    if let Some((_, pool)) = harness {
        pool.close().await;
    }
    let memory = {
        let mut guard = recover_write(&MEMORY_POOL);
        guard.take()
    }; // Write lock dropped here, before .await
    if let Some((_, pool)) = memory {
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
    ensure_drivers();
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

    #[test]
    fn mask_url_hides_password() {
        assert_eq!(
            mask_url("postgres://user:secret@db.example.com/mydb"),
            "postgres://user:***@db.example.com/mydb"
        );
    }

    #[test]
    fn mask_url_no_password() {
        assert_eq!(
            mask_url("postgres://user@localhost/mydb"),
            "postgres://user@localhost/mydb"
        );
    }

    #[test]
    fn mask_url_invalid() {
        assert_eq!(mask_url("not a url"), "<invalid-url>");
    }

    #[test]
    fn mask_url_sqlite() {
        // SQLite URLs have no credentials — should pass through unchanged
        assert_eq!(
            mask_url("sqlite:/path/to/db.sqlite"),
            "sqlite:/path/to/db.sqlite"
        );
    }

    #[test]
    fn tls_mode_prefer_upgrades_for_remote() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@db.example.com/mydb", "prefer"),
            "require"
        );
    }

    #[test]
    fn tls_mode_prefer_keeps_for_localhost() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@localhost/mydb", "prefer"),
            "prefer"
        );
    }

    #[test]
    fn tls_mode_prefer_keeps_for_127() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@127.0.0.1/mydb", "prefer"),
            "prefer"
        );
    }

    #[test]
    fn tls_mode_prefer_keeps_for_ipv6_loopback() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@[::1]/mydb", "prefer"),
            "prefer"
        );
    }

    #[test]
    fn tls_mode_require_stays() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@localhost/mydb", "require"),
            "require"
        );
    }

    #[test]
    fn tls_mode_disable_stays() {
        assert_eq!(
            effective_tls_mode("postgres://u:p@localhost/mydb", "disable"),
            "disable"
        );
    }

    #[test]
    fn tls_mode_invalid_url_defaults_require() {
        assert_eq!(effective_tls_mode("not a url", "prefer"), "require");
    }
}
