//! store/ — Node/Edge SQLite I/O (sqlx async pool)
//!
//! Re-exports all public items from focused submodules.

// ── Internal submodules ──────────────────────────────

pub(crate) mod conn;
mod decay;
mod dedup;
mod edge;
mod index;
mod node;
mod recall;
pub(crate) mod schema;
mod search;
pub mod types;
pub(crate) mod util;

#[cfg(test)]
mod tests;

// ── Re-exports: types ────────────────────────────────

pub use types::{Edge, IndexNode, Node, NodeFrontmatter, importance_for_type};

// ── Re-exports: util ─────────────────────────────────

#[allow(unused_imports)]
pub use util::{
    atomic_write, db_path, graph_path, new_uuid, nodes_dir, now_iso, parse_iso_to_secs,
    validate_uuid,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use util::parse_iso_to_secs as _parse_iso_to_secs;

// ── Re-exports: schema ───────────────────────────────

pub use schema::init_schema_pool;

// ── Re-exports: node ─────────────────────────────────

#[allow(unused_imports)]
pub use node::{
    delete_node_file, list_node_ids, parse_node, read_node, read_nodes_limited_pool,
    serialize_node, write_node, write_node_pool, read_node_pool, read_nodes_pool,
    read_all_nodes_pool, delete_node_pool, node_exists_pool, list_node_ids_pool,
};

// ── Re-exports: edge ──────────────────────────────────

#[allow(unused_imports)]
pub use edge::{
    append_edge, append_edge_pool, delete_edge_by_id, read_edges, read_edges_pool,
    remove_edges_for_node, remove_edges_for_node_pool, delete_edge_by_id_pool, read_edges_limit,
};

// ── Re-exports: index ─────────────────────────────────

#[allow(unused_imports)]
pub use index::{read_index, remove_from_index, upsert_index};

// ── Re-exports: dedup ─────────────────────────────────

pub use dedup::{write_node_dedup, write_node_dedup_pool};

// ── Re-exports: decay ─────────────────────────────────

pub use decay::{decay_importance, tag_stale_nodes, touch_nodes_pool};

// ── Re-exports: recall ────────────────────────────────

pub use recall::{smart_recall, smart_recall_pool};

// ── Re-exports: search ────────────────────────────────

pub use search::{query_nodes, search_nodes, query_nodes_pool, search_nodes_pool};
