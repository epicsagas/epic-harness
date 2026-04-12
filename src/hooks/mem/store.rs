/// store.rs — Node/Edge file I/O (atomic write, file lock)

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

pub fn memory_dir() -> PathBuf {
    let home = std::env::var("HARNESS_ROOT")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".harness").join("memory")
}

pub fn nodes_dir() -> PathBuf {
    memory_dir().join("nodes")
}

pub fn edges_path() -> PathBuf {
    memory_dir().join("edges.jsonl")
}

pub fn index_path() -> PathBuf {
    memory_dir().join("index.json")
}

pub fn graph_path() -> PathBuf {
    memory_dir().join("graph.json")
}

pub fn node_path(id: &str) -> PathBuf {
    nodes_dir().join(format!("{id}.md"))
}

// ── File lock ─────────────────────────────────────────

pub struct FileLock {
    path: PathBuf,
}

impl FileLock {
    pub fn acquire(base: &Path) -> io::Result<Self> {
        let lock_path = base.with_extension("lock");
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => return Ok(FileLock { path: lock_path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "lock timeout",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

// ── Atomic write ──────────────────────────────────────

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

// ── Node serialization ────────────────────────────────

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

// ── Node I/O ──────────────────────────────────────────

pub fn write_node(node: &Node) -> io::Result<()> {
    let path = node_path(&node.frontmatter.id);
    // Ensure directory exists before acquiring lock
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _lock = FileLock::acquire(&path)?;
    let data = serialize_node(node);
    atomic_write(&path, data.as_bytes())
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let path = node_path(id);
    let content = fs::read_to_string(&path)?;
    parse_node(&content).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "failed to parse node frontmatter")
    })
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let path = node_path(id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let dir = nodes_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut ids = vec![];
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.ends_with(".md") {
            ids.push(s.trim_end_matches(".md").to_string());
        }
    }
    Ok(ids)
}

// ── Edge I/O ──────────────────────────────────────────

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let path = edges_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Create file if not exists so lock can be acquired
    let _ = fs::OpenOptions::new().create(true).append(true).open(&path);
    let _lock = FileLock::acquire(&path)?;
    let line = serde_json::to_string(edge).unwrap_or_default();
    let mut f = fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

pub fn read_edges() -> Vec<Edge> {
    let path = edges_path();
    if !path.exists() {
        return vec![];
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn write_edges(edges: &[Edge]) -> io::Result<()> {
    let path = edges_path();
    let data: String = edges
        .iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .map(|s| s + "\n")
        .collect();
    atomic_write(&path, data.as_bytes())
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let edges: Vec<Edge> = read_edges()
        .into_iter()
        .filter(|e| e.id != edge_id)
        .collect();
    write_edges(&edges)
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let edges: Vec<Edge> = read_edges()
        .into_iter()
        .filter(|e| e.source != node_id && e.target != node_id)
        .collect();
    write_edges(&edges)
}

// ── Index I/O ─────────────────────────────────────────

pub fn read_index() -> Index {
    let path = index_path();
    if !path.exists() {
        return Index::default();
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn write_index(index: &Index) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(index)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    atomic_write(&index_path(), &data)
}

pub fn upsert_index(node: &Node) -> io::Result<()> {
    let mut idx = read_index();
    let fm = &node.frontmatter;

    // Remove existing entry if present
    idx.nodes.retain(|n| n.id != fm.id);

    idx.nodes.push(IndexNode {
        id: fm.id.clone(),
        title: fm.title.clone(),
        node_type: fm.node_type.clone(),
        tags: fm.tags.clone(),
        projects: fm.projects.clone(),
        updated: fm.updated.clone(),
    });

    // Rebuild by_tag, by_type, by_project
    rebuild_index_maps(&mut idx);
    write_index(&idx)
}

pub fn remove_from_index(node_id: &str) -> io::Result<()> {
    let mut idx = read_index();
    idx.nodes.retain(|n| n.id != node_id);
    rebuild_index_maps(&mut idx);
    write_index(&idx)
}

fn rebuild_index_maps(idx: &mut Index) {
    idx.by_tag.clear();
    idx.by_type.clear();
    idx.by_project.clear();
    for n in &idx.nodes {
        for tag in &n.tags {
            idx.by_tag.entry(tag.clone()).or_default().push(n.id.clone());
        }
        idx.by_type
            .entry(n.node_type.clone())
            .or_default()
            .push(n.id.clone());
        for proj in &n.projects {
            idx.by_project
                .entry(proj.clone())
                .or_default()
                .push(n.id.clone());
        }
    }
}

// ── Timestamp ─────────────────────────────────────────

pub fn now_iso() -> String {
    // Use std only — no chrono dep
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as rough ISO8601 (UTC)
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400;
    // days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z"
    )
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
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
