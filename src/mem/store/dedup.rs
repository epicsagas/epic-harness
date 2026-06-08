//! dedup.rs — Node deduplication via llm-kernel

use std::io;

use super::conn::memory_conn;
use super::types::{Node, node_to_graph};

/// Write-with-dedup: checks for a duplicate, writes only when none is found.
/// Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    let gn = node_to_graph(node.clone());
    llm_kernel::graph::dedup::upsert_node_dedup(&guard, &gn, window_hours)
        .map_err(|e| io::Error::other(e.to_string()))
}

/// Async write-with-dedup using pool (delegates to sync).
pub async fn write_node_dedup_pool(
    _pool: &sqlx::AnyPool,
    node: &Node,
    window_hours: u64,
) -> io::Result<(String, bool)> {
    write_node_dedup(node, window_hours)
}
