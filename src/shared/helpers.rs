use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use super::paths::harness_dir;

/// Agent timeout threshold in seconds (default: 600 = 10 minutes).
/// When EPIC_ORCHESTRATION is enabled, agents exceeding this runtime
/// trigger a warning hint in the observe hook.
pub const AGENT_TIMEOUT_SECS: u64 = 600;

/// Number of recent stream.jsonl entries to check for concurrent write conflict detection.
pub const CONFLICT_LOOKBACK: usize = 3;

pub fn harness_exists() -> bool {
    harness_dir().is_dir()
}

pub fn ensure_dir(path: &Path) {
    let _ = fs::create_dir_all(path);
}

/// Civil date `YYYYMMDD` for a count of days since the Unix epoch.
fn civil_date(days_since_epoch: i64) -> String {
    // Simple date calc (no chrono dep)
    let mut y = 1970i64;
    let mut remaining = days_since_epoch;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &d) in month_days.iter().enumerate() {
        if remaining < d as i64 {
            m = i;
            break;
        }
        remaining -= d as i64;
    }
    format!("{:04}{:02}{:02}", y, m + 1, remaining + 1)
}

fn epoch_days() -> i64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400) as i64
}

pub fn today() -> String {
    civil_date(epoch_days())
}

/// `YYYYMMDD` for the UTC date `n` days before today. Used for retention cutoffs.
pub fn days_ago(n: u64) -> String {
    civil_date(epoch_days().saturating_sub(n as i64).max(0))
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Reuse today() logic for full ISO
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 0usize;
    for (i, &d) in month_days.iter().enumerate() {
        if remaining < d as i64 {
            mo = i;
            break;
        }
        remaining -= d as i64;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo + 1,
        remaining + 1,
        h,
        m,
        s
    )
}

/// Emit a tagged human-facing line.
///
/// Routed to stdout when the host consumes plain stdout as model context (Codex
/// `SessionStart`), otherwise to stderr. Without this, everything `resume`
/// surfaces — previous snapshot, pending work, metrics, memory, team and
/// orchestration state — was written to stderr and never reached a Codex model.
pub fn hint(tag: &str, msg: &str) {
    if crate::shared::host::captures_session_start_context() {
        crate::shared::host::append_session_start_context(&format!("[{tag}] {msg}"));
    } else if crate::shared::host::stdout_is_context() {
        println!("[{tag}] {msg}");
    } else {
        eprintln!("[{tag}] {msg}");
    }
}

/// Emit an untagged human-facing line. Same routing rules as [`hint`].
pub fn raw(line: &str) {
    if crate::shared::host::captures_session_start_context() {
        crate::shared::host::append_session_start_context(line);
    } else if crate::shared::host::stdout_is_context() {
        println!("{line}");
    } else {
        eprintln!("{line}");
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path, fallback: T) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(fallback)
}

/// On-disk shape of `session_start_file` — just the partition date the
/// SessionStart hook used, plus a write timestamp for debugging.
#[derive(serde::Deserialize)]
struct SessionStartRec {
    date: String,
}

/// JSON payload for the session-start record. Separate so the write/read
/// round-trip is unit-testable without touching `harness_dir()`.
fn session_start_payload(date: &str) -> String {
    serde_json::json!({ "date": date, "written_at": now_iso() }).to_string()
}

/// Record the partition date the SessionStart hook used this session, so
/// SessionEnd reproduces the same holdout arm even if the session spans UTC
/// midnight. Best-effort: failure is non-fatal (reflect falls back to today).
pub fn write_session_start(date: &str) {
    let _ = fs::write(
        super::paths::session_start_file(),
        session_start_payload(date),
    );
}

/// The partition date the current session started with, if `resume` wrote one.
/// None on a cold start (no session yet) or an unreadable/corrupt record —
/// callers fall back to `today()`.
pub fn read_session_start_date() -> Option<String> {
    let data = fs::read_to_string(super::paths::session_start_file()).ok()?;
    serde_json::from_str::<SessionStartRec>(&data)
        .ok()
        .map(|r| r.date)
}

pub fn read_jsonl(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn read_jsonl_typed<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn append_jsonl(path: &Path, record: &impl Serialize) {
    use std::io::Write;
    if let Ok(json) = serde_json::to_string(record)
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = writeln!(f, "{json}");
    }
}

pub fn list_dirs(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_files(dir: &Path, ext: &str) -> Vec<String> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.ends_with(ext))
                .collect()
        })
        .unwrap_or_default()
}

pub fn copy_dir(src: &Path, dest: &Path) {
    if !src.is_dir() {
        return;
    }
    ensure_dir(dest);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir(&src_path, &dest_path);
            } else {
                let _ = fs::copy(&src_path, &dest_path);
            }
        }
    }
}

pub struct CopyResult {
    pub ok: u64,
    pub errors: u64,
}

/// Like `copy_dir` but counts successes and errors instead of silently ignoring failures.
pub fn copy_dir_counted(src: &Path, dest: &Path) -> CopyResult {
    let mut result = CopyResult { ok: 0, errors: 0 };
    if !src.is_dir() {
        return result;
    }
    ensure_dir(dest);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                let sub = copy_dir_counted(&src_path, &dest_path);
                result.ok += sub.ok;
                result.errors += sub.errors;
            } else {
                match fs::copy(&src_path, &dest_path) {
                    Ok(_) => result.ok += 1,
                    Err(_) => result.errors += 1,
                }
            }
        }
    }
    result
}

pub fn rm_dir(dir: &Path) {
    if dir.is_dir() {
        let _ = fs::remove_dir_all(dir);
    }
}

/// Identifier for the current conversation, stable across hook processes.
///
/// Prefers the host-supplied conversation id recorded by `host::init`. The PID
/// fallback only applies when the host sends none: a hook runs in its own
/// process, so on Codex the PID form produced a distinct "session" for nearly
/// every tool call, which broke sequence detection, repeated-error analysis and
/// the per-session telemetry cap.
///
/// The `YYYYMMDD_` prefix is kept in both forms — the dashboard derives a
/// session's date from it, and reflect scopes analysis by day.
pub fn session_id() -> String {
    match super::host::session_id() {
        Some(host_id) => format!("{}_{}", today(), host_id),
        None => format!("{}_{}", today(), std::process::id()),
    }
}

/// Returns true when EPIC_ORCHESTRATION=enabled env var is set.
#[allow(dead_code)]
pub fn is_orchestration_enabled() -> bool {
    std::env::var("EPIC_ORCHESTRATION").as_deref() == Ok("enabled")
}

pub fn hash_string(s: &str) -> String {
    let mut hash: u32 = 0;
    for b in s.bytes() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(b as u32);
    }
    format!("{:08x}", hash)
}

pub fn normalize_error(snippet: &str) -> String {
    static TS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\dZ]*").unwrap());
    static LC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":\d+:\d+").unwrap());
    static PATH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/[\w./-]+/").unwrap());
    static WS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

    let s = TS_RE.replace_all(snippet, "");
    let s = LC_RE.replace_all(&s, ":L:C");
    let s = PATH_RE.replace_all(&s, "/PATH/");
    let s = WS_RE.replace_all(&s, " ");
    let trimmed = s.trim();
    trimmed[..trimmed.len().min(200)].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_start_payload_round_trips_date() {
        let payload = session_start_payload("20260703");
        let rec: SessionStartRec = serde_json::from_str(&payload).unwrap();
        assert_eq!(rec.date, "20260703");
        assert!(payload.contains("written_at"));
    }
}
