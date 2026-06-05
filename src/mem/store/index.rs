//! index.rs — Index (built from DB) operations

use super::node::{delete_node_file, read_all_nodes_pool, write_node};
use super::types::{Index, IndexNode};
use super::util::split_csv;

#[allow(dead_code)]
pub fn read_index() -> Index {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        read_all_nodes_pool(&pool).await
    })
    .map(|nodes| {
        let mut by_tag: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut by_type: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut by_project: std::collections::HashMap<String, Vec<String>> = Default::default();

        let index_nodes: Vec<IndexNode> = nodes
            .iter()
            .map(|n| {
                let fm = &n.frontmatter;
                for tag in &fm.tags {
                    by_tag.entry(tag.clone()).or_default().push(fm.id.clone());
                }
                by_type
                    .entry(fm.node_type.clone())
                    .or_default()
                    .push(fm.id.clone());
                for proj in &fm.projects {
                    by_project
                        .entry(proj.clone())
                        .or_default()
                        .push(fm.id.clone());
                }
                IndexNode {
                    id: fm.id.clone(),
                    title: fm.title.clone(),
                    node_type: fm.node_type.clone(),
                    tags: split_csv(&fm.tags.join(",")),
                    projects: split_csv(&fm.projects.join(",")),
                    updated: fm.updated.clone(),
                }
            })
            .collect();

        Index {
            nodes: index_nodes,
            by_tag,
            by_type,
            by_project,
        }
    })
    .unwrap_or_default()
}

/// Upsert a node into the DB (same as write_node — the index IS the DB).
pub fn upsert_index(node: &super::types::Node) -> std::io::Result<()> {
    write_node(node)
}

/// Remove a node from the index (same as delete_node_file).
pub fn remove_from_index(node_id: &str) -> std::io::Result<()> {
    delete_node_file(node_id)
}
