use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use super::paths::orbit_dir;

/// Scan a directory for PIPELINE-*.json files with `"status": "running"`.
/// Returns the most recent running pipeline (by filename sort order), or None.
/// Returns None immediately if the directory does not exist (hot path optimization).
/// Symlinks are skipped to prevent path traversal attacks.
/// When multiple running files exist (should not happen; concurrent-orbit guard prevents it),
/// logs a warning and returns the most recently named one deterministically.
pub(crate) fn scan_running_pipeline_in(dir: &Path) -> Option<serde_json::Value> {
    if !dir.is_dir() {
        return None;
    }
    // Collect all PIPELINE-*.json entries (non-symlink) sorted by filename for determinism.
    // The PIPELINE-{timestamp} naming makes lexicographic order equal to creation order.
    let mut candidates: Vec<(String, serde_json::Value)> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("PIPELINE-") || !name_str.ends_with(".json") {
                return None;
            }
            let path = entry.path();
            // Symlink defense: skip symlinks to prevent path traversal.
            // Cache the metadata to avoid a second stat between the check and the read.
            let meta = path.symlink_metadata().ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            let content = fs::read_to_string(&path).ok()?;
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(val) if val.get("status").and_then(|v| v.as_str()) == Some("running") => {
                    Some((name_str.into_owned(), val))
                }
                Err(_) => {
                    eprintln!("[orbit] WARNING: Failed to parse {}", name_str);
                    None
                }
                _ => None,
            }
        })
        .collect();

    if candidates.len() > 1 {
        eprintln!(
            "[orbit] WARNING: {} running pipeline files found; using most recent",
            candidates.len()
        );
    }
    // Sort ascending by filename; the last element is the most recent timestamp.
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.into_iter().last().map(|(_, val)| val)
}

/// Scan the orbit directory for a running pipeline.
/// Delegates to `scan_running_pipeline_in` with the real orbit directory.
fn scan_running_pipeline() -> Option<serde_json::Value> {
    scan_running_pipeline_in(&orbit_dir())
}

// ── Shared orbit state cache ──────────────────────────
// Used by detect_active_orbit_id() (called from observe.rs and polish.rs on every hook fire)
// to avoid a full directory scan per tool call. TTL mirrors the guard.rs cache.

struct OrbitIdCache {
    value: Option<serde_json::Value>,
    cached_at: Instant,
    initialized: bool,
}

static ORBIT_ID_CACHE: OnceLock<Mutex<OrbitIdCache>> = OnceLock::new();
const ORBIT_ID_CACHE_TTL_SECS: u64 = 60;

fn cached_orbit_state_common() -> Option<serde_json::Value> {
    let cache = ORBIT_ID_CACHE.get_or_init(|| {
        Mutex::new(OrbitIdCache {
            value: None,
            cached_at: Instant::now(),
            initialized: false,
        })
    });
    let mut guard = cache.lock().unwrap();
    let expired =
        !guard.initialized || guard.cached_at.elapsed().as_secs() >= ORBIT_ID_CACHE_TTL_SECS;
    if !expired {
        return guard.value.clone();
    }
    let value = scan_running_pipeline();
    guard.value = value.clone();
    guard.cached_at = Instant::now();
    guard.initialized = true;
    value
}

/// Detect an active orbit pipeline by scanning PIPELINE-*.json files.
/// Results are cached for 60 seconds to avoid a directory scan per hook call.
/// Returns Some(pipeline_id) if a file with `"status": "running"` exists.
/// The returned ID is normalized: only `a-z`, `0-9`, `-`, `_` are kept.
pub fn detect_active_orbit_id() -> Option<String> {
    let val = cached_orbit_state_common()?;
    val.get("id")
        .and_then(|v| v.as_str())
        .map(normalize_pipeline_id)
}

/// Read the full pipeline state for an active orbit (uncached, authoritative).
/// Use this when you need the latest state, not a cached snapshot.
pub fn read_active_orbit_state() -> Option<serde_json::Value> {
    scan_running_pipeline()
}

/// Normalize a raw pipeline ID for safe use in filenames and observation records.
/// Keeps only `a-z`, `0-9`, `-`, `_`; replaces all other characters with `-`.
/// Truncates to 128 characters.
pub fn normalize_pipeline_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(128)
        .collect()
}

/// Sanitize a string extracted from pipeline state before emitting to LLM context.
///
/// Strips:
/// - Control characters (Unicode `Cc`: `\n`, `\r`, ESC, BEL, C1 block U+0080–U+009F)
/// - Bidirectional override/isolate characters (`Cf`: U+202A–U+202E, U+2066–U+2069)
///   which can reverse rendered text to hide prompt injection payloads
/// - Unicode line/paragraph separators (U+2028, U+2029) which act as newlines in
///   many parsers but are not caught by `is_control()`
/// - Plane-14 Unicode tag characters (U+E0000–U+E01EF), the primary LLM injection vector
///
/// Truncates to 256 Unicode scalar values.
#[allow(dead_code)]
pub fn sanitize_orbit_field(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control()
                && !('\u{E0000}'..='\u{E01EF}').contains(c)
                && !('\u{202A}'..='\u{202E}').contains(c)
                && !('\u{2066}'..='\u{2069}').contains(c)
                && *c != '\u{2028}'
                && *c != '\u{2029}'
        })
        .take(256)
        .collect()
}
