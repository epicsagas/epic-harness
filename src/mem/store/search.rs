//! search.rs — FTS search and dynamic filter queries

use std::io;

use sqlx::SqlitePool;

use super::node::row_to_node_pool;
use super::types::Node;
use super::util::{NODE_COLUMNS, NODE_COLUMNS_PREFIXED, escape_like};

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let query = query.to_string();
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        search_nodes_pool(&pool, &query, limit as i64).await
    })
    .unwrap_or_else(|_| vec![])
}

pub async fn search_nodes_pool(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> io::Result<Vec<Node>> {
    let sql = format!(
        "SELECT n.{NODE_COLUMNS_PREFIXED}
         FROM nodes n
         JOIN nodes_fts ON n.rowid = nodes_fts.rowid
         WHERE nodes_fts MATCH ?
         ORDER BY n.importance DESC
         LIMIT ?"
    );
    let rows = sqlx::query(&sql)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_node_pool).collect()
}

/// Dynamic filter query.
pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let tag = tag.map(|s| s.to_string());
    let node_type = node_type.map(|s| s.to_string());
    let project = project.map(|s| s.to_string());
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        query_nodes_pool(
            &pool,
            tag.as_deref(),
            node_type.as_deref(),
            project.as_deref(),
            limit,
        )
        .await
    })
    .unwrap_or_else(|_| vec![])
}

/// Async dynamic filter query using QueryBuilder.
pub async fn query_nodes_pool(
    pool: &SqlitePool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
    let limit = limit.min(200) as i64;

    let mut qb = sqlx::QueryBuilder::new(format!("SELECT {NODE_COLUMNS} FROM nodes WHERE 1=1"));

    if let Some(t) = tag {
        qb.push(" AND (',' || tags || ',' LIKE '%,' || ");
        qb.push_bind(escape_like(t));
        qb.push(" || ',%' ESCAPE '\\')");
    }
    if let Some(nt) = node_type {
        qb.push(" AND type = ");
        qb.push_bind(nt);
    }
    if let Some(p) = project {
        qb.push(" AND (',' || projects || ',' LIKE '%,' || ");
        qb.push_bind(escape_like(p));
        qb.push(" || ',%' ESCAPE '\\')");
    }

    qb.push(" ORDER BY updated DESC LIMIT ");
    qb.push_bind(limit);

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_node_pool).collect()
}
