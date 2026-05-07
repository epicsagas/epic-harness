//! schema.rs — Database schema initialization and legacy migration

use rusqlite::Connection;
use std::fs;
use std::io;

use super::types::Edge;
use super::util::{db_path, join_csv};

/// Apply the full schema (tables, indexes, triggers) to an open connection.
/// Exposed for tests that open an in-memory DB and need the same schema.
pub(crate) fn init_schema(conn: &Connection) -> io::Result<()> {
    // WAL auto-checkpoint for better concurrency
    let _ = conn.execute_batch("PRAGMA wal_autocheckpoint=100;");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id           TEXT PRIMARY KEY,
            type         TEXT NOT NULL,
            title        TEXT NOT NULL,
            tags         TEXT NOT NULL DEFAULT '',
            projects     TEXT NOT NULL DEFAULT '',
            agents       TEXT NOT NULL DEFAULT '',
            created      TEXT NOT NULL,
            updated      TEXT NOT NULL,
            body         TEXT NOT NULL DEFAULT '',
            importance   REAL NOT NULL DEFAULT 0.5,
            access_count INTEGER NOT NULL DEFAULT 0,
            accessed_at  TEXT NOT NULL DEFAULT ''
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts
            USING fts5(title, body, tags, content=nodes, content_rowid=rowid, tokenize='trigram');

        CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, title, body, tags)
            VALUES (new.rowid, new.title, new.body, new.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title, body, tags)
            VALUES('delete', old.rowid, old.title, old.body, old.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title, body, tags)
            VALUES('delete', old.rowid, old.title, old.body, old.tags);
            INSERT INTO nodes_fts(rowid, title, body, tags)
            VALUES (new.rowid, new.title, new.body, new.tags);
        END;

        -- Schema migration: add importance/access columns if missing
        -- ALTER TABLE IF NOT EXISTS is not supported, so we use a trick:
        -- these will silently fail if columns already exist.
        ",
    )
    .map_err(io::Error::other)?;

    // Migrate existing DBs: add new columns (ignore errors if already present)
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN importance REAL NOT NULL DEFAULT 0.5");
    let _ =
        conn.execute_batch("ALTER TABLE nodes ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN accessed_at TEXT NOT NULL DEFAULT ''");

    // Backfill importance for existing nodes based on type
    let _ = conn.execute_batch(
        "UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
         UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
         UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
         UPDATE nodes SET importance = 0.05 WHERE importance = 0.5 AND type = 'session';",
    );

    // Downgrade session importance from 0.2 to 0.05 (for existing DBs where backfill already ran)
    let _ = conn.execute_batch(
        "UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;",
    );

    // Schema version tracking
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT OR IGNORE INTO _meta (key, value) VALUES ('schema_version', '2');",
    );

    // Migrate FTS to trigram tokenizer if needed (schema_version < 2)
    let needs_fts_migrate: bool = conn
        .query_row(
            "SELECT value FROM _meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_none_or(|v| v != "2");

    if needs_fts_migrate {
        let _ = conn.execute_batch(
            "DROP TABLE IF EXISTS nodes_fts;
             CREATE VIRTUAL TABLE nodes_fts
                 USING fts5(title, body, tags, content=nodes, content_rowid=rowid, tokenize='trigram');
             INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');
             UPDATE _meta SET value = '2' WHERE key = 'schema_version';"
        );
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS edges (
            id       TEXT PRIMARY KEY,
            source   TEXT NOT NULL,
            target   TEXT NOT NULL,
            relation TEXT NOT NULL DEFAULT 'related',
            weight   REAL NOT NULL DEFAULT 1.0,
            ts       TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_edges_source  ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target  ON edges(target);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_src_tgt_rel ON edges(source, target, relation);
        CREATE INDEX IF NOT EXISTS idx_nodes_type    ON nodes(type);
        CREATE INDEX IF NOT EXISTS idx_nodes_updated ON nodes(updated DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_title_updated ON nodes(title, updated DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);
        ",
    )
    .map_err(io::Error::other)?;

    Ok(())
}

/// Silently import any legacy `nodes/*.md` + `edges.jsonl` into the DB.
/// Runs only once — leaves a `.migrated` marker in the old nodes dir.
pub(crate) fn auto_migrate_legacy(conn: &Connection) {
    use super::node::parse_node;
    use super::types::importance_for_type;

    let harness_dir = db_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"))
        .to_path_buf();
    let legacy_nodes_dir = harness_dir.join("memory").join("nodes");
    let marker = harness_dir.join("memory").join(".migrated");

    // Already migrated or no legacy data
    if marker.exists() || !legacy_nodes_dir.exists() {
        return;
    }

    // Import nodes/*.md
    let mut count = 0usize;
    if let Ok(entries) = fs::read_dir(&legacy_nodes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(node) = parse_node(&content) else {
                continue;
            };
            let fm = &node.frontmatter;
            let imp = importance_for_type(&fm.node_type);
            let _ = conn.execute(
                "INSERT OR IGNORE INTO nodes
                 (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,'')",
                rusqlite::params![
                    fm.id, fm.node_type, fm.title,
                    join_csv(&fm.tags), join_csv(&fm.projects), join_csv(&fm.agents),
                    fm.created, fm.updated, node.body, imp,
                ],
            );
            count += 1;
        }
    }

    // Import edges.jsonl
    let edges_path = harness_dir.join("memory").join("edges.jsonl");
    if edges_path.exists()
        && let Ok(content) = fs::read_to_string(&edges_path)
    {
        for line in content.lines() {
            if let Ok(edge) = serde_json::from_str::<Edge>(line) {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO edges
                     (id, source, target, relation, weight, ts)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                    rusqlite::params![
                        edge.id,
                        edge.source,
                        edge.target,
                        edge.relation,
                        edge.weight,
                        edge.ts,
                    ],
                );
            }
        }
    }

    // Write marker so we never run again
    let _ = fs::write(&marker, format!("migrated {} nodes\n", count));
}
