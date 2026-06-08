//! schema.rs — Schema initialization (delegated to conn.rs)
//!
//! Kept for backward compatibility with pool.rs callers.
//! The actual schema init now happens in conn.rs via llm-kernel.

use sqlx::AnyPool;
use std::io;

/// No-op: schema initialization is now handled by `conn::memory_conn()`.
pub async fn init_schema_pool(_pool: &AnyPool) -> io::Result<()> {
    Ok(())
}
