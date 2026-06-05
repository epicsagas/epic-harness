//! decay.rs — Importance decay, stale tagging, and access tracking

use sqlx::AnyPool;
use std::io;

use super::util::{days_to_ymd, now_iso};

/// Record an access event: increment access_count and update accessed_at.
#[allow(dead_code)]
pub async fn touch_node_pool(pool: &AnyPool, id: &str) {
    let now = now_iso();
    let _ = sqlx::query(
        "UPDATE nodes SET access_count = access_count + 1, accessed_at = $1 WHERE id = $2",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await;
}

/// Async batch-touch using a single batched UPDATE.
pub async fn touch_nodes_pool(pool: &AnyPool, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let now = now_iso();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return,
    };
    // Build parameterized IN clause: UPDATE ... WHERE id IN ($2, $3, ...)
    let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
    let sql = format!(
        "UPDATE nodes SET access_count = access_count + 1, accessed_at = $1 WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut query = sqlx::query(&sql).bind(&now);
    for id in ids {
        query = query.bind(id);
    }
    if let Err(e) = query.execute(&mut *tx).await {
        eprintln!("[mem] touch_nodes_pool update failed: {e}");
        let _ = tx.rollback().await;
        return;
    }
    if let Err(e) = tx.commit().await {
        eprintln!("[mem] touch_nodes_pool commit failed: {e}");
    }
}

/// Gradually decay importance for nodes not accessed in `days`.
/// Returns the number of nodes decayed.
pub fn decay_importance(days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        decay_importance_pool(&pool, days, factor, floor).await
    })
}

/// Tag nodes not updated within `days` as stale by appending "stale" to their tags.
pub fn tag_stale_nodes(days: u64) -> io::Result<u64> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        tag_stale_nodes_pool(&pool, days).await
    })
}

/// Async importance decay using pool.
pub async fn decay_importance_pool(
    pool: &AnyPool,
    days: u64,
    factor: f64,
    floor: f64,
) -> io::Result<u64> {
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
        "UPDATE nodes SET importance = MAX($5, importance * $2)
         WHERE (accessed_at < $1 OR accessed_at = '')
           AND updated < $3
           AND importance > $4
           AND ',' || tags || ',' NOT LIKE '%,pinned,%'",
    )
    .bind(&cutoff)
    .bind(factor)
    .bind(&cutoff)
    .bind(floor)
    .bind(floor)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected())
}

/// Async stale tagging using pool.
pub async fn tag_stale_nodes_pool(pool: &AnyPool, days: u64) -> io::Result<u64> {
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
         WHERE updated < $1
           AND ',' || tags || ',' NOT LIKE '%,stale,%'",
    )
    .bind(&cutoff)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected())
}
