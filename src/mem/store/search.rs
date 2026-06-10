//! search.rs — FTS search and dynamic filter queries via sqlx

use std::io;

use sqlx::Row;

use super::conn::memory_pool_sync;
use super::types::Node;
use super::util::{NODE_COLUMNS, row_to_node};
use crate::store::runtime;

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    runtime::block_on(async {
        // Escape FTS5 special characters
        let fts_query = escape_fts(query);
        sqlx::query(&format!(
            "SELECT {NODE_COLUMNS} FROM nodes WHERE id IN \
             (SELECT id FROM nodes_fts WHERE nodes_fts MATCH ? ORDER BY rank) \
             LIMIT ?"
        ))
            .bind(&fts_query)
            .bind(limit as i64)
            .fetch_all(&pool)
            .await
            .map(|rows| rows.iter().map(|r| row_to_node(r)).collect())
            .unwrap_or_default()
    })
}

pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    runtime::block_on(async {
        let mut sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE 1=1");
        let mut has_tag = false;
        let mut has_type = false;
        let mut has_project = false;

        if tag.is_some() {
            sql.push_str(" AND tags LIKE ?");
            has_tag = true;
        }
        if node_type.is_some() {
            sql.push_str(" AND type = ?");
            has_type = true;
        }
        if project.is_some() {
            sql.push_str(" AND projects LIKE ?");
            has_project = true;
        }
        // Cap limit at 200
        let safe_limit = (limit.min(200)) as i64;
        sql.push_str(" ORDER BY updated DESC LIMIT ?");

        let mut query = sqlx::query(&sql);
        if let Some(t) = tag {
            query = query.bind(format!("%{t}%"));
        }
        if let Some(t) = node_type {
            query = query.bind(t);
        }
        if let Some(p) = project {
            query = query.bind(format!("%{p}%"));
        }
        query = query.bind(safe_limit);

        query
            .fetch_all(&pool)
            .await
            .map(|rows| rows.iter().map(|r| row_to_node(r)).collect())
            .unwrap_or_default()
    })
}

/// Escape special FTS5 characters in a search query.
fn escape_fts(s: &str) -> String {
    let sanitized: String = s
        .replace('"', "")
        .replace('*', "")
        .replace(':', " ")
        .replace('^', "")
        .replace('{', "")
        .replace('}', "")
        .replace('(', "")
        .replace(')', "")
        .trim()
        .to_string();

    if sanitized.is_empty() {
        return "*".to_string();
    }

    let words: Vec<&str> = sanitized.split_whitespace().take(5).collect();
    if words.is_empty() {
        return "*".to_string();
    }
    words.iter().map(|w| format!("\"{}\"", w)).collect::<Vec<_>>().join(" OR ")
}

// ── Pool-compatible wrappers ─────────────────────────────────

pub async fn search_nodes_pool(
    _pool: &sqlx::AnyPool,
    query: &str,
    limit: i64,
) -> io::Result<Vec<Node>> {
    Ok(search_nodes(query, limit as usize))
}

pub async fn query_nodes_pool(
    _pool: &sqlx::AnyPool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
    Ok(query_nodes(tag, node_type, project, limit))
}
