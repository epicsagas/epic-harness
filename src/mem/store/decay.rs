//! decay.rs — Importance decay, stale tagging, and access tracking via llm-kernel

use std::io;

use super::conn::memory_conn;

pub fn decay_importance(days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::lifecycle::decay_importance(&guard, days, factor, floor)
        .map_err(|e| io::Error::other(e.to_string()))
}

pub fn tag_stale_nodes(days: u64) -> io::Result<u64> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::lifecycle::tag_stale_nodes(&guard, days)
        .map_err(|e| io::Error::other(e.to_string()))
}

pub fn touch_nodes_pool(_pool: &sqlx::AnyPool, ids: &[String]) {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return,
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    llm_kernel::graph::lifecycle::touch_nodes(&guard, ids);
}

// ── Pool-compatible wrappers ─────────────────────────────

#[allow(dead_code)]
pub async fn decay_importance_pool(
    _pool: &sqlx::AnyPool,
    days: u64,
    factor: f64,
    floor: f64,
) -> io::Result<u64> {
    decay_importance(days, factor, floor)
}

#[allow(dead_code)]
pub async fn tag_stale_nodes_pool(_pool: &sqlx::AnyPool, days: u64) -> io::Result<u64> {
    tag_stale_nodes(days)
}
