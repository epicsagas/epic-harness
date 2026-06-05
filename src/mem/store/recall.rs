//! recall.rs — Smart recall with composite scoring and graph boost

use sqlx::{AnyPool, Row};
use std::io;

use super::decay::touch_nodes_pool;
use super::node::row_to_node_pool;
use super::search::search_nodes_pool;
use super::types::{Node, ScoredNode};
use super::util::{NODE_COLUMNS, escape_like, parse_iso_to_secs};

/// Composite relevance score weights.
pub const W_RECENCY: f64 = 0.20; // was 0.25 — reduced to make room for graph boost
pub const W_IMPORTANCE: f64 = 0.35;
pub const W_ACCESS: f64 = 0.15;
pub const W_FTS: f64 = 0.20; // was 0.25
pub const W_GRAPH: f64 = 0.10; // edge-weight connectivity boost (new)

/// Smart recall: returns nodes ranked by composite relevance.
pub fn smart_recall(
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    // Clone to static strings so they can be sent across the thread boundary.
    let project = project.map(|s| s.to_string());
    let hint = hint.map(|s| s.to_string());
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        smart_recall_pool(&pool, project.as_deref(), hint.as_deref(), limit).await
    })
}

/// Compute recency score (0.0–1.0) with exponential decay, half-life = 30 days.
pub(crate) fn compute_recency(updated: &str, now_secs: u64) -> f64 {
    let node_secs = parse_iso_to_secs(updated);
    if node_secs == 0 || node_secs > now_secs {
        return 0.5; // unknown or future timestamp
    }
    let age_days = (now_secs - node_secs) as f64 / 86400.0;
    let half_life = 30.0;
    (-age_days * (2.0_f64.ln()) / half_life).exp()
}

// ── Async pool functions ─────────────────────────────

/// Async smart recall using a sqlx pool.
pub async fn smart_recall_pool(
    pool: &AnyPool,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> io::Result<Vec<ScoredNode>> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Gather FTS matches if hint is provided
    let fts_ids: std::collections::HashSet<String> = if let Some(h) = hint {
        if !h.is_empty() {
            search_nodes_pool(pool, h, (limit * 4) as i64)
                .await?
                .into_iter()
                .map(|n| n.frontmatter.id.clone())
                .collect()
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };

    // Fetch candidate nodes using QueryBuilder
    let candidate_limit = (limit * 4).max(40) as i64;
    let mut qb = sqlx::QueryBuilder::new(format!(
        "SELECT {NODE_COLUMNS} FROM nodes WHERE ',' || tags || ',' NOT LIKE '%,stale,%'"
    ));
    if let Some(p) = project {
        qb.push(" AND (',' || projects || ',' LIKE '%,' || ");
        qb.push_bind(escape_like(p));
        qb.push(" || ',%' ESCAPE '\\')");
    }
    qb.push(" ORDER BY importance DESC, updated DESC LIMIT ");
    qb.push_bind(candidate_limit);

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    let candidates: Vec<Node> = rows
        .iter()
        .filter_map(|r| row_to_node_pool(r).ok())
        .collect();

    // Score each candidate
    let mut scored: Vec<ScoredNode> = candidates
        .into_iter()
        .map(|node| {
            let recency = compute_recency(&node.frontmatter.updated, now_secs);
            let importance = node.frontmatter.importance;
            let access_freq = (node.frontmatter.access_count.max(0) as f64 / 20.0).min(1.0);
            let fts_match = if fts_ids.contains(&node.frontmatter.id) {
                1.0
            } else {
                0.0
            };

            let score = W_RECENCY * recency
                + W_IMPORTANCE * importance
                + W_ACCESS * access_freq
                + W_FTS * fts_match;

            ScoredNode { node, score }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);

    // Graph-boost pass
    if scored.len() > 1 {
        const MAX_GRAPH_BOOST_PARTICIPANTS: usize = 100;
        let boost_ids: Vec<&str> = scored
            .iter()
            .take(MAX_GRAPH_BOOST_PARTICIPANTS)
            .map(|sn| sn.node.frontmatter.id.as_str())
            .collect();

        let mut qb = sqlx::QueryBuilder::new(
            "SELECT source AS node_id, SUM(weight) AS w FROM edges WHERE source IN (",
        );
        let mut separated = qb.separated(", ");
        for id in &boost_ids {
            separated.push_bind(*id);
        }
        qb.push(") AND target IN (");
        let mut separated2 = qb.separated(", ");
        for id in &boost_ids {
            separated2.push_bind(*id);
        }
        qb.push(") GROUP BY source UNION ALL SELECT target AS node_id, SUM(weight) AS w FROM edges WHERE source IN (");
        let mut separated3 = qb.separated(", ");
        for id in &boost_ids {
            separated3.push_bind(*id);
        }
        qb.push(") AND target IN (");
        let mut separated4 = qb.separated(", ");
        for id in &boost_ids {
            separated4.push_bind(*id);
        }
        qb.push(") GROUP BY target");

        if let Ok(rows) = qb.build().fetch_all(pool).await {
            let mut weight_map: std::collections::HashMap<String, f64> = Default::default();
            for r in &rows {
                if let (Ok(nid), Ok(w)) = (r.try_get::<String, _>(0), r.try_get::<f64, _>(1)) {
                    *weight_map.entry(nid).or_default() += w;
                }
            }
            let max_w = weight_map
                .values()
                .cloned()
                .fold(0.0_f64, f64::max)
                .max(1.0);
            for sn in &mut scored {
                let boost = weight_map
                    .get(&sn.node.frontmatter.id)
                    .copied()
                    .unwrap_or(0.0);
                sn.score += W_GRAPH * (boost / max_w);
            }
            scored.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    // Touch retrieved nodes (batch)
    let ids: Vec<String> = scored
        .iter()
        .map(|sn| sn.node.frontmatter.id.clone())
        .collect();
    touch_nodes_pool(pool, &ids).await;

    Ok(scored)
}
