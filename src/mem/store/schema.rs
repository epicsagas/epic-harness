//! schema.rs — Database schema initialization and legacy migration

use sqlx::AnyPool;
use std::fs;
use std::io;

use super::types::Edge;
use super::util::{db_path, join_csv};
use crate::store::pool::DbType;

/// Async version of schema initialization using a sqlx AnyPool.
pub async fn init_schema_pool(pool: &AnyPool) -> io::Result<()> {
    let db_type = crate::store::pool::memory_db_type();

    match db_type {
        DbType::Sqlite => init_sqlite_schema(pool).await,
        DbType::Postgres => init_postgres_schema(pool).await,
        DbType::Mysql => init_sqlite_schema(pool).await, // MySQL deferred
    }
}

/// ── SQLite schema (nodes, FTS5 trigram, edges) ──────
async fn init_sqlite_schema(pool: &AnyPool) -> io::Result<()> {
    sqlx::raw_sql("PRAGMA wal_autocheckpoint=100;")
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;

    sqlx::raw_sql(
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
        END;",
    )
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    // Migrate existing DBs: add new columns (ignore errors if already present)
    let _ = sqlx::raw_sql("ALTER TABLE nodes ADD COLUMN importance REAL NOT NULL DEFAULT 0.5")
        .execute(pool)
        .await;
    let _ = sqlx::raw_sql("ALTER TABLE nodes ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::raw_sql("ALTER TABLE nodes ADD COLUMN accessed_at TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;

    // Backfill importance for existing nodes based on type
    let _ = sqlx::raw_sql(
        "UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
         UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
         UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
         UPDATE nodes SET importance = 0.05 WHERE importance = 0.5 AND type = 'session';",
    )
    .execute(pool)
    .await;

    // Downgrade session importance from 0.2 to 0.05
    let _ = sqlx::raw_sql(
        "UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;",
    )
    .execute(pool)
    .await;

    // Schema version tracking
    let _ = sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT OR IGNORE INTO _meta (key, value) VALUES ('schema_version', '2');",
    )
    .execute(pool)
    .await;

    // Migrate FTS to trigram tokenizer if needed (schema_version < 2)
    let needs_fts_migrate: bool =
        sqlx::query_scalar::<_, String>("SELECT value FROM _meta WHERE key = 'schema_version'")
            .fetch_optional(pool)
            .await
            .map_err(crate::store::sqlx_err)?
            .is_none_or(|v| v != "2");

    if needs_fts_migrate {
        let _ = sqlx::raw_sql(
            "DROP TABLE IF EXISTS nodes_fts;
             CREATE VIRTUAL TABLE nodes_fts
                 USING fts5(title, body, tags, content=nodes, content_rowid=rowid, tokenize='trigram');
             INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');
             UPDATE _meta SET value = '2' WHERE key = 'schema_version';",
        )
        .execute(pool)
        .await;
    }

    sqlx::raw_sql(
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
        CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);",
    )
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    Ok(())
}

/// ── PostgreSQL schema (nodes, tsvector+GIN, edges) ──
async fn init_postgres_schema(pool: &AnyPool) -> io::Result<()> {
    // Nodes table with generated tsvector column for full-text search.
    sqlx::raw_sql(
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
            importance   DOUBLE PRECISION NOT NULL DEFAULT 0.5,
            access_count INTEGER NOT NULL DEFAULT 0,
            accessed_at  TEXT NOT NULL DEFAULT '',
            search_vector TSVECTOR GENERATED ALWAYS AS (
                setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
                setweight(to_tsvector('english', coalesce(body, '')), 'B') ||
                setweight(to_tsvector('english', coalesce(tags, '')), 'C')
            ) STORED
        );

        CREATE INDEX IF NOT EXISTS idx_nodes_search ON nodes USING GIN (search_vector);
        CREATE INDEX IF NOT EXISTS idx_nodes_type    ON nodes(type);
        CREATE INDEX IF NOT EXISTS idx_nodes_updated ON nodes(updated DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_title_updated ON nodes(title, updated DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_accessed ON nodes(accessed_at DESC);",
    )
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    // Backfill importance for existing nodes based on type
    let _ = sqlx::raw_sql(
        "UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
         UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
         UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
         UPDATE nodes SET importance = 0.05 WHERE importance = 0.5 AND type = 'session';",
    )
    .execute(pool)
    .await;

    let _ = sqlx::raw_sql(
        "UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;",
    )
    .execute(pool)
    .await;

    // Schema version tracking
    let _ = sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO _meta (key, value) VALUES ('schema_version', '2')
         ON CONFLICT (key) DO NOTHING;",
    )
    .execute(pool)
    .await;

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS edges (
            id       TEXT PRIMARY KEY,
            source   TEXT NOT NULL,
            target   TEXT NOT NULL,
            relation TEXT NOT NULL DEFAULT 'related',
            weight   DOUBLE PRECISION NOT NULL DEFAULT 1.0,
            ts       TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_edges_source  ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target  ON edges(target);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_src_tgt_rel ON edges(source, target, relation);",
    )
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;

    Ok(())
}

/// Silently import any legacy `nodes/*.md` + `edges.jsonl` into the pool.
/// Runs only once — leaves a `.migrated` marker in the old nodes dir.
/// Only runs for SQLite backends.
pub(crate) async fn auto_migrate_legacy(pool: &AnyPool) {
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
            let _ = sqlx::query(
                "INSERT INTO nodes
                 (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,0,'')
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&fm.id)
            .bind(&fm.node_type)
            .bind(&fm.title)
            .bind(join_csv(&fm.tags))
            .bind(join_csv(&fm.projects))
            .bind(join_csv(&fm.agents))
            .bind(&fm.created)
            .bind(&fm.updated)
            .bind(&node.body)
            .bind(imp)
            .execute(pool)
            .await;
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
                let _ = sqlx::query(
                    "INSERT INTO edges
                     (id, source, target, relation, weight, ts)
                     VALUES ($1,$2,$3,$4,$5,$6)
                     ON CONFLICT (id) DO NOTHING",
                )
                .bind(&edge.id)
                .bind(&edge.source)
                .bind(&edge.target)
                .bind(&edge.relation)
                .bind(edge.weight)
                .bind(&edge.ts)
                .execute(pool)
                .await;
            }
        }
    }

    // Write marker so we never run again
    let _ = fs::write(&marker, format!("migrated {} nodes\n", count));
}
