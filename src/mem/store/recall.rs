//! recall.rs — Smart recall with composite scoring via direct SQL
//!
//! Scoring formula: recency(25%) + importance(35%) + access_freq(15%) + FTS_match(25%)
//! Recency uses exponential decay with 30-day half-life.

use std::io;

use sqlx::Row as _;

use super::conn::memory_pool_sync;
use super::types::{ScoredNode, graph_to_node};
use super::util::{NODE_COLUMNS, now_iso, parse_iso_to_secs, row_to_graph_node};
use crate::store::runtime;

/// Half-life in seconds for recency decay (30 days).
const HALF_LIFE_SECS: f64 = 30.0 * 86400.0;

pub fn smart_recall(
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    let pool = memory_pool_sync()?;
    runtime::block_on(smart_recall_async(&pool, project, hint, limit))
}

/// Async core — shared by sync wrapper and pool variant.
pub(crate) async fn smart_recall_async(
    pool: &sqlx::AnyPool,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    let now_secs = parse_iso_to_secs(&now_iso()) as f64;

    // Step 1: Get candidate nodes filtered by project and/or FTS hint
    let candidates = if let Some(query) = hint {
        let fts_query = escape_fts(query);
        // Get matching rowids from FTS, then fetch full nodes — avoids JOIN column ambiguity
        // with sqlx AnyPool which doesn't support FTS5 virtual table column access reliably.
        let rowids: Vec<i64> = sqlx::query(
            "SELECT rowid FROM nodes_fts WHERE nodes_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(&fts_query)
        .bind(limit as i64 * 3)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r.try_get::<i64, _>(0).ok())
                .collect()
        })
        .unwrap_or_default();

        if rowids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = rowids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        if let Some(proj) = project {
            let csv_proj = format!("%{proj}%");
            let sql = format!(
                "SELECT {NODE_COLUMNS} FROM nodes WHERE rowid IN ({placeholders}) AND projects LIKE ? \
                 ORDER BY importance DESC"
            );
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
            for rid in &rowids {
                q = q.bind(*rid);
            }
            q.bind(&csv_proj).fetch_all(pool).await
        } else {
            let sql = format!(
                "SELECT {NODE_COLUMNS} FROM nodes WHERE rowid IN ({placeholders}) \
                 ORDER BY importance DESC"
            );
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
            for rid in &rowids {
                q = q.bind(*rid);
            }
            q.fetch_all(pool).await
        }
    } else if let Some(proj) = project {
        let csv_proj = format!("%{proj}%");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {NODE_COLUMNS} FROM nodes WHERE projects LIKE ? \
             ORDER BY importance DESC LIMIT ?"
        )))
        .bind(&csv_proj)
        .bind(limit as i64 * 3)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {NODE_COLUMNS} FROM nodes ORDER BY importance DESC LIMIT ?"
        )))
        .bind(limit as i64 * 3)
        .fetch_all(pool)
        .await
    };

    let rows = candidates.map_err(io::Error::other)?;

    // Step 2: Score each candidate
    let mut scored: Vec<ScoredNode> = rows
        .iter()
        .map(|r| {
            let gn = row_to_graph_node(r);
            let node = graph_to_node(gn.clone());

            // Recency: exponential decay
            let updated_secs = parse_iso_to_secs(&gn.updated) as f64;
            let age_secs = (now_secs - updated_secs).max(0.0);
            let recency = (-age_secs * 0.693 / HALF_LIFE_SECS).exp();

            // Importance: direct
            let importance = gn.importance;

            // Access frequency: saturates at 20 accesses
            let access_freq = (gn.access_count as f64 / 20.0).min(1.0);

            // FTS match: 1.0 if hint was provided and matched, 0.0 otherwise
            let fts_match = if hint.is_some() { 1.0 } else { 0.0 };

            let score = 0.25 * recency + 0.35 * importance + 0.15 * access_freq + 0.25 * fts_match;

            ScoredNode { node, score }
        })
        .collect();

    // Step 3: Sort by score descending, truncate to limit
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);

    // Step 4: Touch accessed nodes (increment access_count)
    let ids: Vec<String> = scored
        .iter()
        .map(|s| s.node.frontmatter.id.clone())
        .collect();
    if !ids.is_empty() {
        let _ = touch_nodes_async(pool, &ids).await;
    }

    Ok(scored)
}

/// Increment access_count and update accessed_at for the given node IDs.
async fn touch_nodes_async(pool: &sqlx::AnyPool, ids: &[String]) -> io::Result<()> {
    let now = now_iso();
    for id in ids {
        sqlx::query(
            "UPDATE nodes SET access_count = access_count + 1, accessed_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;
    }
    Ok(())
}

/// Escape special FTS5 characters in a query string.
fn escape_fts(s: &str) -> String {
    // For FTS5, wrap terms in quotes if they contain special chars
    let sanitized: String = s
        .replace(['"', '*'], "")
        .replace(':', " ")
        .replace("AND", "")
        .replace("OR", "")
        .replace("NOT", "")
        .replace(['^', '{', '}', '(', ')'], "")
        .trim()
        .to_string();

    if sanitized.is_empty() {
        return "*".to_string(); // match all
    }

    // Split into words and join with OR for broad matching
    let words: Vec<&str> = sanitized.split_whitespace().take(5).collect();
    if words.is_empty() {
        return "*".to_string();
    }
    words
        .iter()
        .map(|w| format!("\"{}\"", w))
        .collect::<Vec<_>>()
        .join(" OR ")
}

// ── Pool-compatible wrappers ─────────────────────────────────

pub async fn smart_recall_pool(
    pool: &sqlx::AnyPool,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    smart_recall_async(pool, project, hint, limit).await
}
