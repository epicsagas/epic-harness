//! recall.rs — Smart recall with composite scoring via llm-kernel

use std::io;

use super::conn::memory_conn;
use super::types::{ScoredNode, graph_to_node};

pub fn smart_recall(
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::recall::smart_recall(&guard, project, hint, limit)
        .map(|scored| {
            scored
                .into_iter()
                .map(|sn| ScoredNode {
                    node: graph_to_node(sn.node),
                    score: sn.score,
                })
                .collect()
        })
        .map_err(|e| io::Error::other(e.to_string()))
}

// ── Pool-compatible wrappers ─────────────────────────────

pub async fn smart_recall_pool(
    _pool: &sqlx::AnyPool,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    smart_recall(project, hint, limit)
}
