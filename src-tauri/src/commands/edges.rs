use crate::state::AppState;
use epic_harness::mem::store::{
    Edge, append_edge_conn, delete_edge_by_id_conn, new_uuid, node_exists_conn, now_iso,
    read_edges_conn, validate_uuid,
};
use serde::{Deserialize, Serialize};
use tauri::State;

const MAX_RELATION_LEN: usize = 128;

/// Validate that a relation string contains only alphanumeric, underscore, or hyphen characters.
fn validate_relation(relation: &str) -> Result<(), String> {
    if relation.is_empty() {
        return Err("relation must not be empty".into());
    }
    if relation.len() > MAX_RELATION_LEN {
        return Err(format!(
            "relation exceeds max length of {MAX_RELATION_LEN} characters"
        ));
    }
    if !relation
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "relation must contain only alphanumeric, underscore, or hyphen characters".into(),
        );
    }
    Ok(())
}

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
pub async fn create_edge(
    input: CreateEdgeInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if !validate_uuid(&input.source) {
        return Err("invalid source id".into());
    }
    if !validate_uuid(&input.target) {
        return Err("invalid target id".into());
    }
    if input.source == input.target {
        return Err("source and target must be different".to_string());
    }
    validate_relation(&input.relation)?;
    if input.weight.is_nan() {
        return Err("weight must be a valid number".into());
    }
    let edge = Edge {
        id: new_uuid(),
        source: input.source,
        target: input.target,
        relation: input.relation,
        weight: input.weight.clamp(0.0, 100.0),
        ts: now_iso(),
    };
    let id = edge.id.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        // Verify both endpoints exist before creating edge
        if !node_exists_conn(&conn, &edge.source) {
            return Err(format!("source node {} does not exist", edge.source));
        }
        if !node_exists_conn(&conn, &edge.target) {
            return Err(format!("target node {} does not exist", edge.target));
        }
        append_edge_conn(&conn, &edge).map_err(|e| format!("failed to create edge: {e}"))?;
        Ok(id)
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}

#[tauri::command]
pub async fn delete_edge(id: String, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&id) {
        return Err("invalid edge id".into());
    }
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        delete_edge_by_id_conn(&conn, &id).map_err(|e| format!("failed to delete edge: {e}"))?;
        Ok(id)
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}

#[tauri::command]
pub async fn get_edges(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<EdgeResponse>, String> {
    let limit = limit.unwrap_or(2000).min(5000);
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        let edges =
            read_edges_conn(&conn, limit).map_err(|e| format!("failed to read edges: {e}"))?;
        Ok(edges.into_iter().map(EdgeResponse::from).collect())
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}
