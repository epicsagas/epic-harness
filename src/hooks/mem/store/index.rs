//! index.rs — Index (built from DB) operations

use super::node::{delete_node_file, write_node};
use super::types::{Index, IndexNode};
use super::util::split_csv;

pub fn read_index() -> Index {
    let conn = match super::open_db() {
        Ok(c) => c,
        Err(_) => return Index::default(),
    };
    let mut stmt = match conn
        .prepare("SELECT id, type, title, tags, projects, updated FROM nodes ORDER BY updated DESC")
    {
        Ok(s) => s,
        Err(_) => return Index::default(),
    };

    let index_nodes: Vec<IndexNode> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let node_type: String = row.get(1)?;
            let title: String = row.get(2)?;
            let tags_str: String = row.get(3)?;
            let projects_str: String = row.get(4)?;
            let updated: String = row.get(5)?;
            Ok((id, node_type, title, tags_str, projects_str, updated))
        })
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(
                    |(id, node_type, title, tags_str, projects_str, updated)| IndexNode {
                        id,
                        title,
                        node_type,
                        tags: split_csv(&tags_str),
                        projects: split_csv(&projects_str),
                        updated,
                    },
                )
                .collect()
        })
        .unwrap_or_default();

    let mut by_tag: std::collections::HashMap<String, Vec<String>> = Default::default();
    let mut by_type: std::collections::HashMap<String, Vec<String>> = Default::default();
    let mut by_project: std::collections::HashMap<String, Vec<String>> = Default::default();

    for n in &index_nodes {
        for tag in &n.tags {
            by_tag.entry(tag.clone()).or_default().push(n.id.clone());
        }
        by_type
            .entry(n.node_type.clone())
            .or_default()
            .push(n.id.clone());
        for proj in &n.projects {
            by_project
                .entry(proj.clone())
                .or_default()
                .push(n.id.clone());
        }
    }

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
