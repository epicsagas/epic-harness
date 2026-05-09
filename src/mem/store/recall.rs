//! recall.rs — Smart recall with composite scoring and graph boost

use rusqlite::Connection;
use std::io;

use super::decay::touch_nodes_conn;
use super::search::search_nodes_conn;
use super::types::{Node, ScoredNode};
use super::util::{NODE_COLUMNS, parse_iso_to_secs};

/// Composite relevance score weights.
pub const W_RECENCY: f64 = 0.20; // was 0.25 — reduced to make room for graph boost
pub const W_IMPORTANCE: f64 = 0.35;
pub const W_ACCESS: f64 = 0.15;
pub const W_FTS: f64 = 0.20; // was 0.25
pub const W_GRAPH: f64 = 0.10; // edge-weight connectivity boost (new)

/// Smart recall using an existing connection.
pub fn smart_recall_conn(
    conn: &Connection,
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
            search_nodes_conn(conn, h, limit * 4)?
                .into_iter()
                .map(|n| n.frontmatter.id.clone())
                .collect()
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };

    // Fetch candidate nodes (broad set); candidate_limit is computed, not user input.
    let candidate_limit = (limit * 4).max(40) as i64;
    let mut conditions: Vec<&str> = vec!["',' || tags || ',' NOT LIKE '%,stale,%'"];
    // Collect bound parameter values alongside conditions.
    let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(p) = project {
        conditions.push("(',' || projects || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(p.to_string()));
    }
    let where_clause = format!("WHERE {}", conditions.join(" AND "));
    let sql = format!(
        "SELECT {NODE_COLUMNS} FROM nodes {where_clause}
         ORDER BY importance DESC, updated DESC
         LIMIT {candidate_limit}"
    );

    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|b| b.as_ref()).collect();
    let candidates: Vec<Node> = stmt
        .query_map(refs.as_slice(), super::util::row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

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

    // Graph-boost pass: add W_GRAPH * edge_weight_score for edges between top candidates.
    if scored.len() > 1 {
        const MAX_GRAPH_BOOST_PARTICIPANTS: usize = 100;
        let boost_ids: Vec<&str> = scored
            .iter()
            .take(MAX_GRAPH_BOOST_PARTICIPANTS)
            .map(|sn| sn.node.frontmatter.id.as_str())
            .collect();
        let ph = boost_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // Sum edge weights for each node connected to other top-scored candidates.
        let sql = format!(
            "SELECT source AS node_id, SUM(weight) AS w FROM edges \
             WHERE source IN ({ph}) AND target IN ({ph}) GROUP BY source \
             UNION ALL \
             SELECT target AS node_id, SUM(weight) AS w FROM edges \
             WHERE source IN ({ph}) AND target IN ({ph}) GROUP BY target"
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let base: Vec<&dyn rusqlite::ToSql> =
                boost_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let sql_params: Vec<&dyn rusqlite::ToSql> = base
                .iter()
                .copied()
                .chain(base.iter().copied())
                .chain(base.iter().copied())
                .chain(base.iter().copied())
                .collect();
            let weight_map: std::collections::HashMap<String, f64> = stmt
                .query_map(sql_params.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                })
                .map(|rows| {
                    let mut map: std::collections::HashMap<String, f64> = Default::default();
                    for r in rows.flatten() {
                        *map.entry(r.0).or_default() += r.1;
                    }
                    map
                })
                .unwrap_or_default();
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
            // Re-sort after boost
            scored.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    // Touch retrieved nodes (batch)
    let ids: Vec<String> = scored.iter().map(|sn| sn.node.frontmatter.id.clone()).collect();
    touch_nodes_conn(conn, &ids);

    Ok(scored)
}

/// Smart recall: returns nodes ranked by composite relevance.
///
/// Opens its own connection; for repeated calls within a single session prefer
/// `smart_recall_conn` to reuse an already-open connection.
pub fn smart_recall(project: Option<&str>, hint: Option<&str>, limit: usize) -> io::Result<Vec<ScoredNode>> {
    let conn = super::open_db()?;
    smart_recall_conn(&conn, project, hint, limit)
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
