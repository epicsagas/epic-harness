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

/// Initialize the graph schema and indexes without rewriting user importance.
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

    // Existing importance values (including 0.5 and session overrides) are user data.
    // Type defaults are applied when a node is created or imported, never on open.

    // Migrate FTS schema if it lacks the 'id' and 'projects' columns added in v2.
    migrate_fts_schema(pool).await;

    // Convert a pre-label edges table (id/relation/weight/ts) in place.
    migrate_legacy_edges(pool).await;

    // Legacy file migration
    migrate_legacy_files(pool).await;

    Ok(())
}

const EDGES_LEGACY_SQL: &str =
    "SELECT COUNT(*) FROM pragma_table_info('edges') WHERE name = 'label'";

async fn edges_is_legacy(conn: &mut sqlx::AnyConnection) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(EDGES_LEGACY_SQL).fetch_one(conn).await?;
    Ok(n == 0)
}

/// Old memory.db files have `edges(id, source, target, relation, weight, ts)`; the
/// `CREATE TABLE IF NOT EXISTS` in `GRAPH_SCHEMA_SQL` never converts them.
/// Failures are logged to stderr and never fail `init_schema_pool`.
async fn migrate_legacy_edges(pool: &AnyPool) {
    if let Err(e) = try_migrate_legacy_edges(pool).await {
        eprintln!("[harness] memory.db: legacy edges migration failed: {e}");
    }
}

async fn try_migrate_legacy_edges(pool: &AnyPool) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;
    if !edges_is_legacy(&mut conn).await? {
        return Ok(());
    }

    // In-memory DBs have no file: migrate without a backup (the data is ephemeral).
    let path: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(&mut *conn)
            .await?;
    let backup = if path.is_empty() {
        None
    } else if has_edges_backup(&path) {
        // A persistently failing conversion must not write a full copy on every open.
        Some("existing backup kept".to_string())
    } else {
        match backup_db(&mut conn, &path).await {
            Ok(bak) => Some(bak),
            Err(e) => {
                eprintln!("[harness] memory.db: edges backup failed, not migrating: {e}");
                return Ok(());
            }
        }
    };

    // BEGIN IMMEDIATE takes the write lock up front, so the re-check below cannot race.
    // The Transaction rolls back on drop, so a failed COMMIT never leaks an open
    // transaction back into the pool.
    let mut tx = sqlx::Connection::begin_with(&mut *conn, "BEGIN IMMEDIATE").await?;
    match convert_legacy_edges(&mut tx).await {
        Ok(Some(n)) => {
            tx.commit().await?;
            eprintln!(
                "[harness] memory.db: converted {n} legacy edges (backup: {})",
                backup.as_deref().unwrap_or("none, in-memory")
            );
            Ok(())
        }
        other => {
            let _ = tx.rollback().await;
            other.map(|_| ())
        }
    }
}

/// True when a completed `<db>.pre-edges-migration-*.bak` already sits next to the DB.
fn has_edges_backup(db: &str) -> bool {
    let db = std::path::Path::new(db);
    let (Some(dir), Some(name)) = (db.parent(), db.file_name()) else {
        return false;
    };
    let prefix = format!("{}.pre-edges-migration-", name.to_string_lossy());
    std::fs::read_dir(dir).is_ok_and(|rd| {
        rd.flatten().any(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with(&prefix) && n.ends_with(".bak")
        })
    })
}

/// Copy the DB with `VACUUM INTO` to `<db>.pre-edges-migration-<secs>-<pid>.bak` (mode 0600
/// like memory.db). Written to a `.tmp` name and renamed, so a `.bak` is always complete.
async fn backup_db(conn: &mut sqlx::AnyConnection, db: &str) -> io::Result<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let bak = format!("{db}.pre-edges-migration-{secs}-{}.bak", std::process::id());
    let tmp = format!("{bak}.tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&tmp)?;
    let r = sqlx::query("VACUUM INTO ?")
        .bind(&tmp)
        .execute(conn)
        .await
        .map_err(io::Error::other)
        .and_then(|_| std::fs::rename(&tmp, &bak));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map(|_| bak)
}

/// Runs inside the write transaction. `Ok(None)` means another process already converted.
async fn convert_legacy_edges(conn: &mut sqlx::AnyConnection) -> Result<Option<u64>, sqlx::Error> {
    if !edges_is_legacy(conn).await? {
        return Ok(None);
    }
    for stmt in [
        "ALTER TABLE edges RENAME TO edges_legacy",
        "CREATE TABLE edges (
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            label TEXT NOT NULL DEFAULT '',
            created TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (source, target, label)
        )",
    ] {
        sqlx::query(stmt).execute(&mut *conn).await?;
    }
    let n = sqlx::query(
        "INSERT OR IGNORE INTO edges (source, target, label, created) \
         SELECT source, target, COALESCE(NULLIF(relation, ''), 'related'), COALESCE(ts, '') \
         FROM edges_legacy ORDER BY ts, rowid",
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    for stmt in [
        "DROP TABLE edges_legacy",
        "CREATE INDEX idx_edges_source ON edges(source)",
        "CREATE INDEX idx_edges_target ON edges(target)",
    ] {
        sqlx::query(stmt).execute(&mut *conn).await?;
    }
    Ok(Some(n))
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
            let gn = node_to_graph(node);
            let _ = upsert_graph_node(pool, &gn).await;
            count += 1;
        }
    }

    // Import edges.jsonl
    let edges_path = harness_dir.join("memory").join("edges.jsonl");
    if edges_path.exists()
        && let Ok(content) = std::fs::read_to_string(&edges_path)
    {
        for line in content.lines() {
            if let Ok(edge) = serde_json::from_str::<super::types::Edge>(line) {
                let ge = super::types::edge_to_graph(&edge);
                let _ = append_graph_edge(pool, &ge).await;
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
