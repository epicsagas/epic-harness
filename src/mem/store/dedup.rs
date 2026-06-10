//! dedup.rs — Node deduplication via sqlx

use std::io;

use sqlx::Row;

use super::conn::memory_pool_sync;
use super::types::{Node, node_to_graph};
use super::util::{NODE_COLUMNS, now_iso, parse_iso_to_secs, row_to_graph_node};
use crate::store::runtime;

/// Write-with-dedup: checks for a duplicate, writes only when none is found.
/// Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    let pool = memory_pool_sync()?;
    runtime::block_on(async {
        let gn = node_to_graph(node.clone());

        // Check for existing node with same title within the time window
        let window_secs = window_hours as u64 * 3600;
        let now_secs = parse_iso_to_secs(&now_iso());
        let cutoff_secs = now_secs.saturating_sub(window_secs);
        let cutoff_ts = {
            let s = cutoff_secs;
            let sec = s % 60;
            let min = (s / 60) % 60;
            let hour = (s / 3600) % 24;
            let d = s / 86400;
            let (y, m, day) = super::util::days_to_ymd(d);
            format!("{y:04}-{m:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
        };

        let existing = sqlx::query(&format!(
            "SELECT {NODE_COLUMNS} FROM nodes WHERE title = ? AND type = ? AND updated >= ? LIMIT 1"
        ))
            .bind(&gn.title)
            .bind(&gn.node_type)
            .bind(&cutoff_ts)
            .fetch_optional(&pool)
            .await
            .map_err(io::Error::other)?;

        if let Some(row) = existing {
            let existing_gn = row_to_graph_node(&row);
            return Ok((existing_gn.id, true));
        }

        // No duplicate found — insert
        super::schema::upsert_graph_node(&pool, &gn).await?;
        Ok((gn.id, false))
    })
}

/// Async write-with-dedup using pool (delegates to sync).
pub async fn write_node_dedup_pool(
    _pool: &sqlx::AnyPool,
    node: &Node,
    window_hours: u64,
) -> io::Result<(String, bool)> {
    write_node_dedup(node, window_hours)
}
