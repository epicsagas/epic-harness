use epic_harness::mem::store::{
    Edge, delete_edge_by_id_pool, new_uuid, now_iso, read_edges_pool, validate_uuid,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

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
    let pool = state.db.clone();
    // Atomically verify endpoints exist and create edge in a single transaction
    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    let source_exists = sqlx::query("SELECT 1 FROM nodes WHERE id = $1 LIMIT 1")
        .bind(&edge.source)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("check source: {e}"))?
        .is_some();
    if !source_exists {
        return Err(format!("source node {} does not exist", edge.source));
    }
    let target_exists = sqlx::query("SELECT 1 FROM nodes WHERE id = $1 LIMIT 1")
        .bind(&edge.target)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("check target: {e}"))?
        .is_some();
    if !target_exists {
        return Err(format!("target node {} does not exist", edge.target));
    }
    sqlx::query(
        "INSERT INTO edges (id, source, target, relation, weight, ts) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&edge.id)
    .bind(&edge.source)
    .bind(&edge.target)
    .bind(&edge.relation)
    .bind(edge.weight)
    .bind(&edge.ts)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("failed to create edge: {e}"))?;
    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;
    Ok(id)
}

#[tauri::command]
pub async fn delete_edge(id: String, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&id) {
        return Err("invalid edge id".into());
    }
    let pool = state.db.clone();
    delete_edge_by_id_pool(&pool, &id)
        .await
        .map_err(|e| format!("failed to delete edge: {e}"))?;
    Ok(id)
}

#[tauri::command]
pub async fn get_edges(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<EdgeResponse>, String> {
    let limit = limit.unwrap_or(2000).min(5000) as i64;
    let pool = state.db.clone();
    let edges = read_edges_pool(&pool, limit)
        .await
        .map_err(|e| format!("failed to read edges: {e}"))?;
    Ok(edges.into_iter().map(EdgeResponse::from).collect())
}
