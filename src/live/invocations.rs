use super::{contained_file, recent, SESSION_WINDOW};
use serde_json::{json, Value};
use std::{collections::hash_map::DefaultHasher, fs, hash::{Hash, Hasher}, io, path::Path};

/// Record a host invocation independently of tool observations and scores.
pub(crate) fn record_invocation(base: &Path, session_id: &str, timestamp: &str) -> io::Result<()> {
    if !base.is_dir() || session_id.trim().is_empty() || session_id.len() > 1024 {
        return Ok(());
    }
    let root = base.canonicalize()?;
    let directory = base.join("live_sessions");
    fs::create_dir_all(&directory)?;
    let directory = directory.canonicalize()?;
    if !directory.starts_with(&root) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "activity directory escapes project"));
    }
    let mut key = DefaultHasher::new();
    session_id.hash(&mut key);
    let target = directory.join(format!("{:016x}.json", key.finish()));
    let temporary = directory.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&temporary)?;
    let result = serde_json::to_writer(file, &json!({"session_id": session_id, "last_seen": timestamp}))
        .map_err(io::Error::other)
        .and_then(|()| fs::rename(&temporary, &target));
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

pub(super) fn read(base: &Path, project: &str, now: u64) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(base.join("live_sessions")) else { return Vec::new() };
    let mut entries: Vec<_> = entries.flatten().filter_map(|entry| {
        let path = entry.path();
        let metadata = entry.metadata().ok()?;
        if path.extension()?.to_str()? != "json" || metadata.len() > 16384 || !contained_file(&path, base) { return None; }
        Some((metadata.modified().ok()?, path))
    }).collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    entries.into_iter().take(100).filter_map(|(_, path)| {
        let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
        let id = value["session_id"].as_str()?;
        let last_seen = value["last_seen"].as_str()?;
        if id.is_empty() || !recent(last_seen, now, SESSION_WINDOW) { return None; }
        Some(json!({"session_id": id, "project": project, "last_seen": last_seen,
            "date": &last_seen[..10], "tool_calls": 0, "avg_score": 0.0, "failures": 0}))
    }).collect()
}
