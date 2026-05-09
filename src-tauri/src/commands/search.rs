use crate::state::AppState;
use epic_harness::mem::store::{query_nodes_conn, search_nodes_conn, smart_recall_conn};
use serde::{Deserialize, Serialize};
use tauri::State;

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
pub fn search_nodes(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let nodes = search_nodes_conn(&conn, &query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())?;
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
pub fn query_nodes(filter: QueryFilter, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let nodes = query_nodes_conn(
        &conn,
        filter.tag.as_deref(),
        filter.node_type.as_deref(),
        filter.project.as_deref(),
        filter.limit.unwrap_or(200),
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&nodes).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub fn recall_nodes(
    project: Option<String>,
    hint: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<ScoredNodeResponse>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let scored = smart_recall_conn(
        &conn,
        project.as_deref(),
        hint.as_deref(),
        limit.unwrap_or(10),
    ).map_err(|e| e.to_string())?;
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
