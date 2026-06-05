//! store/ — Node/Edge SQLite I/O (replaces file-based store)
//!
//! Re-exports all public items from focused submodules.

// ── Internal submodules ──────────────────────────────

mod decay;
mod dedup;
mod edge;
mod index;
mod node;
mod recall;
mod schema;
mod search;
pub mod types;
mod util;

#[cfg(test)]
mod tests;

// ── Re-exports: types ────────────────────────────────

pub use types::{Edge, IndexNode, Node, NodeFrontmatter, importance_for_type};

// ── Re-exports: util ─────────────────────────────────

pub use util::{
    atomic_write, db_path, graph_path, new_uuid, nodes_dir, now_iso, parse_iso_to_secs,
    validate_uuid,
};

// ── Re-exports: schema ───────────────────────────────

#[cfg(test)]
pub(crate) use schema::init_schema;

// ── Re-exports: node ─────────────────────────────────

#[allow(unused_imports)] // re-exported for external crate consumers (graphos-desktop)
pub use node::{
    delete_node_file, delete_node_file_conn, list_node_ids, list_node_ids_conn, node_exists_conn,
    parse_node, read_all_nodes_conn, read_node, read_node_conn, read_nodes_conn,
    read_nodes_limited_conn, serialize_node, write_node,
};

pub use node::write_node_conn;

// ── Re-exports: edge ─────────────────────────────────

#[allow(unused_imports)] // re-exported for external crate consumers (graphos-desktop)
pub use edge::{
    append_edge, append_edge_conn, delete_edge_by_id, delete_edge_by_id_conn, read_edges,
    read_edges_conn, remove_edges_for_node, remove_edges_for_node_conn,
};

// ── Re-exports: index ────────────────────────────────

pub use index::{read_index, remove_from_index, upsert_index};

// ── Re-exports: dedup ────────────────────────────────

pub use dedup::{write_node_dedup, write_node_dedup_conn};

// ── Re-exports: decay ────────────────────────────────

pub use decay::{decay_importance, tag_stale_nodes, touch_nodes_conn};

// ── Re-exports: recall ───────────────────────────────

pub use recall::{smart_recall, smart_recall_conn};

// ── Re-exports: search ───────────────────────────────

pub use search::{query_nodes, query_nodes_conn, search_nodes, search_nodes_conn};

// ── Re-exports: async pool functions ──────────────────

#[allow(unused_imports)] // new async APIs — not yet consumed by callers
pub use decay::{decay_importance_pool, tag_stale_nodes_pool, touch_nodes_pool};
#[allow(unused_imports)]
pub use dedup::write_node_dedup_pool;
#[allow(unused_imports)]
pub use edge::{
    append_edge_pool, delete_edge_by_id_pool, read_edges_pool, remove_edges_for_node_pool,
};
#[allow(unused_imports)]
pub use node::{
    delete_node_pool, list_node_ids_pool, node_exists_pool, read_all_nodes_pool, read_node_pool,
    read_nodes_limited_pool, read_nodes_pool, write_node_pool,
};
#[allow(unused_imports)]
pub use recall::smart_recall_pool;
#[allow(unused_imports)]
pub use schema::init_schema_pool;
#[allow(unused_imports)]
pub use search::{query_nodes_pool, search_nodes_pool};

// ── DB connection ────────────────────────────────────

use rusqlite::Connection;
use std::fs;
use std::io;

/// Open the memory database, applying schema and auto-migration.
pub fn open_db() -> io::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path).map_err(io::Error::other)?;

    // WAL mode for better concurrency
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(io::Error::other)?;

    schema::init_schema(&conn)?;
    schema::auto_migrate_legacy(&conn);
    Ok(conn)
}
