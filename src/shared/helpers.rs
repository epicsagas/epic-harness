use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

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

pub fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple date calc (no chrono dep)
    let days = now / 86400;
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

/// Mirror of everything passed to [`hint`] / [`raw`], for hosts that read
/// model context from stdout rather than stderr.
///
/// Claude Code surfaces hook stderr in the transcript, so `hint` writing to
/// stderr is the whole delivery mechanism there. Codex only feeds a
/// successful hook's *stdout* to the model, so the same calls vanished — the
/// advertised resume context (snapshots, pending work, metrics, memory,
/// guard warnings) never reached it. The buffer lets the host adapter in
/// `main` replay these lines on the right channel without every call site
/// having to know which host it is running under.
static HINT_MIRROR: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn mirror_push(line: String) {
    if let Ok(mut buf) = HINT_MIRROR.lock() {
        buf.push(line);
    }
}

/// Drain the mirrored hint lines (see [`HINT_MIRROR`]).
pub fn take_hint_mirror() -> Vec<String> {
    HINT_MIRROR
        .lock()
        .map(|mut b| std::mem::take(&mut *b))
        .unwrap_or_default()
}

pub fn hint(tag: &str, msg: &str) {
    let line = format!("[{tag}] {msg}");
    eprintln!("{line}");
    mirror_push(line);
}

pub fn raw(line: &str) {
    eprintln!("{line}");
    mirror_push(line.to_string());
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

pub fn session_id() -> String {
    format!("{}_{}", today(), std::process::id())
}

/// Session id for observation grouping — prefers the host-supplied stable id
/// (Claude Code / Codex `session_id` field) over the date+PID fallback.
/// Codex spawns a fresh process per hook invocation, so `session_id()` alone
/// turns almost every tool call into its own "session"; the host id is the
/// same across a whole conversation.
pub fn resolve_session_id(host_session_id: Option<&str>) -> String {
    match host_session_id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => session_id(),
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
    crate::shared::sanitize::truncate_bytes(trimmed, 200).to_string()
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
