//! util.rs — Shared helpers: paths, UUID, timestamps, CSV, row mapping, constants

use super::types::Node;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ── Column constants ──────────────────────────────────

/// Standard SELECT columns for node queries. Use with row_to_node().
pub(crate) const NODE_COLUMNS: &str = "id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at";

/// Same columns but table-prefixed for JOIN queries.
pub(crate) const NODE_COLUMNS_PREFIXED: &str = "id, n.type, n.title, n.tags, n.projects, n.agents, n.created, n.updated, n.body, n.importance, n.access_count, n.accessed_at";

// ── CSV helpers ───────────────────────────────────────

pub(crate) fn join_csv(v: &[String]) -> String {
    v.join(",")
}

pub(crate) fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

// ── Row mapping ───────────────────────────────────────

pub(crate) fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    use super::types::NodeFrontmatter;
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

// ── Paths ─────────────────────────────────────────────

/// Returns the path to the SQLite database file (~/.harness/memory.db).
pub fn db_path() -> PathBuf {
    if let Ok(root) = std::env::var("HARNESS_ROOT") {
        return PathBuf::from(root).join(".harness").join("memory.db");
    }
    crate::shared::paths::dirs_home().join(".harness").join("memory.db")
}

/// Compatibility: returns the .harness directory (parent of db_path).
pub fn nodes_dir() -> PathBuf {
    db_path()
        .parent()
        .expect("db_path always has .harness parent")
        .to_path_buf()
}

/// graph.json path (Web UI).
pub fn graph_path() -> PathBuf {
    db_path()
        .parent()
        .expect("db_path always has .harness parent")
        .join("graph.json")
}

pub fn validate_node_id(id: &str) -> bool {
    // UUID v4 strict: xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx
    let b = id.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
        && b.iter()
            .enumerate()
            .all(|(i, &c)| matches!(i, 8 | 13 | 18 | 23) || c.is_ascii_hexdigit())
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

pub(crate) fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
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

pub(crate) fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Parse ISO 8601 timestamp to seconds since epoch (best-effort).
pub fn parse_iso_to_secs(ts: &str) -> u64 {
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
    let base = 1969i64; // complete years before 1970
    let leaps = (y / 4 - y / 100 + y / 400) - (base / 4 - base / 100 + base / 400);
    let days_from_years = (year as i64 - 1970) * 365 + leaps;

    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let mut days_from_months: u64 = 0;
    let prior_months = (month.saturating_sub(1) as usize).min(12);
    for (m, &md) in MONTH_DAYS.iter().enumerate().take(prior_months) {
        days_from_months += md;
        if m == 1 && leap {
            days_from_months += 1;
        }
    }
    (days_from_years as u64) + days_from_months + day.saturating_sub(1)
}

// ── Atomic write ─────────────────────────────────────

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
