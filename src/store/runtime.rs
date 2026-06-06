//! runtime.rs — Global async runtime for sqlx pool access from sync callers
//!
//! Hooks (observe, reflect, snapshot, resume) and serve.rs (tiny_http) are
//! synchronous. This module provides a lazily-initialized tokio runtime so
//! they can call async pool functions via `block_on()`.

use std::future::Future;
use std::sync::LazyLock;

use crate::config::CONFIG;

/// Global tokio runtime shared by all sync callers.
///
/// Initialized once on first use. Worker threads are set to
/// `min(max_connections + 2, 16)` to prevent excessive thread creation
/// when `max_connections` is set to a high value.
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    let workers = ((CONFIG.db.max_connections + 2) as usize).min(16);
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_io()
        .enable_time()
        .build()
        .expect("failed to create harness store runtime")
});

/// Run an async store function from a synchronous context.
///
/// Safe to call from any thread that is NOT already inside a tokio runtime
/// (e.g., hooks invoked as child processes, `serve.rs` tiny_http thread).
///
/// # Panics
/// Panics if called from inside a tokio task — e.g., an axum handler or
/// a `#[tokio::test]`. Use `.await` directly in async callers instead.
pub(crate) fn block_on<F, T>(fut: F) -> T
where
    F: Future<Output = T>,
{
    // Guard: panic early with a clear message if called from inside tokio.
    // This prevents a confusing double-runtime panic from tokio itself.
    if tokio::runtime::Handle::try_current().is_ok() {
        panic!(
            "store::runtime::block_on() called inside a tokio runtime — use .await directly in async code"
        );
    }
    RUNTIME.block_on(fut)
}
