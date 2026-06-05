//! node.rs — Node CRUD operations

use sqlx::{Row, SqlitePool};
use std::io;

use super::types::Node;
use super::util::{NODE_COLUMNS, join_csv};

pub fn write_node(node: &Node) -> io::Result<()> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        write_node_pool(&pool, node).await
    })
}

pub fn read_node(id: &str) -> io::Result<Node> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        read_node_pool(&pool, id).await
    })
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        delete_node_pool(&pool, id).await
    })
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        list_node_ids_pool(&pool).await
    })
}

// ── Node serialization (kept for migrate import) ──────

pub fn serialize_node(node: &Node) -> String {
    let fm = serde_yaml::to_string(&node.frontmatter).unwrap_or_default();
    format!("---\n{}---\n{}", fm, node.body)
}

pub fn parse_node(content: &str) -> Option<Node> {
    use super::types::NodeFrontmatter;
    let content = content.strip_prefix("---\n").unwrap_or(content);
    let (fm_str, body) = content.split_once("\n---\n")?;
    let frontmatter: NodeFrontmatter = serde_yaml::from_str(fm_str).ok()?;
    Some(Node {
        frontmatter,
        body: body.to_string(),
    })
}

// ── Async pool functions ─────────────────────────────

pub async fn write_node_pool(pool: &SqlitePool, node: &Node) -> io::Result<()> {
    let fm = &node.frontmatter;
    sqlx::query(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&fm.id)
    .bind(&fm.node_type)
    .bind(&fm.title)
    .bind(join_csv(&fm.tags))
    .bind(join_csv(&fm.projects))
    .bind(join_csv(&fm.agents))
    .bind(&fm.created)
    .bind(&fm.updated)
    .bind(&node.body)
    .bind(fm.importance)
    .bind(fm.access_count)
    .bind(&fm.accessed_at)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

pub async fn read_node_pool(pool: &SqlitePool, id: &str) -> io::Result<Node> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("node not found: {id}")))?;
    row_to_node_pool(&row)
}

pub async fn read_nodes_pool(pool: &SqlitePool, ids: &[&str]) -> io::Result<Vec<Node>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut qb = sqlx::QueryBuilder::new(format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id IN ("));
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(*id);
    }
    qb.push(")");
    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_node_pool).collect()
}

pub async fn delete_node_pool(pool: &SqlitePool, id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM nodes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[allow(dead_code)]
pub async fn node_exists_pool(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM nodes WHERE id = ?)")
        .bind(id)
        .fetch_one(pool)
        .await
        .is_ok_and(|v| v != 0)
}

pub async fn read_all_nodes_pool(pool: &SqlitePool) -> io::Result<Vec<Node>> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_node_pool).collect()
}

pub async fn read_nodes_limited_pool(pool: &SqlitePool, limit: i64) -> io::Result<Vec<Node>> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC LIMIT ?");
    let rows = sqlx::query(&sql)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter().map(row_to_node_pool).collect()
}

pub async fn list_node_ids_pool(pool: &SqlitePool) -> io::Result<Vec<String>> {
    let rows = sqlx::query("SELECT id FROM nodes")
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(rows
        .iter()
        .filter_map(|r| r.try_get::<String, _>(0).ok())
        .collect())
}

/// Map an sqlx row to a Node. Column order matches NODE_COLUMNS.
pub(crate) fn row_to_node_pool(row: &sqlx::sqlite::SqliteRow) -> io::Result<Node> {
    use super::types::NodeFrontmatter;
    let tags: String = row.try_get(3).map_err(crate::store::sqlx_err)?;
    let projects: String = row.try_get(4).map_err(crate::store::sqlx_err)?;
    let agents: String = row.try_get(5).map_err(crate::store::sqlx_err)?;
    Ok(Node {
        frontmatter: NodeFrontmatter {
            id: row.try_get(0).map_err(crate::store::sqlx_err)?,
            node_type: row.try_get(1).map_err(crate::store::sqlx_err)?,
            title: row.try_get(2).map_err(crate::store::sqlx_err)?,
            tags: super::util::split_csv(&tags),
            projects: super::util::split_csv(&projects),
            agents: super::util::split_csv(&agents),
            created: row.try_get(6).map_err(crate::store::sqlx_err)?,
            updated: row.try_get(7).map_err(crate::store::sqlx_err)?,
            importance: row.try_get(9).unwrap_or(0.5),
            access_count: row.try_get::<i64, _>(10).unwrap_or(0),
            accessed_at: row.try_get(11).unwrap_or_default(),
        },
        body: row.try_get(8).map_err(crate::store::sqlx_err)?,
    })
}
