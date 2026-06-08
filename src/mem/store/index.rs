//! index.rs — Index (built from DB) operations

use super::conn::memory_conn;
use super::node::{delete_node_file, write_node};
use super::types::{Index, IndexNode, graph_to_node};
use super::util::split_csv;

#[allow(dead_code)]
pub fn read_index() -> Index {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return Index::default(),
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return Index::default(),
    };
    let nodes: Vec<super::types::Node> =
        llm_kernel::graph::store::read_nodes_limited(&guard, 1_000_000)
            .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
            .unwrap_or_default();

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
}

/// Upsert a node into the DB (same as write_node — the index IS the DB).
pub fn upsert_index(node: &super::types::Node) -> std::io::Result<()> {
    write_node(node)
}

/// Remove a node from the index (same as delete_node_file).
pub fn remove_from_index(node_id: &str) -> std::io::Result<()> {
    delete_node_file(node_id)
}
