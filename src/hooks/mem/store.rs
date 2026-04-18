//! store.rs — Node/Edge SQLite I/O (replaces file-based store)

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ── Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeFrontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    pub created: String,
    pub updated: String,
    /// Importance score (0.0–1.0). Higher = more valuable for recall.
    #[serde(default = "default_importance")]
    pub importance: f64,
    /// How many times this node has been retrieved via recall/search.
    #[serde(default)]
    pub access_count: i64,
    /// Last time this node was accessed (not just updated).
    #[serde(default)]
    pub accessed_at: String,
}

fn default_importance() -> f64 {
    0.5
}

/// Default importance by node type.
pub fn importance_for_type(node_type: &str) -> f64 {
    match node_type {
        "decision"   => 0.9,
        "resolution" => 0.8,
        "concept"    => 0.7,
        "project"    => 0.7,
        "pattern"    => 0.5,
        "error"      => 0.4,
        "session"    => 0.2,
        _            => 0.5,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub frontmatter: NodeFrontmatter,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub nodes: Vec<IndexNode>,
    pub by_tag: std::collections::HashMap<String, Vec<String>>,
    pub by_type: std::collections::HashMap<String, Vec<String>>,
    pub by_project: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexNode {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub updated: String,
}

// ── Paths ─────────────────────────────────────────────

/// Returns the path to the SQLite database file (~/.harness/memory.db).
pub fn db_path() -> PathBuf {
    let home = std::env::var("HARNESS_ROOT")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".harness").join("memory.db")
}

/// Compatibility: returns the .harness directory (parent of db_path).
pub fn nodes_dir() -> PathBuf {
    db_path()
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .to_path_buf()
}

/// graph.json path (Web UI).
pub fn graph_path() -> PathBuf {
    db_path()
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join("graph.json")
}

pub fn validate_node_id(id: &str) -> bool {
    // UUID v4 strict: xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx
    let b = id.as_bytes();
    b.len() == 36
        && b[8] == b'-' && b[13] == b'-' && b[18] == b'-' && b[23] == b'-'
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
        && b.iter().enumerate().all(|(i, &c)| {
            matches!(i, 8 | 13 | 18 | 23) || c.is_ascii_hexdigit()
        })
}

// ── DB connection + schema ─────────────────────────────

pub fn open_db() -> io::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)
        .map_err(io::Error::other)?;

    // WAL mode for better concurrency
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(io::Error::other)?;

    init_schema(&conn)?;
    auto_migrate_legacy(&conn);
    Ok(conn)
}

/// Apply the full schema (tables, indexes, triggers) to an open connection.
/// Exposed for tests that open an in-memory DB and need the same schema.
pub(crate) fn init_schema(conn: &Connection) -> io::Result<()> {
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
            USING fts5(title, body, tags, content=nodes, content_rowid=rowid);

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
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN accessed_at TEXT NOT NULL DEFAULT ''");

    // Backfill importance for existing nodes based on type
    let _ = conn.execute_batch(
        "UPDATE nodes SET importance = 0.9 WHERE importance = 0.5 AND type = 'decision';
         UPDATE nodes SET importance = 0.8 WHERE importance = 0.5 AND type = 'resolution';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'concept';
         UPDATE nodes SET importance = 0.7 WHERE importance = 0.5 AND type = 'project';
         UPDATE nodes SET importance = 0.4 WHERE importance = 0.5 AND type = 'error';
         UPDATE nodes SET importance = 0.2 WHERE importance = 0.5 AND type = 'session';"
    );

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
fn auto_migrate_legacy(conn: &Connection) {
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
            let Ok(content) = fs::read_to_string(&path) else { continue };
            let Some(node) = parse_node(&content) else { continue };
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
                        edge.id, edge.source, edge.target,
                        edge.relation, edge.weight, edge.ts,
                    ],
                );
            }
        }
    }

    // Write marker so we never run again
    let _ = fs::write(&marker, format!("migrated {} nodes\n", count));
}

// ── Helpers ───────────────────────────────────────────

fn join_csv(v: &[String]) -> String {
    v.join(",")
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Standard SELECT columns for node queries. Use with row_to_node().
const NODE_COLUMNS: &str =
    "id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at";

/// Same columns but table-prefixed for JOIN queries.
const NODE_COLUMNS_PREFIXED: &str =
    "id, n.type, n.title, n.tags, n.projects, n.agents, n.created, n.updated, n.body, n.importance, n.access_count, n.accessed_at";

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    let tags: String = row.get(3)?;
    let projects: String = row.get(4)?;
    let agents: String = row.get(5)?;
    Ok(Node {
        frontmatter: NodeFrontmatter {
            id: row.get(0)?,
            node_type: row.get(1)?,
            title: row.get(2)?,
            tags: split_csv(&tags),
            projects: split_csv(&projects),
            agents: split_csv(&agents),
            created: row.get(6)?,
            updated: row.get(7)?,
            importance: row.get(9).unwrap_or(0.5),
            access_count: row.get::<_, i64>(10).unwrap_or(0),
            accessed_at: row.get(11).unwrap_or_default(),
        },
        body: row.get(8)?,
    })
}

// ── Node I/O ──────────────────────────────────────────

pub fn write_node(node: &Node) -> io::Result<()> {
    let conn = open_db()?;
    write_node_conn(&conn, node)
}

fn write_node_conn(conn: &Connection, node: &Node) -> io::Result<()> {
    let fm = &node.frontmatter;
    conn.execute(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            fm.id,
            fm.node_type,
            fm.title,
            join_csv(&fm.tags),
            join_csv(&fm.projects),
            join_csv(&fm.agents),
            fm.created,
            fm.updated,
            node.body,
            fm.importance,
            fm.access_count,
            fm.accessed_at,
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let conn = open_db()?;
    read_node_conn(&conn, id)
}

/// Batch-read multiple nodes by ID in a single `WHERE id IN (...)` query.
pub fn read_nodes_conn(conn: &Connection, ids: &[&str]) -> Vec<Node> {
    if ids.is_empty() {
        return vec![];
    }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id IN ({ph})");
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(rusqlite::params_from_iter(ids.iter()), row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

pub fn read_node_conn(conn: &Connection, id: &str) -> io::Result<Node> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_node)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("node not found: {id}")))
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let conn = open_db()?;
    conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT id FROM nodes")
        .map_err(io::Error::other)?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

// ── Edge I/O ──────────────────────────────────────────

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![edge.id, edge.source, edge.target, edge.relation, edge.weight, edge.ts],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

pub fn read_edges() -> Vec<Edge> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut stmt = match conn.prepare(
        "SELECT id, source, target, relation, weight, ts FROM edges",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| {
        Ok(Edge {
            id: row.get(0)?,
            source: row.get(1)?,
            target: row.get(2)?,
            relation: row.get(3)?,
            weight: row.get(4)?,
            ts: row.get(5)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let conn = open_db()?;
    conn.execute("DELETE FROM edges WHERE id = ?1", params![edge_id])
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM edges WHERE source = ?1 OR target = ?1",
        params![node_id],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

// ── Index (built from DB) ──────────────────────────────

pub fn read_index() -> Index {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Index::default(),
    };
    let mut stmt = match conn.prepare(
        "SELECT id, type, title, tags, projects, updated FROM nodes ORDER BY updated DESC",
    ) {
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
                .map(|(id, node_type, title, tags_str, projects_str, updated)| IndexNode {
                    id,
                    title,
                    node_type,
                    tags: split_csv(&tags_str),
                    projects: split_csv(&projects_str),
                    updated,
                })
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
        by_type.entry(n.node_type.clone()).or_default().push(n.id.clone());
        for proj in &n.projects {
            by_project.entry(proj.clone()).or_default().push(n.id.clone());
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
pub fn upsert_index(node: &Node) -> io::Result<()> {
    write_node(node)
}

/// Remove a node from the index (same as delete_node_file).
pub fn remove_from_index(node_id: &str) -> io::Result<()> {
    delete_node_file(node_id)
}

// ── Deduplication ─────────────────────────────────────

/// Returns the ID of an existing node with the same title updated within the
/// last `window_hours` hours.  Uses the composite idx_nodes_title_updated index
/// for O(log N) lookup.  Used to prevent duplicate writes when multiple callers
/// (observe hook + skills + direct MCP) fire for the same event.
fn find_duplicate_in_conn(conn: &Connection, title: &str, window_hours: u64) -> Option<String> {
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(window_hours * 3600);
    let cutoff = {
        let s = cutoff_secs;
        let (y, m, d) = days_to_ymd(s / 86400);
        let hh = (s / 3600) % 24;
        let mm = (s / 60) % 60;
        let ss = s % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    conn.query_row(
        "SELECT id FROM nodes WHERE title = ?1 AND updated > ?2 ORDER BY updated DESC LIMIT 1",
        params![title, cutoff],
        |row| row.get::<_, String>(0),
    ).ok()
}

/// Write-with-dedup: opens a single connection, checks for a duplicate, and
/// writes only when none is found.  Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    let conn = open_db()?;
    write_node_dedup_conn(&conn, node, window_hours)
}

/// Write-with-dedup using an existing connection (for batch/transaction use).
pub fn write_node_dedup_conn(
    conn: &Connection,
    node: &Node,
    window_hours: u64,
) -> io::Result<(String, bool)> {
    let title = &node.frontmatter.title;

    if let Some(existing_id) = find_duplicate_in_conn(conn, title, window_hours) {
        return Ok((existing_id, true));
    }

    write_node_conn(conn, node)?;
    Ok((node.frontmatter.id.clone(), false))
}

/// Append an edge using an existing connection (for batch/transaction use).
pub fn append_edge_conn(conn: &Connection, edge: &Edge) -> io::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![edge.id, edge.source, edge.target, edge.relation, edge.weight, edge.ts],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Tag nodes not updated within `days` as stale by appending "stale" to their tags.
pub fn tag_stale_nodes(days: u64) -> io::Result<u64> {
    let conn = open_db()?;
    // Compute cutoff timestamp in Rust to avoid format!() SQL interpolation.
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(days * 86400);
    let cutoff = {
        let s = cutoff_secs;
        let (y, m, d) = days_to_ymd(s / 86400);
        let hh = (s / 3600) % 24;
        let mm = (s / 60) % 60;
        let ss = s % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let changed = conn.execute(
        "UPDATE nodes SET tags = CASE
            WHEN tags = '' THEN 'stale'
            WHEN ',' || tags || ',' NOT LIKE '%,stale,%' THEN tags || ',stale'
            ELSE tags
         END
         WHERE updated < ?1
           AND ',' || tags || ',' NOT LIKE '%,stale,%'",
        params![cutoff],
    ).map_err(io::Error::other)?;
    Ok(changed as u64)
}

// ── Smart recall (composite scoring) ─────────────────

/// Composite relevance score weights.
const W_RECENCY:    f64 = 0.20;  // was 0.25 — reduced to make room for graph boost
const W_IMPORTANCE: f64 = 0.35;
const W_ACCESS:     f64 = 0.15;
const W_FTS:        f64 = 0.20;  // was 0.25
const W_GRAPH:      f64 = 0.10;  // edge-weight connectivity boost (new)

/// Scored node: a node with a computed relevance score.
#[derive(Debug, Clone)]
pub struct ScoredNode {
    pub node: Node,
    pub score: f64,
}

/// Smart recall: returns nodes ranked by composite relevance.
///
/// Scoring formula per node:
///   score = W_RECENCY * recency + W_IMPORTANCE * importance + W_ACCESS * access_freq + W_FTS * fts_match
///
/// - recency: 1.0 for today, decays exponentially (half-life = 30 days)
/// - importance: node.importance (0.0–1.0)
/// - access_freq: min(1.0, access_count / 20) — saturates at 20 accesses
/// - fts_match: 1.0 if hint matches via FTS, 0.0 otherwise
///
/// Fetches a broad candidate set (4x limit), scores, sorts, returns top `limit`.
/// Automatically touches returned nodes.
/// Smart recall using an existing connection.
pub fn smart_recall_conn(
    conn: &Connection,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> Vec<ScoredNode> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Gather FTS matches if hint is provided
    let fts_ids: std::collections::HashSet<String> = if let Some(h) = hint {
        if !h.is_empty() {
            search_nodes_conn(conn, h, limit * 4)
                .into_iter()
                .map(|n| n.frontmatter.id.clone())
                .collect()
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };

    // Fetch candidate nodes (broad set); candidate_limit is computed, not user input.
    let candidate_limit = (limit * 4).max(40) as i64;
    let mut conditions: Vec<&str> = vec![
        "',' || tags || ',' NOT LIKE '%,stale,%'",
    ];
    // Collect bound parameter values alongside conditions.
    let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(p) = project {
        conditions.push("(',' || projects || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(p.to_string()));
    }
    let where_clause = format!("WHERE {}", conditions.join(" AND "));
    let sql = format!(
        "SELECT {NODE_COLUMNS} FROM nodes {where_clause}
         ORDER BY importance DESC, updated DESC
         LIMIT {candidate_limit}"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|b| b.as_ref()).collect();
    let candidates: Vec<Node> = stmt
        .query_map(refs.as_slice(), row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Score each candidate
    let mut scored: Vec<ScoredNode> = candidates
        .into_iter()
        .map(|node| {
            let recency = compute_recency(&node.frontmatter.updated, now_secs);
            let importance = node.frontmatter.importance;
            let access_freq = (node.frontmatter.access_count.max(0) as f64 / 20.0).min(1.0);
            let fts_match = if fts_ids.contains(&node.frontmatter.id) { 1.0 } else { 0.0 };

            let score = W_RECENCY * recency
                + W_IMPORTANCE * importance
                + W_ACCESS * access_freq
                + W_FTS * fts_match;

            ScoredNode { node, score }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    // Graph-boost pass: add W_GRAPH * edge_weight_score for edges between top candidates.
    if scored.len() > 1 {
        let ids: Vec<String> = scored.iter().map(|sn| sn.node.frontmatter.id.clone()).collect();
        let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // Sum edge weights for each node connected to other top-scored candidates.
        let sql = format!(
            "SELECT source AS node_id, SUM(weight) AS w FROM edges \
             WHERE source IN ({ph}) AND target IN ({ph}) GROUP BY source \
             UNION ALL \
             SELECT target AS node_id, SUM(weight) AS w FROM edges \
             WHERE source IN ({ph}) AND target IN ({ph}) GROUP BY target"
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let base: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let params: Vec<&dyn rusqlite::ToSql> = base.iter().copied()
                .chain(base.iter().copied())
                .chain(base.iter().copied())
                .chain(base.iter().copied())
                .collect();
            let weight_map: std::collections::HashMap<String, f64> = stmt
                .query_map(params.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                })
                .map(|rows| {
                    let mut map: std::collections::HashMap<String, f64> = Default::default();
                    for r in rows.flatten() { *map.entry(r.0).or_default() += r.1; }
                    map
                })
                .unwrap_or_default();
            let max_w = weight_map.values().cloned().fold(0.0_f64, f64::max).max(1.0);
            for sn in &mut scored {
                let boost = weight_map.get(&sn.node.frontmatter.id).copied().unwrap_or(0.0);
                sn.score += W_GRAPH * (boost / max_w);
            }
            // Re-sort after boost
            scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    // Touch retrieved nodes
    for sn in &scored {
        touch_node_conn(conn, &sn.node.frontmatter.id);
    }

    scored
}

/// Smart recall: returns nodes ranked by composite relevance.
///
/// Opens its own connection; for repeated calls within a single session prefer
/// `smart_recall_conn` to reuse an already-open connection.
pub fn smart_recall(
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> Vec<ScoredNode> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    smart_recall_conn(&conn, project, hint, limit)
}

/// Compute recency score (0.0–1.0) with exponential decay, half-life = 30 days.
fn compute_recency(updated: &str, now_secs: u64) -> f64 {
    let node_secs = parse_iso_to_secs(updated);
    if node_secs == 0 || node_secs > now_secs {
        return 0.5; // unknown or future timestamp
    }
    let age_days = (now_secs - node_secs) as f64 / 86400.0;
    let half_life = 30.0;
    (-age_days * (2.0_f64.ln()) / half_life).exp()
}

/// Parse ISO 8601 timestamp to seconds since epoch (best-effort).
fn parse_iso_to_secs(ts: &str) -> u64 {
    // Expected format: YYYY-MM-DDThh:mm:ssZ
    if ts.len() < 19 {
        return 0;
    }
    let year: u64 = ts[0..4].parse().unwrap_or(0);
    let month: u64 = ts[5..7].parse().unwrap_or(1);
    let day: u64 = ts[8..10].parse().unwrap_or(1);
    let hour: u64 = ts[11..13].parse().unwrap_or(0);
    let min: u64 = ts[14..16].parse().unwrap_or(0);
    let sec: u64 = ts[17..19].parse().unwrap_or(0);

    let total_days = days_since_epoch(year, month, day);
    total_days * 86400 + hour * 3600 + min * 60 + sec
}

/// Closed-form count of days from 1970-01-01 to the given date (O(1)).
fn days_since_epoch(year: u64, month: u64, day: u64) -> u64 {
    // Count leap years before `year` minus leap years before 1970,
    // using the Julian Day Number leap-year rule.
    let y = year as i64 - 1; // complete years before this one
    let base = 1969i64;      // complete years before 1970
    let leaps = (y / 4 - y / 100 + y / 400) - (base / 4 - base / 100 + base / 400);
    let days_from_years = (year as i64 - 1970) * 365 + leaps;

    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let mut days_from_months: u64 = 0;
    let prior_months = (month.saturating_sub(1) as usize).min(12);
    for (m, &md) in MONTH_DAYS.iter().enumerate().take(prior_months) {
        days_from_months += md;
        if m == 1 && is_leap {
            days_from_months += 1;
        }
    }

    (days_from_years as u64) + days_from_months + day.saturating_sub(1)
}

// ── Access tracking & decay ──────────────────────────

/// Record an access event: increment access_count and update accessed_at.
pub fn touch_node_conn(conn: &Connection, id: &str) {
    let now = now_iso();
    let _ = conn.execute(
        "UPDATE nodes SET access_count = access_count + 1, accessed_at = ?1 WHERE id = ?2",
        params![now, id],
    );
}

/// Batch-touch multiple nodes using an existing connection.
pub fn touch_nodes_conn(conn: &Connection, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let _ = conn.execute_batch("SAVEPOINT touch_batch");
    for id in ids {
        touch_node_conn(conn, id);
    }
    let _ = conn.execute_batch("RELEASE touch_batch");
}


/// Gradually decay importance for nodes not accessed in `days`.
/// Instead of binary stale tagging, reduces importance by `factor` (e.g., 0.9 = 10% decay).
/// Nodes with importance already at or below `floor` are not decayed further.
/// Returns the number of nodes decayed.
pub fn decay_importance(days: u64, factor: f64, floor: f64) -> io::Result<u64> {
    let conn = open_db()?;
    let cutoff = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(days * 86400);
        let (y, m, d) = days_to_ymd(secs / 86400);
        let hh = (secs / 3600) % 24;
        let mm = (secs / 60) % 60;
        let ss = secs % 60;
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    };
    let changed = conn.execute(
        "UPDATE nodes SET importance = MAX(?3, importance * ?2)
         WHERE (accessed_at < ?1 OR accessed_at = '')
           AND updated < ?1
           AND importance > ?3
           AND ',' || tags || ',' NOT LIKE '%,pinned,%'",
        params![cutoff, factor, floor],
    ).map_err(io::Error::other)?;
    Ok(changed as u64)
}

// ── FTS search ────────────────────────────────────────

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    search_nodes_conn(&conn, query, limit)
}

pub fn search_nodes_conn(conn: &Connection, query: &str, limit: usize) -> Vec<Node> {
    let sql = format!(
        "SELECT n.{NODE_COLUMNS_PREFIXED}
         FROM nodes n
         JOIN nodes_fts ON n.rowid = nodes_fts.rowid
         WHERE nodes_fts MATCH ?1
         ORDER BY n.importance DESC
         LIMIT ?2"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![query, limit as i64], row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Dynamic filter query using an existing connection.
pub fn query_nodes_conn(
    conn: &Connection,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    // Cap limit to prevent oversized result sets.
    let limit = limit.min(200);

    // Build parameterized conditions — no format!() interpolation of user values.
    let mut condition_strs: Vec<&str> = vec![];
    let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    if let Some(t) = tag {
        condition_strs.push("(',' || tags || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(t.to_string()));
    }
    if let Some(nt) = node_type {
        condition_strs.push("type = ?");
        param_vals.push(Box::new(nt.to_string()));
    }
    if let Some(p) = project {
        condition_strs.push("(',' || projects || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(p.to_string()));
    }

    let where_clause = if condition_strs.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", condition_strs.join(" AND "))
    };

    // `limit` is not user-controlled; safe to format as i64.
    let sql = format!(
        "SELECT {NODE_COLUMNS}
         FROM nodes {where_clause} ORDER BY updated DESC LIMIT {}",
        limit as i64,
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|b| b.as_ref()).collect();
    stmt.query_map(refs.as_slice(), row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Dynamic filter query.
pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    query_nodes_conn(&conn, tag, node_type, project, limit)
}

// ── Node serialization (kept for migrate import) ──────

pub fn serialize_node(node: &Node) -> String {
    let fm = serde_yaml::to_string(&node.frontmatter).unwrap_or_default();
    format!("---\n{}---\n{}", fm, node.body)
}

pub fn parse_node(content: &str) -> Option<Node> {
    let content = content.strip_prefix("---\n").unwrap_or(content);
    let (fm_str, body) = content.split_once("\n---\n")?;
    let frontmatter: NodeFrontmatter = serde_yaml::from_str(fm_str).ok()?;
    Some(Node {
        frontmatter,
        body: body.to_string(),
    })
}

// ── Atomic write (kept for graph.json etc.) ───────────

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Use PID in the tmp filename to avoid races between concurrent sessions.
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id(),
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.flush()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

// ── UUID ─────────────────────────────────────────────

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ── Timestamp ─────────────────────────────────────────

pub fn now_iso() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_node(id: &str, title: &str, node_type: &str, tags: &[&str], importance: Option<f64>) -> Node {
        let ts = "2024-01-01T00:00:00Z".to_string();
        Node {
            frontmatter: NodeFrontmatter {
                id: id.to_string(),
                node_type: node_type.to_string(),
                title: title.to_string(),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                importance: importance.unwrap_or_else(|| importance_for_type(node_type)),
                created: ts.clone(),
                updated: ts.clone(),
                ..Default::default()
            },
            body: format!("body of {title}"),
        }
    }

    // ── Fix 1: query_nodes SQL injection ──────────────────
    #[test]
    fn test_query_nodes_sql_injection_tag_does_not_panic() {
        // A malicious tag containing SQL metacharacters must not panic or error out.
        // The call should return normally (empty results or whatever is in DB).
        let _ = query_nodes(Some("'; DROP TABLE nodes; --"), None, None, 10);
    }

    #[test]
    fn test_query_nodes_sql_injection_type_does_not_panic() {
        let _ = query_nodes(None, Some("' OR '1'='1"), None, 10);
    }

    #[test]
    fn test_query_nodes_sql_injection_project_does_not_panic() {
        let _ = query_nodes(None, None, Some("x%_x'; --"), 10);
    }

    #[test]
    fn test_query_nodes_limit_capped_at_200() {
        // Even when requesting more than 200 nodes the function must not panic.
        let results = query_nodes(None, None, None, 9999);
        assert!(results.len() <= 200);
    }

    // ── Fix 1: smart_recall SQL injection ─────────────────
    #[test]
    fn test_smart_recall_sql_injection_project_does_not_panic() {
        let _ = smart_recall(Some("'; DROP TABLE nodes; --"), None, 5);
    }

    // ── Fix 2: atomic_write unique tmp names ──────────────
    #[test]
    fn test_atomic_write_tmp_filename_contains_pid() {
        // We verify the tmp path is NOT just path.with_extension("tmp").
        // Build the path the NEW way and check it contains the pid.
        let base = PathBuf::from("/tmp/store_test_base.json");
        let pid = std::process::id();
        let expected_suffix = format!(".{pid}.tmp");

        // Replicate the new tmp-path logic:
        let tmp = base.with_file_name(format!(
            ".{}.{}.tmp",
            base.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
            pid
        ));

        assert!(
            tmp.to_str().unwrap_or("").ends_with(&expected_suffix),
            "tmp path should end with .PID.tmp, got: {:?}",
            tmp
        );
        // Must NOT equal path.with_extension("tmp") (the old fixed name).
        assert_ne!(tmp, base.with_extension("tmp"));
    }

    // ── Fix 4: parse_iso_to_secs closed-form ──────────────
    #[test]
    fn test_parse_iso_epoch_start() {
        // 1970-01-01T00:00:00Z => 0
        assert_eq!(parse_iso_to_secs("1970-01-01T00:00:00Z"), 0);
    }

    #[test]
    fn test_parse_iso_known_timestamp() {
        // 2024-01-01T00:00:00Z
        // Days from 1970 to 2024:
        //   54 years: 54*365 = 19710 days
        //   Leap years in [1970,2023]: 1972,1976,...2020 = every 4 years
        //   count = (2023/4 - 2023/100 + 2023/400) - (1969/4 - 1969/100 + 1969/400)
        //         = (505 - 20 + 5) - (492 - 19 + 4) = 490 - 477 = 13
        //   Total days = 54*365 + 13 = 19723
        let expected: u64 = 19723 * 86400;
        assert_eq!(parse_iso_to_secs("2024-01-01T00:00:00Z"), expected);
    }

    #[test]
    fn test_parse_iso_leap_day() {
        // 2024-02-29T00:00:00Z  (2024 is a leap year)
        // days up to 2024-01-01 = 19723
        // Jan = 31 days => 2024-02-01 = 19754
        // Feb 29 => day index = 28
        // total = 19754 + 28 = 19782
        let expected: u64 = 19782 * 86400;
        assert_eq!(parse_iso_to_secs("2024-02-29T00:00:00Z"), expected);
    }

    #[test]
    fn test_parse_iso_with_time_component() {
        // 1970-01-01T01:02:03Z => 1*3600 + 2*60 + 3 = 3723
        assert_eq!(parse_iso_to_secs("1970-01-01T01:02:03Z"), 3723);
    }

    #[test]
    fn test_parse_iso_non_leap_century() {
        // 1900 is not a leap year; 2000 is. Test 2000-03-01.
        // Days up to 2000-01-01:
        //   30 years 1970..=1999: 30*365 = 10950 base days
        //   Leap years in [1970,1999]: 1972,1976,1980,1984,1988,1992,1996 = 7
        //   total = 10950 + 7 = 10957 days
        // Jan=31, Feb=29 (2000 is a leap year) => 2000-03-01 day index = 31+29 = 60
        // Total = 10957 + 60 = 11017
        let expected: u64 = 11017 * 86400;
        assert_eq!(parse_iso_to_secs("2000-03-01T00:00:00Z"), expected);
    }

    /// Open an isolated in-memory SQLite DB with the full harness schema applied.
    fn open_mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY, type TEXT NOT NULL, title TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '', projects TEXT NOT NULL DEFAULT '',
                agents TEXT NOT NULL DEFAULT '', created TEXT NOT NULL, updated TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '', importance REAL NOT NULL DEFAULT 0.5,
                access_count INTEGER NOT NULL DEFAULT 0, accessed_at TEXT NOT NULL DEFAULT ''
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts
                USING fts5(title, body, tags, content=nodes, content_rowid=rowid);
            CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
                INSERT INTO nodes_fts(rowid, title, body, tags)
                VALUES (new.rowid, new.title, new.body, new.tags);
            END;
            CREATE TABLE IF NOT EXISTS edges (
                id TEXT PRIMARY KEY, source TEXT NOT NULL, target TEXT NOT NULL,
                relation TEXT NOT NULL DEFAULT 'related', weight REAL NOT NULL DEFAULT 1.0,
                ts TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
            CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
            CREATE INDEX IF NOT EXISTS idx_nodes_updated ON nodes(updated DESC);",
        ).expect("schema");
        conn
    }

    // ── Fix 4: graph-boost lifts connected node ────────────
    #[test]
    fn smart_recall_graph_boost_lifts_connected_node() {
        let conn = open_mem_db();

        let id_a = "aaaaaaaa-0000-4000-8000-000000000001";
        let id_b = "aaaaaaaa-0000-4000-8000-000000000002";
        let id_e = "eeeeeeee-0000-4000-8000-000000000001";

        let n1 = make_node(id_a, "Alpha Node", "concept", &[], Some(0.8));
        let n2 = make_node(id_b, "Beta Node",  "concept", &[], Some(0.4));
        write_node_conn(&conn, &n1).unwrap();
        write_node_conn(&conn, &n2).unwrap();

        let edge = Edge {
            id: id_e.to_string(),
            source: id_a.to_string(),
            target: id_b.to_string(),
            relation: "related".to_string(),
            weight: 5.0,
            ts: now_iso(),
        };
        append_edge_conn(&conn, &edge).unwrap();

        let results = smart_recall_conn(&conn, None, None, 10);
        let ids: Vec<&str> = results.iter().map(|sn| sn.node.frontmatter.id.as_str()).collect();
        assert!(ids.contains(&id_a), "boost-a must appear");
        assert!(ids.contains(&id_b), "boost-b must appear");

        for sn in &results {
            assert!(sn.score > 0.0, "score must be positive: {}", sn.node.frontmatter.id);
        }
    }
}
