use crate::state::AppState;
use epic_harness::mem::store::{
    append_edge_conn, delete_edge_by_id_conn, new_uuid, now_iso, read_edges_conn,
    validate_uuid, Edge,
};

const MAX_RELATION_LEN: usize = 128;

/// Validate that a relation string contains only alphanumeric, underscore, or hyphen characters.
fn validate_relation(relation: &str) -> Result<(), String> {
    if relation.is_empty() {
        return Err("relation must not be empty".into());
    }
    if relation.len() > MAX_RELATION_LEN {
        return Err(format!("relation exceeds max length of {MAX_RELATION_LEN} characters"));
    }
    if !relation.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("relation must contain only alphanumeric, underscore, or hyphen characters".into());
    }
    Ok(())
}
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize)]
pub struct EdgeResponse {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
}

impl From<Edge> for EdgeResponse {
    fn from(e: Edge) -> Self {
        Self {
            id: e.id,
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateEdgeInput {
    pub source: String,
    pub target: String,
    #[serde(default = "default_relation")]
    pub relation: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
}

fn default_relation() -> String {
    "related".into()
}
fn default_weight() -> f64 {
    1.0
}

#[tauri::command]
pub fn create_edge(input: CreateEdgeInput, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&input.source) {
        return Err("invalid source id".into());
    }
    if !validate_uuid(&input.target) {
        return Err("invalid target id".into());
    }
    validate_relation(&input.relation)?;
    let edge = Edge {
        id: new_uuid(),
        source: input.source,
        target: input.target,
        relation: input.relation,
        weight: input.weight.clamp(0.0, 100.0),
        ts: now_iso(),
    };
    let id = edge.id.clone();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    append_edge_conn(&conn, &edge).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn delete_edge(id: String, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&id) {
        return Err("invalid edge id".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    delete_edge_by_id_conn(&conn, &id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn get_edges(limit: Option<usize>, state: State<'_, AppState>) -> Result<Vec<EdgeResponse>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let edges = read_edges_conn(&conn, limit.unwrap_or(2000)).map_err(|e| e.to_string())?;
    Ok(edges.into_iter().map(EdgeResponse::from).collect())
}
