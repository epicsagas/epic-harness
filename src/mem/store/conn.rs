//! conn.rs — MemoryConn: rusqlite-backed connection for memory.db via llm-kernel

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(test)]
use std::cell::RefCell;

use rusqlite::Connection;

use super::util::db_path;

/// Global singleton: (resolved_path, connection) pair.
/// Reinitializes when the resolved path changes (e.g., HARNESS_ROOT override in integration tests).
static MEMORY_CONN: Mutex<Option<(PathBuf, Arc<Mutex<Connection>>)>> = Mutex::new(None);

/// Per-thread override used by tests for isolation.
#[cfg(test)]
thread_local! {
    static TEST_CONN_OVERRIDE: RefCell<Option<Arc<Mutex<Connection>>>> = RefCell::new(None);
}

/// Returns the shared rusqlite connection for memory.db.
///
/// In tests, checks thread-local override first so each test thread
/// gets its own isolated in-memory DB.  Falls back to the global
/// singleton, reinitializing when the resolved path changes.
pub fn memory_conn() -> io::Result<Arc<Mutex<Connection>>> {
    // Fast path: thread-local override (tests only)
    #[cfg(test)]
    {
        let override_conn = TEST_CONN_OVERRIDE.with(|tc| tc.borrow().clone());
        if let Some(c) = override_conn {
            return Ok(c);
        }
    }

    let current_path = db_path();
    let mut slot = MEMORY_CONN
        .lock()
        .map_err(|e| io::Error::other(e.to_string()))?;
    if let Some((cached_path, c)) = slot.as_ref() {
        if cached_path == &current_path {
            return Ok(c.clone());
        }
    }
    let conn = open_and_init(&current_path).map(Arc::new)?;
    *slot = Some((current_path, conn.clone()));
    Ok(conn)
}

/// Open a connection and apply the graph schema + epic-harness migrations.
fn open_and_init(path: &PathBuf) -> io::Result<Mutex<Connection>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn =
        Connection::open(path).map_err(|e| io::Error::other(format!("open memory.db: {e}")))?;

    // WAL mode for concurrent reads
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| io::Error::other(format!("set WAL: {e}")))?;

    // llm-kernel graph schema (nodes, FTS5, edges, indexes)
    llm_kernel::graph::schema::init_graph_schema(&conn)
        .map_err(|e| io::Error::other(format!("init graph schema: {e}")))?;

    // epic-harness additional indexes for recall performance
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
         CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);
         CREATE INDEX IF NOT EXISTS idx_nodes_imp_upd ON nodes(importance DESC, updated DESC);",
    )
    .map_err(|e| io::Error::other(format!("create indexes: {e}")))?;

    // Backfill importance for existing nodes based on type
    let _ = conn.execute_batch(
        "UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
         UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
         UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
         UPDATE nodes SET importance = 0.05 WHERE importance = 0.5 AND type = 'session';
         UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;",
    );

    // Legacy file migration (nodes/*.md → DB)
    migrate_legacy_files(&conn);

    Ok(Mutex::new(conn))
}

/// Import legacy `nodes/*.md` + `edges.jsonl` if not yet migrated.
fn migrate_legacy_files(conn: &Connection) {
    use super::node::parse_node;
    use super::types::importance_for_type;

    let harness_dir = db_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"))
        .to_path_buf();
    let legacy_nodes_dir = harness_dir.join("memory").join("nodes");
    let marker = harness_dir.join("memory").join(".migrated");

    if marker.exists() || !legacy_nodes_dir.exists() {
        return;
    }

    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(&legacy_nodes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(node) = parse_node(&content) else {
                continue;
            };
            let node_type = node.frontmatter.node_type.clone();
            let mut gn = super::types::node_to_graph(node);
            gn.importance = importance_for_type(&node_type);
            let _ = llm_kernel::graph::store::upsert_node(conn, &gn);
            count += 1;
        }
    }

    // Import edges.jsonl
    let edges_path = harness_dir.join("memory").join("edges.jsonl");
    if edges_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&edges_path) {
            for line in content.lines() {
                if let Ok(edge) = serde_json::from_str::<super::types::Edge>(line) {
                    let ge = super::types::edge_to_graph(edge);
                    let _ = llm_kernel::graph::store::append_edge(conn, &ge);
                }
            }
        }
    }

    let _ = std::fs::write(&marker, format!("migrated {count} nodes\n"));
}

/// Create an in-memory connection for tests.
#[cfg(test)]
pub fn test_conn() -> Arc<Mutex<Connection>> {
    let conn = Connection::open_in_memory().unwrap();
    llm_kernel::graph::schema::init_graph_schema(&conn).unwrap();
    // Additional indexes
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
         CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);
         CREATE INDEX IF NOT EXISTS idx_nodes_imp_upd ON nodes(importance DESC, updated DESC);",
    )
    .unwrap();
    Arc::new(Mutex::new(conn))
}

/// Inject a test connection as a thread-local override.
/// Each test thread gets its own isolated in-memory DB.
#[cfg(test)]
pub fn set_test_conn(conn: Arc<Mutex<Connection>>) {
    TEST_CONN_OVERRIDE.with(|tc| *tc.borrow_mut() = Some(conn));
}

/// Reset the thread-local override so this thread falls back to the global singleton.
#[cfg(test)]
#[allow(dead_code)]
pub fn reset_test_conn() {
    TEST_CONN_OVERRIDE.with(|tc| *tc.borrow_mut() = None);
}
