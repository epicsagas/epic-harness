use crate::state::AppState;
use epic_harness::mem::graph::{rebuild_graph_json_conn, graph_neighbors_conn};
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
pub fn get_graph(state: State<'_, AppState>) -> Result<GraphResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let json_str = rebuild_graph_json_conn(&conn).map_err(|e| e.to_string())?;
    let graph: GraphResponse =
        serde_json::from_str(&json_str).map_err(|e| format!("Parse error: {e}"))?;
    Ok(graph)
}

#[tauri::command]
pub fn get_neighbors(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<NeighborResponse>, String> {
    for id in &ids {
        if !validate_uuid(id) {
            return Err(format!("invalid node id: {id}"));
        }
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let neighbors = graph_neighbors_conn(&conn, &ids);
    Ok(
        neighbors
            .into_iter()
            .map(|(id, weight)| NeighborResponse { id, weight })
            .collect(),
    )
}
