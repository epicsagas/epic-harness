//! decay.rs — Importance decay, stale tagging, and access tracking via sqlx

use std::io;

use super::conn::memory_pool_sync;
use super::util::now_iso;
use crate::store::runtime;

pub fn decay_importance(days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    let pool = memory_pool_sync()?;
    runtime::block_on(decay_importance_async(&pool, days, factor, floor))
}

async fn decay_importance_async(pool: &sqlx::AnyPool, days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    // Decay importance for nodes not accessed in `days` days.
    // Excludes 'session' type (already at floor 0.05) and pinned nodes.
    // Calculate cutoff timestamp
    let now_secs = super::util::parse_iso_to_secs(&now_iso());
    let cutoff_secs = now_secs.saturating_sub(days * 86400);
    // Reconstruct ISO timestamp from cutoff_secs
    let cutoff_ts = {
        let s = cutoff_secs;
        let sec = s % 60;
        let min = (s / 60) % 60;
        let hour = (s / 3600) % 24;
        let d = s / 86400;
        let (y, m, day) = super::util::days_to_ymd(d);
        format!("{y:04}-{m:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
    };

    let result = sqlx::query(
        "UPDATE nodes SET importance = MAX(importance * ?, ?) \
         WHERE accessed_at < ? AND accessed_at != '' \
         AND type != 'session' AND tags NOT LIKE '%pinned%'"
    )
        .bind(factor)
        .bind(floor)
        .bind(&cutoff_ts)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;

    Ok(result.rows_affected())
}

pub fn tag_stale_nodes(days: u64) -> io::Result<u64> {
    let pool = memory_pool_sync()?;
    runtime::block_on(tag_stale_nodes_async(&pool, days))
}

async fn tag_stale_nodes_async(pool: &sqlx::AnyPool, days: u64) -> io::Result<u64> {
    let now_secs = super::util::parse_iso_to_secs(&now_iso());
    let cutoff_secs = now_secs.saturating_sub(days * 86400);
    let cutoff_ts = {
        let s = cutoff_secs;
        let sec = s % 60;
        let min = (s / 60) % 60;
        let hour = (s / 3600) % 24;
        let d = s / 86400;
        let (y, m, day) = super::util::days_to_ymd(d);
        format!("{y:04}-{m:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
    };

    let result = sqlx::query(
        "UPDATE nodes SET tags = CASE \
         WHEN tags = '' THEN 'stale' \
         ELSE tags || ',stale' END \
         WHERE updated < ? AND updated != '' \
         AND type != 'session' AND tags NOT LIKE '%pinned%' \
         AND tags NOT LIKE '%stale%'"
    )
        .bind(&cutoff_ts)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;

    Ok(result.rows_affected())
}

pub fn touch_nodes_pool(_pool: &sqlx::AnyPool, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    // This sync function still needs its own pool for callers outside tokio.
    // For async callers, use touch_nodes_async from recall.rs instead.
    let owned_pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(_) => return,
    };
    runtime::block_on(async {
        let now = now_iso();
        for id in ids {
            let _ = sqlx::query(
                "UPDATE nodes SET access_count = access_count + 1, accessed_at = ? WHERE id = ?"
            )
                .bind(&now)
                .bind(id)
                .execute(&owned_pool)
                .await;
        }
    });
}

// ── Pool-compatible wrappers ─────────────────────────────────

#[allow(dead_code)]
pub async fn decay_importance_pool(
    pool: &sqlx::AnyPool,
    days: u64,
    factor: f64,
    floor: f64,
) -> io::Result<u64> {
    decay_importance_async(pool, days, factor, floor).await
}

#[allow(dead_code)]
pub async fn tag_stale_nodes_pool(pool: &sqlx::AnyPool, days: u64) -> io::Result<u64> {
    tag_stale_nodes_async(pool, days).await
}
