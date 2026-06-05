//! dedup.rs — Node deduplication logic

use rusqlite::{Connection, params};
use sqlx::SqlitePool;
use std::io;

use super::node::{write_node_conn, write_node_pool};
use super::types::Node;
use super::util::days_to_ymd;

/// Returns the ID of an existing node with the same title updated within the
/// last `window_hours` hours.  Uses the composite idx_nodes_title_updated index
/// for O(log N) lookup.  Used to prevent duplicate writes when multiple callers
/// (observe hook + skills + direct MCP) fire for the same event.
pub(crate) fn find_duplicate_in_conn(
    conn: &Connection,
    title: &str,
    window_hours: u64,
) -> Option<String> {
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
    conn.query_row(
        "SELECT id FROM nodes WHERE title = ?1 AND updated > ?2 ORDER BY updated DESC LIMIT 1",
        params![title, cutoff],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// Write-with-dedup: opens a single connection, checks for a duplicate, and
/// writes only when none is found.  Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    let conn = super::open_db()?;
    write_node_dedup_conn(&conn, node, window_hours)
}

/// Write-with-dedup using an existing connection (for batch/transaction use).
pub fn write_node_dedup_conn(
    conn: &Connection,
    node: &Node,
    window_hours: u64,
) -> io::Result<(String, bool)> {
    let title = &node.frontmatter.title;

    if let Some(existing_id) = find_duplicate_in_conn(conn, title, window_hours) {
        return Ok((existing_id, true));
    }

    write_node_conn(conn, node)?;
    Ok((node.frontmatter.id.clone(), false))
}

// ── Async pool functions ─────────────────────────────

/// Async write-with-dedup using a sqlx pool.
#[allow(dead_code)]
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

#[allow(dead_code)]
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
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(result)
}
