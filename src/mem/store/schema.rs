//! schema.rs — Schema initialization for memory.db (nodes, FTS5, edges)
//!
//! Creates the graph schema (nodes table, FTS5 virtual table, edges table,
//! indexes) plus epic-harness-specific indexes and data migrations.
//! Called by `pool::memory_pool()` on first pool creation.

use sqlx::AnyPool;
use std::io;

/// SQL DDL for the graph schema — nodes table, FTS5 virtual table, edges table, and indexes.
const GRAPH_SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '',
    projects TEXT NOT NULL DEFAULT '',
    agents TEXT NOT NULL DEFAULT '',
    created TEXT NOT NULL DEFAULT '',
    updated TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    importance REAL NOT NULL DEFAULT 0.5,
    access_count INTEGER NOT NULL DEFAULT 0,
    accessed_at TEXT NOT NULL DEFAULT ''
);

CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    id, title, body, tags, projects,
    content=nodes, content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO nodes_fts(rowid, id, title, body, tags, projects)
    VALUES (new.rowid, new.id, new.title, new.body, new.tags, new.projects);
END;

CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, id, title, body, tags, projects)
    VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags, old.projects);
END;

CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, id, title, body, tags, projects)
    VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags, old.projects);
    INSERT INTO nodes_fts(rowid, id, title, body, tags, projects)
    VALUES (new.rowid, new.id, new.title, new.body, new.tags, new.projects);
END;

CREATE TABLE IF NOT EXISTS edges (
    source TEXT NOT NULL,
    target TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    created TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (source, target, label)
);

CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
";

/// SQL for epic-harness-specific indexes.
const HARNESS_INDEXES_SQL: &str = "
CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);
CREATE INDEX IF NOT EXISTS idx_nodes_imp_upd ON nodes(importance DESC, updated DESC);
";

/// SQL to backfill importance for existing nodes based on type.
const IMPORTANCE_BACKFILL_SQL: &str = "
UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
UPDATE nodes SET importance = 0.05 WHERE importance = 0.5 AND type = 'session';
UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;
";

/// Initialize the graph schema + indexes + importance backfill.
///
/// Called by `pool::memory_pool()` after pool creation. Idempotent (IF NOT EXISTS).
pub async fn init_schema_pool(pool: &AnyPool) -> io::Result<()> {
    // Core graph schema
    sqlx::query(GRAPH_SCHEMA_SQL)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;

    // Harness-specific indexes
    sqlx::query(HARNESS_INDEXES_SQL)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;

    // Importance backfill (ignore errors — best-effort)
    let _ = sqlx::query(IMPORTANCE_BACKFILL_SQL).execute(pool).await;

    // Migrate FTS schema if it lacks the 'id' and 'projects' columns added in v2.
    migrate_fts_schema(pool).await;

    // Legacy file migration
    migrate_legacy_files(pool).await;

    Ok(())
}

/// Check if nodes_fts has the v2 columns (id, projects). If not, drop and recreate with triggers.
async fn migrate_fts_schema(pool: &AnyPool) {
    // Try to query 'id' column from FTS; if it fails the old schema is in place.
    let needs_migration = sqlx::query("SELECT id FROM nodes_fts LIMIT 1")
        .fetch_optional(pool)
        .await
        .is_err();

    if !needs_migration {
        return;
    }

    // Drop old triggers, old FTS table, recreate with id+projects columns, restore triggers.
    let stmts = [
        "DROP TRIGGER IF EXISTS nodes_ai",
        "DROP TRIGGER IF EXISTS nodes_ad",
        "DROP TRIGGER IF EXISTS nodes_au",
        "DROP TABLE IF EXISTS nodes_fts",
        "CREATE VIRTUAL TABLE nodes_fts USING fts5(id, title, body, tags, projects, content=nodes, content_rowid=rowid)",
        "CREATE TRIGGER nodes_ai AFTER INSERT ON nodes BEGIN \
            INSERT INTO nodes_fts(rowid, id, title, body, tags, projects) \
            VALUES (new.rowid, new.id, new.title, new.body, new.tags, new.projects); \
         END",
        "CREATE TRIGGER nodes_ad AFTER DELETE ON nodes BEGIN \
            INSERT INTO nodes_fts(nodes_fts, rowid, id, title, body, tags, projects) \
            VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags, old.projects); \
         END",
        "CREATE TRIGGER nodes_au AFTER UPDATE ON nodes BEGIN \
            INSERT INTO nodes_fts(nodes_fts, rowid, id, title, body, tags, projects) \
            VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags, old.projects); \
            INSERT INTO nodes_fts(rowid, id, title, body, tags, projects) \
            VALUES (new.rowid, new.id, new.title, new.body, new.tags, new.projects); \
         END",
        "INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')",
    ];

    for stmt in &stmts {
        let _ = sqlx::query(*stmt).execute(pool).await;
    }
}

/// Import legacy `nodes/*.md` + `edges.jsonl` if not yet migrated.
async fn migrate_legacy_files(pool: &AnyPool) {
    use super::util::db_path;

    let harness_dir = db_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"))
        .to_path_buf();
    let legacy_nodes_dir = harness_dir.join("memory").join("nodes");
    let marker = harness_dir.join("memory").join(".migrated");

    if marker.exists() || !legacy_nodes_dir.exists() {
        return;
    }

    use super::node::parse_node;
    use super::types::importance_for_type;
    use super::types::node_to_graph;

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
            let mut gn = node_to_graph(node);
            gn.importance = importance_for_type(&node_type);
            let _ = upsert_graph_node(pool, &gn).await;
            count += 1;
        }
    }

    // Import edges.jsonl
    let edges_path = harness_dir.join("memory").join("edges.jsonl");
    if edges_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&edges_path) {
            for line in content.lines() {
                if let Ok(edge) = serde_json::from_str::<super::types::Edge>(line) {
                    let ge = super::types::edge_to_graph(&edge);
                    let _ = append_graph_edge(pool, &ge).await;
                }
            }
        }
    }

    let _ = std::fs::write(&marker, format!("migrated {count} nodes\n"));
}

/// Upsert a GraphNode into the nodes table.
///
/// FTS sync is handled by triggers (nodes_ai / nodes_ad / nodes_au). INSERT OR REPLACE fires
/// DELETE+INSERT internally, so the triggers keep nodes_fts consistent automatically.
pub(crate) async fn upsert_graph_node(
    pool: &AnyPool,
    gn: &super::types::GraphNode,
) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(&gn.id)
        .bind(&gn.node_type)
        .bind(&gn.title)
        .bind(&gn.tags)
        .bind(&gn.projects)
        .bind(&gn.agents)
        .bind(&gn.created)
        .bind(&gn.updated)
        .bind(&gn.body)
        .bind(gn.importance)
        .bind(gn.access_count)
        .bind(&gn.accessed_at)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;

    Ok(())
}

/// Append a GraphEdge into the edges table.
pub(crate) async fn append_graph_edge(
    pool: &AnyPool,
    ge: &super::types::GraphEdge,
) -> io::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO edges (source, target, label, created) VALUES (?, ?, ?, ?)",
    )
    .bind(&ge.source)
    .bind(&ge.target)
    .bind(&ge.label)
    .bind(&ge.created)
    .execute(pool)
    .await
    .map_err(io::Error::other)?;
    Ok(())
}
