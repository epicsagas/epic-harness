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

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id       TEXT PRIMARY KEY,
            type     TEXT NOT NULL,
            title    TEXT NOT NULL,
            tags     TEXT NOT NULL DEFAULT '',
            projects TEXT NOT NULL DEFAULT '',
            agents   TEXT NOT NULL DEFAULT '',
            created  TEXT NOT NULL,
            updated  TEXT NOT NULL,
            body     TEXT NOT NULL DEFAULT ''
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

        CREATE TABLE IF NOT EXISTS edges (
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
        ",
    )
    .map_err(io::Error::other)?;

    // Auto-migrate legacy file-based store on first open
    auto_migrate_legacy(&conn);

    Ok(conn)
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
            let _ = conn.execute(
                "INSERT OR IGNORE INTO nodes
                 (id, type, title, tags, projects, agents, created, updated, body)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    fm.id, fm.node_type, fm.title,
                    join_csv(&fm.tags), join_csv(&fm.projects), join_csv(&fm.agents),
                    fm.created, fm.updated, node.body,
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
        },
        body: row.get(8)?,
    })
}

// ── Node I/O ──────────────────────────────────────────

pub fn write_node(node: &Node) -> io::Result<()> {
    let conn = open_db()?;
    let fm = &node.frontmatter;
    conn.execute(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let conn = open_db()?;
    conn.query_row(
        "SELECT id, type, title, tags, projects, agents, created, updated, body
         FROM nodes WHERE id = ?1",
        params![id],
        row_to_node,
    )
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
    // window_hours is u64 — no SQL injection risk; bind parameter not supported
    // inside datetime() modifier strings in SQLite.
    let sql = format!(
        "SELECT id FROM nodes
         WHERE title = ?1
           AND updated > datetime('now', '-{window_hours} hours')
         ORDER BY updated DESC
         LIMIT 1"
    );
    conn.query_row(&sql, params![title], |row| row.get::<_, String>(0))
        .ok()
}

/// Write-with-dedup: opens a single connection, checks for a duplicate, and
/// writes only when none is found.  Returns `(id, was_deduplicated)`.
pub fn write_node_dedup(node: &Node, window_hours: u64) -> io::Result<(String, bool)> {
    let conn = open_db()?;
    let title = &node.frontmatter.title;

    if let Some(existing_id) = find_duplicate_in_conn(&conn, title, window_hours) {
        return Ok((existing_id, true));
    }

    let fm = &node.frontmatter;
    conn.execute(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            fm.id, fm.node_type, fm.title,
            join_csv(&fm.tags), join_csv(&fm.projects), join_csv(&fm.agents),
            fm.created, fm.updated, node.body,
        ],
    )
    .map_err(io::Error::other)?;

    Ok((node.frontmatter.id.clone(), false))
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

    let fm = &node.frontmatter;
    conn.execute(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            fm.id, fm.node_type, fm.title,
            join_csv(&fm.tags), join_csv(&fm.projects), join_csv(&fm.agents),
            fm.created, fm.updated, node.body,
        ],
    )
    .map_err(io::Error::other)?;

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

/// Query recent nodes for a project, excluding stale ones by default.
pub fn recall_project_nodes(project: &str, limit: usize) -> Vec<Node> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let sql = "SELECT id, type, title, tags, projects, agents, created, updated, body
               FROM nodes
               WHERE (',' || projects || ',' LIKE '%,' || ?1 || ',%')
                 AND (',' || tags || ',' NOT LIKE '%,stale,%')
               ORDER BY updated DESC
               LIMIT ?2";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![project, limit as i64], row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

// ── FTS search ────────────────────────────────────────

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let sql = "SELECT n.id, n.type, n.title, n.tags, n.projects, n.agents, n.created, n.updated, n.body
               FROM nodes n
               JOIN nodes_fts ON n.rowid = nodes_fts.rowid
               WHERE nodes_fts MATCH ?1
               LIMIT ?2";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![query, limit as i64], row_to_node)
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

    let mut conditions: Vec<String> = vec![];
    if let Some(t) = tag {
        // tags is a comma-separated column; use LIKE for containment
        conditions.push(format!("(',' || tags || ',' LIKE '%,{},%')", t.replace('\'', "''")));
    }
    if let Some(nt) = node_type {
        conditions.push(format!("type = '{}'", nt.replace('\'', "''")));
    }
    if let Some(p) = project {
        conditions.push(format!(
            "(',' || projects || ',' LIKE '%,{},%')",
            p.replace('\'', "''")
        ));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, type, title, tags, projects, agents, created, updated, body
         FROM nodes {} ORDER BY updated DESC LIMIT {}",
        where_clause, limit
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], row_to_node)
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
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
    let tmp = path.with_extension("tmp");
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
