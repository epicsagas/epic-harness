use epic_harness::mem::store::{query_nodes_pool, search_nodes_pool, smart_recall_pool};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

const MAX_QUERY_LEN: usize = 200;
const MAX_SEARCH_LIMIT: usize = 100;
const MAX_QUERY_LIMIT: usize = 200;

const VALID_NODE_TYPES: &[&str] = &[
    "decision",
    "resolution",
    "psychographic",
    "instinct",
    "concept",
    "project",
    "pattern",
    "error",
    "session",
];

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub snippet: String,
}

#[derive(Deserialize)]
pub struct QueryFilter {
    pub tag: Option<String>,
    pub node_type: Option<String>,
    pub project: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct ScoredNodeResponse {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub score: f64,
    pub body: String,
    pub tags: Vec<String>,
    pub importance: f64,
}

#[tauri::command]
pub async fn search_nodes(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    if query.len() > MAX_QUERY_LEN {
        return Err("search query too long".to_string());
    }
    let limit = limit.unwrap_or(20).min(MAX_SEARCH_LIMIT) as i64;
    let pool = state.db.clone();
    let nodes = search_nodes_pool(&pool, &query, limit)
        .await
        .map_err(|e| format!("failed to search nodes: {e}"))?;
    Ok(nodes
        .into_iter()
        .map(|n| SearchResult {
            id: n.frontmatter.id,
            title: n.frontmatter.title,
            node_type: n.frontmatter.node_type,
            snippet: n.body.chars().take(160).collect(),
        })
        .collect())
}

#[tauri::command]
pub async fn query_nodes(
    filter: QueryFilter,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    if let Some(ref nt) = filter.node_type
        && !VALID_NODE_TYPES.contains(&nt.as_str())
    {
        return Err(format!(
            "invalid node_type: must be one of {}",
            VALID_NODE_TYPES.join(", ")
        ));
    }
    let limit = filter.limit.unwrap_or(200).min(MAX_QUERY_LIMIT);
    let pool = state.db.clone();
    let nodes = query_nodes_pool(
        &pool,
        filter.tag.as_deref(),
        filter.node_type.as_deref(),
        filter.project.as_deref(),
        limit,
    )
    .await
    .map_err(|e| format!("failed to query nodes: {e}"))?;
    Ok(serde_json::to_value(&nodes).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub async fn recall_nodes(
    project: Option<String>,
    hint: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<ScoredNodeResponse>, String> {
    let limit = limit.unwrap_or(10).min(50);
    let pool = state.db.clone();
    let scored = smart_recall_pool(&pool, project.as_deref(), hint.as_deref(), limit)
        .await
        .map_err(|e| format!("failed to recall nodes: {e}"))?;
    Ok(scored
        .into_iter()
        .map(|sn| ScoredNodeResponse {
            id: sn.node.frontmatter.id,
            title: sn.node.frontmatter.title,
            node_type: sn.node.frontmatter.node_type,
            score: sn.score,
            body: sn.node.body,
            tags: sn.node.frontmatter.tags,
            importance: sn.node.frontmatter.importance,
        })
        .collect())
}
