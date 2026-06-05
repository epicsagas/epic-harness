//! decay.rs — Importance decay, stale tagging, and access tracking

use rusqlite::{Connection, params};
use sqlx::SqlitePool;
use std::io;

use super::util::{days_to_ymd, now_iso};

/// Record an access event: increment access_count and update accessed_at.
pub fn touch_node_conn(conn: &Connection, id: &str) {
    let now = now_iso();
    let _ = conn.execute(
        "UPDATE nodes SET access_count = access_count + 1, accessed_at = ?1 WHERE id = ?2",
        params![now, id],
    );
}

/// Batch-touch multiple nodes using an existing connection.
pub fn touch_nodes_conn(conn: &Connection, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let _ = conn.execute_batch("SAVEPOINT touch_batch");
    for id in ids {
        touch_node_conn(conn, id);
    }
    let _ = conn.execute_batch("RELEASE touch_batch");
}

/// Gradually decay importance for nodes not accessed in `days`.
/// Instead of binary stale tagging, reduces importance by `factor` (e.g., 0.9 = 10% decay).
/// Nodes with importance already at or below `floor` are not decayed further.
/// Returns the number of nodes decayed.
pub fn decay_importance(days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    let conn = super::open_db()?;
    let cutoff = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(days * 86400);
        let (y, m, d) = days_to_ymd(secs / 86400);
        let hh = (secs / 3600) % 24;
        let mm = (secs / 60) % 60;
        let ss = secs % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let changed = conn
        .execute(
            "UPDATE nodes SET importance = MAX(?3, importance * ?2)
         WHERE (accessed_at < ?1 OR accessed_at = '')
           AND updated < ?1
           AND importance > ?3
           AND ',' || tags || ',' NOT LIKE '%,pinned,%'",
            params![cutoff, factor, floor],
        )
        .map_err(io::Error::other)?;
    Ok(changed as u64)
}

/// Tag nodes not updated within `days` as stale by appending "stale" to their tags.
pub fn tag_stale_nodes(days: u64) -> io::Result<u64> {
    let conn = super::open_db()?;
    // Compute cutoff timestamp in Rust to avoid format!() SQL interpolation.
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(days * 86400);
    let cutoff = {
        let s = cutoff_secs;
        let (y, m, d) = days_to_ymd(s / 86400);
        let hh = (s / 3600) % 24;
        let mm = (s / 60) % 60;
        let ss = s % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let changed = conn
        .execute(
            "UPDATE nodes SET tags = CASE
            WHEN tags = '' THEN 'stale'
            WHEN ',' || tags || ',' NOT LIKE '%,stale,%' THEN tags || ',stale'
            ELSE tags
         END
         WHERE updated < ?1
           AND ',' || tags || ',' NOT LIKE '%,stale,%'",
            params![cutoff],
        )
        .map_err(io::Error::other)?;
    Ok(changed as u64)
}

// ── Async pool functions ─────────────────────────────

/// Async batch-touch using a transaction (replaces SAVEPOINT/RELEASE).
pub async fn touch_nodes_pool(pool: &SqlitePool, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return,
    };
    let now = now_iso();
    for id in ids {
        let _ = sqlx::query(
            "UPDATE nodes SET access_count = access_count + 1, accessed_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await;
    }
    let _ = tx.commit().await;
}

/// Async importance decay using pool.
pub async fn decay_importance_pool(pool: &SqlitePool, days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    let cutoff = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(days * 86400);
        let (y, m, d) = days_to_ymd(secs / 86400);
        let hh = (secs / 3600) % 24;
        let mm = (secs / 60) % 60;
        let ss = secs % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let result = sqlx::query(
        "UPDATE nodes SET importance = MAX(?, importance * ?)
         WHERE (accessed_at < ? OR accessed_at = '')
           AND updated < ?
           AND importance > ?
           AND ',' || tags || ',' NOT LIKE '%,pinned,%'",
    )
    .bind(&cutoff)
    .bind(factor)
    .bind(&cutoff)
    .bind(&cutoff)
    .bind(floor)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(result.rows_affected())
}

/// Async stale tagging using pool.
pub async fn tag_stale_nodes_pool(pool: &SqlitePool, days: u64) -> io::Result<u64> {
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(days * 86400);
    let cutoff = {
        let s = cutoff_secs;
        let (y, m, d) = days_to_ymd(s / 86400);
        let hh = (s / 3600) % 24;
        let mm = (s / 60) % 60;
        let ss = s % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let result = sqlx::query(
        "UPDATE nodes SET tags = CASE
            WHEN tags = '' THEN 'stale'
            WHEN ',' || tags || ',' NOT LIKE '%,stale,%' THEN tags || ',stale'
            ELSE tags
         END
         WHERE updated < ?
           AND ',' || tags || ',' NOT LIKE '%,stale,%'",
    )
    .bind(&cutoff)
    .execute(pool)
    .await
    .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(result.rows_affected())
}
