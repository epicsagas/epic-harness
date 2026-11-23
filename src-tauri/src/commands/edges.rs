use epic_harness::mem::store::{
    Edge, MAX_RELATION_CHARS, delete_edge_pool, now_iso, read_edges_pool, validate_uuid,
};
use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use tauri::State;

use crate::state::AppState;

/// Validate that a relation string contains only alphanumeric, underscore, or hyphen characters.
fn validate_relation(relation: &str) -> Result<(), String> {
    if relation.is_empty() {
        return Err("relation must not be empty".into());
    }
    if relation.chars().count() > MAX_RELATION_CHARS {
        return Err(format!(
            "relation exceeds max length of {MAX_RELATION_CHARS} characters"
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

/// Edge identity: the `(source, target, relation)` primary key.
#[derive(Serialize, Debug, PartialEq)]
pub struct EdgeKey {
    pub source: String,
    pub target: String,
    pub relation: String,
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
) -> Result<EdgeKey, String> {
    create_edge_in(&state.db, input).await
}

async fn create_edge_in(pool: &AnyPool, input: CreateEdgeInput) -> Result<EdgeKey, String> {
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
        id: String::new(),
        source: input.source,
        target: input.target,
        relation: input.relation,
        weight: input.weight.clamp(0.0, 100.0),
        ts: now_iso(),
    };
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
        "INSERT OR REPLACE INTO edges (source, target, label, created) VALUES ($1, $2, $3, $4)",
    )
    .bind(&edge.source)
    .bind(&edge.target)
    .bind(&edge.relation)
    .bind(&edge.ts)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("failed to create edge: {e}"))?;
    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;
    Ok(EdgeKey {
        source: edge.source,
        target: edge.target,
        relation: edge.relation,
    })
}

#[tauri::command]
pub async fn delete_edge(
    source: String,
    target: String,
    relation: String,
    state: State<'_, AppState>,
) -> Result<EdgeKey, String> {
    delete_edge_in(&state.db, source, target, relation).await
}

async fn delete_edge_in(
    pool: &AnyPool,
    source: String,
    target: String,
    relation: String,
) -> Result<EdgeKey, String> {
    if !validate_uuid(&source) {
        return Err("invalid source id".into());
    }
    if !validate_uuid(&target) {
        return Err("invalid target id".into());
    }
    // Accept every relation that can be stored (any characters), unlike create.
    if relation.is_empty() || relation.chars().count() > MAX_RELATION_CHARS {
        return Err(format!(
            "relation must be 1 to {MAX_RELATION_CHARS} characters"
        ));
    }
    let deleted = delete_edge_pool(pool, &source, &target, &relation)
        .await
        .map_err(|e| format!("failed to delete edge: {e}"))?;
    if deleted == 0 {
        return Err("edge not found".into());
    }
    Ok(EdgeKey {
        source,
        target,
        relation,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::any::AnyPoolOptions;

    const A: &str = "00000000-0000-4000-8000-00000000000a";
    const B: &str = "00000000-0000-4000-8000-00000000000b";

    async fn temp_pool() -> AnyPool {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        epic_harness::mem::store::init_schema_pool(&pool)
            .await
            .unwrap();
        for id in [A, B] {
            sqlx::query("INSERT INTO nodes (id) VALUES (?)")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }
        pool
    }

    fn input(relation: &str) -> CreateEdgeInput {
        CreateEdgeInput {
            source: A.into(),
            target: B.into(),
            relation: relation.into(),
            weight: 1.0,
        }
    }

    async fn edge_count(pool: &AnyPool) -> i64 {
        use sqlx::Row;
        sqlx::query("SELECT COUNT(*) FROM edges")
            .fetch_one(pool)
            .await
            .unwrap()
            .get::<i64, _>(0)
    }

    #[tokio::test]
    async fn create_then_delete_round_trips_by_key() {
        let pool = temp_pool().await;
        let key = create_edge_in(&pool, input("uses")).await.unwrap();
        create_edge_in(&pool, input("blocks")).await.unwrap();
        assert_eq!(
            key,
            EdgeKey {
                source: A.into(),
                target: B.into(),
                relation: "uses".into()
            }
        );

        let deleted = delete_edge_in(&pool, key.source, key.target, key.relation)
            .await
            .unwrap();
        assert_eq!(deleted.relation, "uses");
        assert_eq!(edge_count(&pool).await, 1, "the other label must survive");

        let again = delete_edge_in(&pool, A.into(), B.into(), "uses".into()).await;
        assert_eq!(again.unwrap_err(), "edge not found");
    }

    #[tokio::test]
    async fn create_caps_relation_at_the_deletable_length() {
        let pool = temp_pool().await;
        assert!(
            create_edge_in(&pool, input(&"x".repeat(MAX_RELATION_CHARS + 1)))
                .await
                .is_err()
        );
        let key = create_edge_in(&pool, input(&"x".repeat(MAX_RELATION_CHARS)))
            .await
            .unwrap();
        delete_edge_in(&pool, key.source, key.target, key.relation)
            .await
            .unwrap();
        assert_eq!(edge_count(&pool).await, 0);
    }

    #[tokio::test]
    async fn delete_accepts_any_stored_relation() {
        let pool = temp_pool().await;
        // create_edge rejects spaces, but HTTP and CLI can store them.
        let stored = Edge {
            id: String::new(),
            source: A.into(),
            target: B.into(),
            relation: "depends on".into(),
            weight: 1.0,
            ts: now_iso(),
        };
        epic_harness::mem::store::append_edge_pool(&pool, &stored)
            .await
            .unwrap();
        assert!(create_edge_in(&pool, input("depends on")).await.is_err());
        delete_edge_in(&pool, A.into(), B.into(), "depends on".into())
            .await
            .unwrap();
        assert_eq!(edge_count(&pool).await, 0);
    }

    #[tokio::test]
    async fn delete_rejects_invalid_input() {
        let pool = temp_pool().await;
        create_edge_in(&pool, input("uses")).await.unwrap();
        for (s, t, r) in [
            ("not-a-uuid", B, "uses"),
            (A, "bad/id", "uses"),
            (A, B, ""),
            (A, B, &"x".repeat(MAX_RELATION_CHARS + 1)),
        ] {
            assert!(
                delete_edge_in(&pool, s.into(), t.into(), r.into())
                    .await
                    .is_err()
            );
        }
        assert_eq!(edge_count(&pool).await, 1);
    }
}
