use epic_harness::mem::{graph::build_graph_pool_virtual, store::*};

#[tokio::test]
async fn issue_227_legacy_writer_columns_remain_compatible() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    init_schema_pool(&pool).await.unwrap();
    for target in ["first", "second"] {
        sqlx::query("INSERT INTO edges (source,target,label,created) VALUES ('legacy-writer',?,'related','2024-01-01T00:00:00Z')").bind(target).execute(&pool).await.unwrap();
    }
    init_schema_pool(&pool).await.unwrap();
    let edges = read_edges_pool(&pool, 10).await.unwrap();
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().all(|edge| validate_uuid(&edge.id)));
    assert_ne!(edges[0].id, edges[1].id);
}

#[tokio::test]
async fn issue_227_edge_metadata_survives_migration_and_exact_deletion() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE edges (source TEXT NOT NULL, target TEXT NOT NULL, label TEXT NOT NULL DEFAULT '', created TEXT NOT NULL DEFAULT '', PRIMARY KEY(source,target,label))").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO edges VALUES ('legacy-source','legacy-target','related','2024-01-01T00:00:00Z')").execute(&pool).await.unwrap();
    init_schema_pool(&pool).await.unwrap();
    let old = read_edges_pool(&pool, 10).await.unwrap();
    assert_eq!(old.len(), 1);
    assert!(
        validate_uuid(&old[0].id),
        "legacy edges receive persistent UUIDs"
    );
    let legacy_id = old[0].id.clone();
    init_schema_pool(&pool).await.unwrap();
    assert_eq!(read_edges_pool(&pool, 10).await.unwrap()[0].id, legacy_id);

    let edge = Edge {
        id: new_uuid(),
        source: "source-node".into(),
        target: "target-node".into(),
        relation: "related".into(),
        weight: 0.23,
        ts: now_iso(),
    };
    append_edge_pool(&pool, &edge).await.unwrap();
    let sibling = Edge {
        id: new_uuid(),
        relation: "depends".into(),
        weight: 0.7,
        ..edge.clone()
    };
    append_edge_pool(&pool, &sibling).await.unwrap();
    let persisted = read_edges_pool(&pool, 10).await.unwrap();
    assert_eq!(
        persisted.iter().find(|e| e.id == edge.id).unwrap().weight,
        0.23
    );
    delete_edge_by_id_pool(&pool, &edge.source).await.unwrap();
    assert_eq!(
        read_edges_pool(&pool, 10).await.unwrap().len(),
        3,
        "node IDs must not act as edge IDs"
    );
    delete_edge_by_id_pool(&pool, &edge.id).await.unwrap();
    let remaining = read_edges_pool(&pool, 10).await.unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().any(|e| e.id == sibling.id));
    let graph = build_graph_pool_virtual(&pool, false).await.unwrap();
    assert_eq!(
        graph
            .edges
            .iter()
            .find(|e| e.relation == "depends")
            .unwrap()
            .weight,
        0.7
    );
}
