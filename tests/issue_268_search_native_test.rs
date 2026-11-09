#[path = "../src-tauri/src/commands/search_data.rs"]
mod search_data;

use epic_harness::mem::store::{Node, NodeFrontmatter};

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
async fn issue_268_native_type_filter_precedes_search_limit() {
    let pool = pool().await;
    for _ in 0..25 {
        seed(&pool, "concept").await;
    }
    seed(&pool, "decision").await;
    let rows = search_data::search_nodes(&pool, "Zebrafixture", Some(1), Some("decision"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].node_type, "decision");
    let rows = search_data::search_nodes(&pool, "Zebrafixture", None, Some("resolution"))
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn issue_268_native_type_filter_validates_supported_values() {
    let pool = pool().await;
    assert!(
        search_data::search_nodes(&pool, "Zebrafixture", None, Some("bogus"))
            .await
            .is_err()
    );
    for node_type in ["instinct", "psychographic"] {
        seed(&pool, node_type).await;
        let rows = search_data::search_nodes(&pool, "Zebrafixture", None, Some(node_type))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].node_type, node_type);
    }
}
