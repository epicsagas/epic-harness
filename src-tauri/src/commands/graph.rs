use crate::state::AppState;
use epic_harness::mem::graph::{build_graph_conn, graph_neighbors_conn};
use epic_harness::mem::store::validate_uuid;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize, Deserialize)]
pub struct GraphNodeResponse {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    pub importance: f64,
}

#[derive(Serialize, Deserialize)]
pub struct GraphEdgeResponse {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
}

#[derive(Serialize, Deserialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNodeResponse>,
    pub edges: Vec<GraphEdgeResponse>,
}

#[derive(Serialize)]
pub struct NeighborResponse {
    pub id: String,
    pub weight: f64,
}

#[tauri::command]
pub async fn get_graph(state: State<'_, AppState>) -> Result<GraphResponse, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        let graph = build_graph_conn(&conn).map_err(|e| format!("failed to build graph: {e}"))?;
        Ok(GraphResponse {
            nodes: graph
                .nodes
                .into_iter()
                .map(|n| GraphNodeResponse {
                    id: n.id,
                    title: n.title,
                    node_type: n.node_type,
                    tags: n.tags,
                    importance: n.importance,
                })
                .collect(),
            edges: graph
                .edges
                .into_iter()
                .map(|e| GraphEdgeResponse {
                    source: e.source,
                    target: e.target,
                    relation: e.relation,
                    weight: e.weight,
                })
                .collect(),
        })
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}

#[tauri::command]
pub async fn get_neighbors(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<NeighborResponse>, String> {
    for id in &ids {
        if !validate_uuid(id) {
            return Err(format!("invalid node id: {id}"));
        }
    }
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        let neighbors = graph_neighbors_conn(&conn, &ids);
        Ok(neighbors
            .into_iter()
            .map(|(id, weight)| NeighborResponse { id, weight })
            .collect())
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}
