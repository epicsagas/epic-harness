#[path = "../src-tauri/src/commands/search_data.rs"]
mod search_data;

use epic_harness::mem::store::{Node, NodeFrontmatter};
use serde_json::json;

async fn pool() -> sqlx::AnyPool {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    epic_harness::mem::store::init_schema_pool(&pool)
        .await
        .unwrap();
    pool
}

async fn seed(pool: &sqlx::AnyPool, node_type: &str) {
    epic_harness::mem::store::write_node_pool(
        pool,
        &Node {
            frontmatter: NodeFrontmatter {
                id: uuid::Uuid::new_v4().to_string(),
                node_type: node_type.into(),
                title: "Zebrafixture needle".into(),
                tags: vec!["fixture".into()],
                projects: vec!["project-a".into()],
                agents: vec!["synthetic-agent".into()],
                created: "2026-11-09T00:00:00Z".into(),
                updated: "2026-11-09T00:00:00Z".into(),
                importance: 0.8,
                ..Default::default()
            },
            body: "Zebrafixture body".into(),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn issue_240_native_search_returns_renderable_memory_rows() {
    let pool = pool().await;
    seed(&pool, "concept").await;
    let rows = search_data::search_nodes(&pool, "Zebrafixture", None)
        .await
        .unwrap();
    let value = serde_json::to_value(rows).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 1);
    assert_eq!(value[0]["tags"], json!(["fixture"]));
    assert_eq!(value[0]["projects"], json!(["project-a"]));
    assert_eq!(value[0]["agents"], json!(["synthetic-agent"]));
    assert_eq!(value[0]["importance"], 0.8);
    assert!(value[0]["updated"].is_string());
    assert!(value[0]["snippet"].is_string());
}
