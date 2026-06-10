//! conn.rs — Memory pool access for memory.db via sqlx
//!
//! Provides both async and sync access to the memory.db AnyPool.
//! Schema initialization is handled by `schema.rs` and triggered
//! by `pool::memory_pool()` on first connection.

use std::io;

use sqlx::AnyPool;

use crate::store::runtime;

/// Returns the shared AnyPool for memory.db (async).
///
/// Delegates to `crate::store::pool::memory_pool()` which handles
/// singleton lifecycle, URL change detection, and schema init.
pub async fn memory_pool_async() -> io::Result<AnyPool> {
    crate::store::pool::memory_pool().await
}

/// Returns the shared AnyPool for memory.db (sync).
///
/// Uses `runtime::block_on()` to bridge from sync callers.
pub fn memory_pool_sync() -> io::Result<AnyPool> {
    runtime::block_on(memory_pool_async())
}

/// Execute a fallible async closure with the memory pool from a sync context.
///
/// Helper to reduce boilerplate in sync store functions:
/// ```ignore
/// with_pool(|pool| async move {
///     sqlx::query("SELECT ...").fetch_optional(pool).await
/// })
/// ```
pub(crate) fn with_pool<F, Fut, T>(f: F) -> io::Result<T>
where
    F: FnOnce(AnyPool) -> Fut,
    Fut: std::future::Future<Output = io::Result<T>>,
{
    let pool = memory_pool_sync()?;
    runtime::block_on(f(pool))
}
