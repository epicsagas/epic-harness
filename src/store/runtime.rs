//! runtime.rs — Global async runtime for sqlx pool access from sync callers
//!
//! Hooks (observe, reflect, snapshot, resume) and serve.rs (tiny_http) are
//! synchronous. This module provides a lazily-initialized tokio runtime so
//! they can call async pool functions via `block_on()`.

use std::future::Future;
use std::sync::LazyLock;

/// Global tokio runtime shared by all sync callers.
///
/// Initialized once on first use. Two worker threads are sufficient for
/// SQLite I/O (which is the only async work in this codebase).
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to create harness store runtime")
});

/// Run an async store function from a synchronous context.
///
/// Creates the global runtime on first call. Safe to call from any thread.
/// **Panics** if called from within the runtime itself (would deadlock).
pub(crate) fn block_on<F, T>(fut: F) -> T
where
    F: Future<Output = T>,
{
    RUNTIME.block_on(fut)
}
