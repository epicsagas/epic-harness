//! Native memory search data contract, independent of the desktop runtime.

use epic_harness::mem::store::search_nodes_filtered_pool;
use serde::Serialize;

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub agents: Vec<String>,
    pub created: String,
    pub updated: String,
    pub importance: f64,
    pub access_count: i64,
    pub body: String,
}

pub async fn search_nodes(
    pool: &sqlx::AnyPool,
    query: &str,
    limit: Option<usize>,
    node_type: Option<&str>,
) -> Result<Vec<SearchResult>, String> {
    if let Some(node_type) = node_type.filter(|node_type| !node_type.is_empty())
        && !matches!(
            node_type,
            "decision"
                | "resolution"
                | "concept"
                | "project"
                | "pattern"
                | "error"
                | "session"
                | "instinct"
                | "psychographic"
        )
    {
        return Err("invalid node type".into());
    }
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    if query.len() > 200 {
        return Err("search query too long".into());
    }
    let nodes = search_nodes_filtered_pool(
        pool,
        query,
        limit.unwrap_or(20).min(100) as i64,
        node_type.filter(|node_type| !node_type.is_empty()),
    )
    .await
    .map_err(|error| format!("failed to search nodes: {error}"))?;
    Ok(nodes
        .into_iter()
        .map(|node| SearchResult {
            id: node.frontmatter.id,
            title: node.frontmatter.title,
            node_type: node.frontmatter.node_type,
            snippet: node.body.chars().take(160).collect(),
            tags: node.frontmatter.tags,
            projects: node.frontmatter.projects,
            agents: node.frontmatter.agents,
            created: node.frontmatter.created,
            updated: node.frontmatter.updated,
            importance: node.frontmatter.importance,
            access_count: node.frontmatter.access_count,
            body: node.body,
        })
        .collect())
}
