use regex::Regex;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;

use super::paths::{harness_dir, session_state_dir};
use super::sanitize::truncate_utf8;

/// Agent timeout threshold in seconds (default: 600 = 10 minutes).
/// When EPIC_ORCHESTRATION is enabled, agents exceeding this runtime
/// trigger a warning hint in the observe hook.
pub const AGENT_TIMEOUT_SECS: u64 = 600;

/// Number of recent stream.jsonl entries to check for concurrent write conflict detection.
pub const CONFLICT_LOOKBACK: usize = 3;

pub fn harness_exists() -> bool {
    harness_dir().is_dir()
}

pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::other(format!(
                "private directory path is a symlink: {}",
                path.display()
            )));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(io::Error::other(format!(
                "private directory path is not a directory: {}",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
        }
        Err(error) => return Err(error),
    }

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "private directory path changed during creation: {}",
            path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn ensure_dir(path: &Path) {
    if let Err(error) = ensure_private_dir(path) {
        eprintln!(
            "[harness] cannot prepare private directory {}: {error}",
            path.display()
        );
    }
}

fn private_file_options(append: bool) -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options
}

fn open_private_file(path: &Path, append: bool) -> io::Result<fs::File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_private_dir(parent)?;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(io::Error::other(format!(
                "private file path is not a regular file: {}",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let file = private_file_options(append).open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "private file path is not a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

pub fn write_private_file(path: &Path, content: impl AsRef<[u8]>) -> io::Result<()> {
    let mut file = open_private_file(path, false)?;
    file.write_all(content.as_ref())?;
    file.sync_all()
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

fn session_start_key() -> String {
    super::host::session_id().unwrap_or_else(|| std::process::id().to_string())
}

fn session_start_path(base: &Path, session_key: &str) -> std::path::PathBuf {
    base.join(format!("session_start.{session_key}.json"))
}

fn read_host_session_start_date(session_key: &str) -> io::Result<Option<String>> {
    let path = session_start_path(&session_state_dir(), session_key);
    if let Some(date) = read_session_start_date_at(&path)? {
        return Ok(Some(date));
    }
    read_session_start_date_at(&session_start_path(&harness_dir(), session_key))
}

fn parse_session_start_date(data: &str) -> io::Result<String> {
    let record: SessionStartRec = serde_json::from_str(data)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if record.date.len() != 8 || !record.date.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "session start date must be YYYYMMDD",
        ));
    }
    Ok(record.date)
}

fn read_session_start_date_at(path: &Path) -> io::Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(io::Error::other(format!(
                "session start record is not a regular file: {}",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    fs::read_to_string(path)
        .and_then(|data| parse_session_start_date(&data))
        .map(Some)
}

fn ensure_session_start_date_at(path: &Path, current_date: &str) -> io::Result<String> {
    if let Some(date) = read_session_start_date_at(path)? {
        return Ok(date);
    }
    if current_date.len() != 8 || !current_date.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "current session date must be YYYYMMDD",
        ));
    }
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }

    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(session_start_payload(current_date).as_bytes())?;
            file.sync_all()?;
            Ok(current_date.to_string())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            read_session_start_date_at(path)?
                .ok_or_else(|| io::Error::other("session start record disappeared during creation"))
        }
        Err(error) => Err(error),
    }
}

/// Create this host session's partition record once, or return its original
/// date. Corruption and persistence failures are returned to the SessionStart
/// hook instead of silently switching to a new UTC date.
pub fn ensure_session_start_date(current_date: &str) -> io::Result<String> {
    let session_key = session_start_key();
    let Some(_) = super::host::session_id() else {
        let path = session_start_path(&harness_dir(), &session_key);
        return ensure_session_start_date_at(&path, current_date);
    };

    // Preserve sessions established by a pre-global-state runtime. A
    // SessionStart in the same project migrates the old record atomically;
    // future hooks can then follow the host session across directories.
    let path = session_start_path(&session_state_dir(), &session_key);
    let date =
        read_host_session_start_date(&session_key)?.unwrap_or_else(|| current_date.to_string());
    ensure_session_start_date_at(&path, &date)
}

/// The partition date the current session started with, if `resume` wrote one.
/// Missing state returns `None`; corrupt or unreadable state is reported.
pub fn read_session_start_date() -> Option<String> {
    let session_key = session_start_key();
    let path = if super::host::session_id().is_some() {
        session_start_path(&session_state_dir(), &session_key)
    } else {
        session_start_path(&harness_dir(), &session_key)
    };
    let date = if super::host::session_id().is_some() {
        read_host_session_start_date(&session_key)
    } else {
        read_session_start_date_at(&path)
    };
    match date {
        Ok(date) => date,
        Err(error) => {
            eprintln!(
                "[harness] cannot read session start record {}: {error}",
                path.display()
            );
            None
        }
    }
}

pub fn read_jsonl(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn append_jsonl(path: &Path, record: &impl Serialize) {
    let result = serde_json::to_string(record)
        .map_err(io::Error::other)
        .and_then(|json| {
            let mut file = open_private_file(path, true)?;
            writeln!(file, "{json}")?;
            file.sync_data()
        });
    if let Err(error) = result {
        eprintln!(
            "[harness] cannot append private JSONL {}: {error}",
            path.display()
        );
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

pub(crate) fn validate_regular_tree(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "copy source is a symlink: {}",
            path.display()
        )));
    }
    if metadata.is_file() {
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "copy source is not a regular file or directory: {}",
            path.display()
        )));
    }
    for entry in fs::read_dir(path)? {
        validate_regular_tree(&entry?.path())?;
    }
    Ok(())
}

/// Like `copy_dir` but counts successes and errors instead of silently ignoring failures.
pub fn copy_dir_counted(src: &Path, dest: &Path) -> CopyResult {
    fn copy_tree(src: &Path, dest: &Path, result: &mut CopyResult) -> io::Result<()> {
        ensure_private_dir(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            let metadata = fs::symlink_metadata(&src_path)?;
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other("source changed to a symlink during copy"));
            }
            if metadata.is_dir() {
                copy_tree(&src_path, &dest_path, result)?;
            } else if metadata.is_file() {
                copy_regular_file(&src_path, &dest_path)?;
                result.ok += 1;
            } else {
                return Err(io::Error::other(
                    "source changed to a non-regular entry during copy",
                ));
            }
        }
        Ok(())
    }

    if validate_regular_tree(src).is_err() {
        return CopyResult { ok: 0, errors: 1 };
    }

    let mut result = CopyResult { ok: 0, errors: 0 };
    if copy_tree(src, dest, &mut result).is_err() {
        result.errors += 1;
    }
    result
}

pub(crate) fn copy_regular_file(src: &Path, dest: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(src)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "copy source is not a regular file: {}",
            src.display()
        )));
    }

    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut source = options.open(src)?;
    if !source.metadata()?.is_file() {
        return Err(io::Error::other("copy source is not a regular file"));
    }
    let mut destination = open_private_file(dest, false)?;
    io::copy(&mut source, &mut destination)?;
    destination.sync_all()
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
/// session's date from it, and reflect scopes analysis by day. For a host
/// session, the recorded start date remains fixed across UTC midnight.
pub fn session_id() -> String {
    let host_id = super::host::session_id();
    let recorded_date = host_id.as_ref().and_then(|_| {
        let session_key = session_start_key();
        let path = session_start_path(&session_state_dir(), &session_key);
        match read_host_session_start_date(&session_key) {
            Ok(date) => date,
            Err(error) => {
                eprintln!(
                    "[harness] invalid session start record {}; using stable invalid partition: {error}",
                    path.display()
                );
                Some("invalid".to_string())
            }
        }
    });
    session_id_for_date(
        host_id.as_deref(),
        &today(),
        recorded_date.as_deref(),
        std::process::id(),
    )
    .unwrap_or_else(|_| match host_id {
        Some(host_id) => format!("{}_{}", today(), host_id),
        None => format!("{}_{}", today(), std::process::id()),
    })
}

/// Resolve the current session identity without inventing a date for an
/// established host session. SessionStart persists the required date before
/// calling this function.
pub fn try_session_id() -> io::Result<String> {
    let host_id = super::host::session_id();
    let recorded_start_date = match host_id.as_deref() {
        Some(host_id) => {
            let path = session_start_path(&session_state_dir(), host_id);
            Some(read_host_session_start_date(host_id)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("session start record is missing: {}", path.display()),
                )
            })?)
        }
        None => None,
    };
    session_id_for_date(
        host_id.as_deref(),
        &today(),
        recorded_start_date.as_deref(),
        std::process::id(),
    )
}

#[cfg(test)]
fn session_id_at(
    base: &Path,
    host_id: Option<&str>,
    current_date: &str,
    process_id: u32,
) -> io::Result<String> {
    let recorded_start_date = match host_id {
        Some(host_id) => {
            let path = session_start_path(base, host_id);
            Some(read_session_start_date_at(&path)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("session start record is missing: {}", path.display()),
                )
            })?)
        }
        None => None,
    };
    session_id_for_date(
        host_id,
        current_date,
        recorded_start_date.as_deref(),
        process_id,
    )
}

fn session_id_for_date(
    host_id: Option<&str>,
    current_date: &str,
    recorded_start_date: Option<&str>,
    process_id: u32,
) -> io::Result<String> {
    match host_id {
        Some(host_id) => recorded_start_date
            .map(|date| format!("{date}_{host_id}"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "established host session has no persisted start date",
                )
            }),
        None => Ok(format!("{current_date}_{process_id}")),
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
    truncate_utf8(trimmed, 200).to_string()
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

    #[test]
    fn host_session_identity_keeps_recorded_start_date_across_midnight() {
        let id = session_id_for_date(
            Some("019a2f30-1c4d-7000-8f11-2b3c4d5e6f70"),
            "20260729",
            Some("20260728"),
            42,
        )
        .unwrap();

        assert_eq!(id, "20260728_019a2f30-1c4d-7000-8f11-2b3c4d5e6f70");
    }

    #[test]
    fn established_host_session_identity_requires_persisted_start_date() {
        let dir = tempfile::tempdir().expect("tempdir");

        let error = session_id_at(dir.path(), Some("host-session"), "20260729", 42)
            .expect_err("an established host session must not invent a new date");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn established_host_session_identity_rejects_corrupt_start_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = session_start_path(dir.path(), "host-session");
        std::fs::write(&path, "{not-json").unwrap();

        let error = session_id_at(dir.path(), Some("host-session"), "20260729", 42)
            .expect_err("corrupt state must not produce a replacement identity");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn session_start_attribution_paths_are_isolated_by_host_session() {
        let dir = tempfile::tempdir().expect("tempdir");

        let first = session_start_path(dir.path(), "session-a");
        let second = session_start_path(dir.path(), "session-b");

        assert_ne!(first, second);
        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("session_start.session-a.json")
        );
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("session_start.session-b.json")
        );
    }

    #[test]
    fn session_start_date_is_created_once_and_kept_across_midnight() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = session_start_path(dir.path(), "session-a");

        assert_eq!(
            ensure_session_start_date_at(&path, "20260728").unwrap(),
            "20260728"
        );
        assert_eq!(
            ensure_session_start_date_at(&path, "20260729").unwrap(),
            "20260728",
            "an existing host session must keep its original partition"
        );
    }

    #[test]
    fn corrupt_session_start_state_is_an_error_not_a_new_date() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = session_start_path(dir.path(), "session-a");
        std::fs::write(&path, "{not-json").expect("corrupt record");

        let error = ensure_session_start_date_at(&path, "20260729")
            .expect_err("corruption must be visible");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{not-json");
    }

    #[test]
    fn session_start_persistence_failure_is_returned_to_caller() {
        let dir = tempfile::tempdir().expect("tempdir");
        let blocked_parent = dir.path().join("not-a-directory");
        std::fs::write(&blocked_parent, "file").expect("blocking file");
        let path = blocked_parent.join("session_start.session-a.json");

        assert!(
            ensure_session_start_date_at(&path, "20260728").is_err(),
            "persistence errors must not be converted to a current-date fallback"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_counted_rejects_nested_symlink_before_copying_anything() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        let external = dir.path().join("external.txt");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("safe.txt"), "safe").unwrap();
        std::fs::write(&external, "external secret").unwrap();
        symlink(&external, source.join("nested").join("linked.txt")).unwrap();

        let result = copy_dir_counted(&source, &destination);

        assert_eq!(result.ok, 0);
        assert!(result.errors > 0);
        assert!(
            !destination.join("safe.txt").exists(),
            "validation must finish before any copy starts"
        );
        assert!(!destination.join("nested").join("linked.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_counted_rejects_nested_non_regular_entry() {
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        let _socket = UnixListener::bind(source.join("nested").join("socket")).unwrap();

        let result = copy_dir_counted(&source, &destination);

        assert_eq!(result.ok, 0);
        assert!(result.errors > 0);
    }

    #[test]
    fn normalize_error_does_not_split_utf8_at_byte_limit() {
        let input = format!("{}日", "a".repeat(199));

        let normalized = normalize_error(&input);

        assert_eq!(normalized, "a".repeat(199));
        assert!(normalized.len() <= 200);
    }

    #[cfg(unix)]
    #[test]
    fn private_directory_creation_and_repair_use_owner_only_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("private");
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        ensure_private_dir(&path).unwrap();

        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_file_creation_and_repair_use_owner_only_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secret.json");
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_private_file(&path, b"new").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_file_write_rejects_a_symlink_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        let link = temp.path().join("secret.json");
        std::fs::write(&outside, "unchanged").unwrap();
        symlink(&outside, &link).unwrap();

        assert!(write_private_file(&link, b"overwritten").is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "unchanged");
    }
}
