//! util.rs — Shared helpers: paths, UUID, timestamps, atomic write
//!
//! CSV helpers, column constants, and timestamp utilities are now provided
//! by llm-kernel. This file retains only epic-harness-specific utilities.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

// ── Paths ─────────────────────────────────────────────

/// Returns the path to the SQLite database file (~/.harness/memory.db).
pub fn db_path() -> PathBuf {
    if let Ok(root) = std::env::var("HARNESS_ROOT") {
        return PathBuf::from(root).join(".harness").join("memory.db");
    }
    crate::shared::paths::dirs_home()
        .join(".harness")
        .join("memory.db")
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

/// Validate a UUID v4 string.
pub fn validate_uuid(id: &str) -> bool {
    llm_kernel::graph::types::validate_uuid(id)
}

// ── UUID ─────────────────────────────────────────────

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ── Timestamp ─────────────────────────────────────────

pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
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

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Parse ISO 8601 timestamp to seconds since epoch.
pub fn parse_iso_to_secs(ts: &str) -> u64 {
    // Simplified ISO 8601 parser: YYYY-MM-DDTHH:MM:SSZ
    let bytes = ts.as_bytes();
    if bytes.len() < 19 {
        return 0;
    }
    let year: u64 = (bytes[0] - b'0') as u64 * 1000
        + (bytes[1] - b'0') as u64 * 100
        + (bytes[2] - b'0') as u64 * 10
        + (bytes[3] - b'0') as u64;
    let month: u64 = (bytes[5] - b'0') as u64 * 10 + (bytes[6] - b'0') as u64;
    let day: u64 = (bytes[8] - b'0') as u64 * 10 + (bytes[9] - b'0') as u64;
    let hour: u64 = (bytes[11] - b'0') as u64 * 10 + (bytes[12] - b'0') as u64;
    let min: u64 = (bytes[14] - b'0') as u64 * 10 + (bytes[15] - b'0') as u64;
    let sec: u64 = (bytes[17] - b'0') as u64 * 10 + (bytes[18] - b'0') as u64;

    // Days from year
    let mut days = 0u64;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
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
    for (i, &md) in month_days.iter().enumerate() {
        if (i as u64) < month - 1 {
            days += md;
        }
    }
    days += day - 1;
    days * 86400 + hour * 3600 + min * 60 + sec
}

// ── Atomic write ─────────────────────────────────────

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
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
