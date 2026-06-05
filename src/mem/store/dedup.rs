//! dedup.rs — Node deduplication logic

use sqlx::SqlitePool;
use std::io;

use super::node::write_node_pool;
use super::types::Node;
use super::util::days_to_ymd;

/// Write-with-dedup: checks for a duplicate, writes only when none is found.
/// Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        write_node_dedup_pool(&pool, node, window_hours).await
    })
}

/// Async write-with-dedup using a sqlx pool.
pub async fn write_node_dedup_pool(
    pool: &SqlitePool,
    node: &Node,
    window_hours: u64,
) -> io::Result<(String, bool)> {
    let title = &node.frontmatter.title;

    if let Some(existing_id) = find_duplicate_in_pool(pool, title, window_hours).await? {
        return Ok((existing_id, true));
    }

    write_node_pool(pool, node).await?;
    Ok((node.frontmatter.id.clone(), false))
}

async fn find_duplicate_in_pool(
    pool: &SqlitePool,
    title: &str,
    window_hours: u64,
) -> io::Result<Option<String>> {
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(window_hours * 3600);
    let cutoff = {
        let s = cutoff_secs;
        let (y, m, d) = days_to_ymd(s / 86400);
        let hh = (s / 3600) % 24;
        let mm = (s / 60) % 60;
        let ss = s % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let result = sqlx::query_scalar::<_, String>(
        "SELECT id FROM nodes WHERE title = ? AND updated > ? ORDER BY updated DESC LIMIT 1",
    )
    .bind(title)
    .bind(&cutoff)
    .fetch_optional(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result)
}
