use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use super::paths::orbit_dir;

/// A `running` pipeline file not updated within this window belongs to a
/// crashed orbit and is treated as NOT running. Mirrors the 45-minute
/// crash-recovery threshold in skills/orbit/SKILL.md — without it, a crash
/// leaves a `running` file forever and the orbit lock can never be
/// reacquired (`epic orbit lock` exits 1 for every future orbit).
const STALE_RUNNING_SECS: u64 = 45 * 60;

/// Parse an ISO-8601 UTC timestamp (`2026-08-30T13:57:58Z`, the `now_iso()`
/// format) to epoch seconds. Returns None for missing/malformed values.
/// Day-precision core is the same Howard Hinnant days-from-civil algorithm
/// used by `evolve::skills::date_to_ordinal`.
fn epoch_from_iso(s: &str) -> Option<u64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some((days * 86400 + h * 3600 + mi * 60 + sec) as u64)
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Scan a directory for PIPELINE-*.json files with `"status": "running"`.
/// Returns the most recent running pipeline (by filename sort order), or None.
/// Returns None immediately if the directory does not exist (hot path optimization).
/// Symlinks are skipped to prevent path traversal attacks.
/// A `running` file whose `updated_at` is older than STALE_RUNNING_SECS (or
/// missing/unparseable) is skipped — crashed orbits must not block lock
/// reclamation or pollute observe/polish orbit detection.
/// When multiple running files exist (should not happen; the orbit skill runs
/// `epic orbit lock` first — see `try_acquire_orbit_lock`), logs a warning and
/// returns the most recently named one deterministically.
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
                    let fresh = val
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .and_then(epoch_from_iso)
                        .is_some_and(|epoch| {
                            now_epoch_secs().saturating_sub(epoch) <= STALE_RUNNING_SECS
                        });
                    if fresh {
                        Some((name_str.into_owned(), val))
                    } else {
                        None
                    }
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

// ── Concurrent-orbit guard ────────────────────────────
//
// `epic orbit lock` / `epic orbit unlock` (wired in main.rs). The lock must
// outlive the short-lived CLI process, so it is a lock FILE, not a flock:
// acquisition fails while any PIPELINE-*.json is still `running` and succeeds
// otherwise. The running-pipeline scan is the source of truth for staleness —
// a crashed orbit's leftover lock is reclaimable once its `running` pipeline
// file goes stale (updated_at older than STALE_RUNNING_SECS, matching the
// SKILL.md 45-minute crash-recovery protocol).

/// Try to acquire the concurrent-orbit guard in `dir` (the orbit state dir).
/// Returns true on success (creates `.orbit.lock`), false if another orbit is
/// running. # ponytail: hole between acquire and the first PIPELINE write
/// (seconds) — a second start inside that window is not excluded.
pub fn try_acquire_orbit_lock(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let path = dir.join(".orbit.lock");
    if path.exists() {
        // Reclaimable when no pipeline is actually running (crashed orbit
        // left the lock behind).
        if scan_running_pipeline_in(dir).is_some() {
            return false;
        }
        let _ = fs::remove_file(&path);
    }
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let body = format!("{{\"pid\":{},\"acquired_at\":{epoch}}}", std::process::id());
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, body.as_bytes()))
        .is_ok()
}

/// Release the concurrent-orbit guard (best-effort; safe if absent/foreign).
pub fn release_orbit_lock(dir: &Path) {
    let _ = fs::remove_file(dir.join(".orbit.lock"));
}

/// Read the full pipeline state for an active orbit (uncached, authoritative).
/// Use this when you need the latest state, not a cached snapshot.
pub fn read_active_orbit_state() -> Option<serde_json::Value> {
    scan_running_pipeline()
}

/// Check the completion invariants `skills/orbit/SKILL.md` states as prose.
///
/// The skill documents `max_retries` and the evidence a `complete` pipeline
/// must carry, but nothing executable checked them, so pipelines reached
/// `complete` with no PR and with more audit failures than the retry budget
/// allowed. Returns one message per violation.
///
/// Advisory by design: `unlock` also runs on the abort path and after a
/// crash, where an incomplete pipeline is the expected state, so refusing to
/// unlock would strand the lock and block every later orbit.
pub fn check_completion_invariants(pl: &serde_json::Value) -> Vec<String> {
    let mut violations = Vec::new();
    let status = pl["status"].as_str().unwrap_or("");
    if status != "complete" {
        return violations;
    }

    let fails = pl["audit_fail_count"].as_u64().unwrap_or(0);
    // Absent max_retries falls back to the skill's documented default.
    let max_retries = pl["max_retries"].as_u64().unwrap_or(3);
    if fails > max_retries {
        violations.push(format!(
            "audit_fail_count ({fails}) exceeds max_retries ({max_retries}) — \
             the pipeline should have paused for user input"
        ));
    }

    // A pipeline that reached `complete` went through Ship, which creates the
    // PR. Missing PR evidence means the phase was recorded but not performed.
    let has_pr = pl["pr_url"].as_str().is_some_and(|s| !s.is_empty());
    if !has_pr {
        violations.push("marked complete without a PR url".to_string());
    }

    let phase = pl["phase"].as_str().unwrap_or("");
    if phase != "evolve" {
        violations.push(format!(
            "marked complete at phase '{phase}', expected 'evolve'"
        ));
    }

    violations
}

/// Scan the orbit dir for the pipeline being closed and warn on any violated
/// completion invariant. Returns the number of violations reported.
pub fn warn_on_invalid_completion(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut newest: Option<(String, serde_json::Value)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("PIPELINE-") || !name.ends_with(".json") {
            continue;
        }
        // Symlinks are skipped for the same reason as scan_running_pipeline_in.
        if entry.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        let Ok(content) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(pl) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        // PIPELINE-{timestamp} makes lexicographic order equal creation order.
        if newest.as_ref().is_none_or(|(n, _)| name > *n) {
            newest = Some((name, pl));
        }
    }

    let Some((name, pl)) = newest else {
        return 0;
    };
    let violations = check_completion_invariants(&pl);
    for v in &violations {
        eprintln!("[orbit] warning: {name}: {v}");
    }
    violations.len()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_from_iso_roundtrip() {
        assert_eq!(epoch_from_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_from_iso("2026-08-30T13:57:58Z"), Some(1788098278));
        assert_eq!(epoch_from_iso("garbage"), None);
        assert_eq!(epoch_from_iso(""), None);
    }

    #[test]
    fn scan_skips_stale_running_pipeline() {
        let dir = std::env::temp_dir().join(format!("epic_orbit_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // 2020 is always more than STALE_RUNNING_SECS ago.
        fs::write(
            dir.join("PIPELINE-20200101T000000.json"),
            r#"{"id":"p-old","status":"running","updated_at":"2020-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert!(
            scan_running_pipeline_in(&dir).is_none(),
            "stale running file must be ignored"
        );

        // A fresh running file is still detected (now_iso = same format as the writer).
        let fresh = crate::shared::helpers::now_iso();
        fs::write(
            dir.join("PIPELINE-20260202T000000.json"),
            format!(r#"{{"id":"p-fresh","status":"running","updated_at":"{fresh}"}}"#),
        )
        .unwrap();
        let detected = scan_running_pipeline_in(&dir)
            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(String::from));
        assert_eq!(detected, Some("p-fresh".to_string()));

        // Stale lock + no live pipeline → reclaimable (remove the fresh
        // pipeline first — while it is running, the lock must NOT be acquirable).
        fs::remove_file(dir.join("PIPELINE-20260202T000000.json")).unwrap();
        fs::write(dir.join(".orbit.lock"), "{}").unwrap();
        assert!(try_acquire_orbit_lock(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── completion invariants ──────────────────────────

    fn complete_pipeline() -> serde_json::Value {
        serde_json::json!({
            "id": "p1",
            "status": "complete",
            "phase": "evolve",
            "audit_fail_count": 1,
            "max_retries": 3,
            "pr_url": "https://example.invalid/pr/1"
        })
    }

    #[test]
    fn valid_completion_has_no_violations() {
        assert!(check_completion_invariants(&complete_pipeline()).is_empty());
    }

    /// The reported defect: pipelines reached `complete` with no PR created.
    #[test]
    fn completion_without_pr_is_flagged() {
        let mut pl = complete_pipeline();
        pl["pr_url"] = serde_json::Value::Null;
        let v = check_completion_invariants(&pl);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("without a PR"));
    }

    /// An empty string is as absent as null — `gh pr create` failing yields "".
    #[test]
    fn completion_with_empty_pr_is_flagged() {
        let mut pl = complete_pipeline();
        pl["pr_url"] = serde_json::json!("");
        assert_eq!(check_completion_invariants(&pl).len(), 1);
    }

    /// The skill must pause at max_retries; completing past it means it did not.
    #[test]
    fn completion_past_max_retries_is_flagged() {
        let mut pl = complete_pipeline();
        pl["audit_fail_count"] = serde_json::json!(5);
        let v = check_completion_invariants(&pl);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("exceeds max_retries"));
    }

    /// Exactly at the budget is allowed — 3 failures with max_retries 3 pauses,
    /// and the user may legitimately choose to continue.
    #[test]
    fn completion_at_max_retries_is_allowed() {
        let mut pl = complete_pipeline();
        pl["audit_fail_count"] = serde_json::json!(3);
        assert!(check_completion_invariants(&pl).is_empty());
    }

    #[test]
    fn completion_at_wrong_phase_is_flagged() {
        let mut pl = complete_pipeline();
        pl["phase"] = serde_json::json!("ship");
        let v = check_completion_invariants(&pl);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("phase 'ship'"));
    }

    /// A missing max_retries falls back to the skill's documented default of 3.
    #[test]
    fn missing_max_retries_uses_documented_default() {
        let mut pl = complete_pipeline();
        pl["max_retries"] = serde_json::Value::Null;
        pl["audit_fail_count"] = serde_json::json!(4);
        assert_eq!(check_completion_invariants(&pl).len(), 1);
    }

    /// Aborted and running pipelines are not held to completion invariants —
    /// unlock runs on those paths too.
    #[test]
    fn non_complete_status_is_never_flagged() {
        for status in ["running", "aborted"] {
            let mut pl = complete_pipeline();
            pl["status"] = serde_json::json!(status);
            pl["pr_url"] = serde_json::Value::Null;
            pl["audit_fail_count"] = serde_json::json!(99);
            assert!(
                check_completion_invariants(&pl).is_empty(),
                "{status} must not be checked"
            );
        }
    }

    #[test]
    fn warn_on_invalid_completion_reads_newest_pipeline() {
        let dir = std::env::temp_dir().join(format!("epic_orbit_inv_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Older pipeline is clean; the newest one is the broken one.
        fs::write(
            dir.join("PIPELINE-20260101T000000.json"),
            serde_json::to_string(&complete_pipeline()).unwrap(),
        )
        .unwrap();
        let mut bad = complete_pipeline();
        bad["pr_url"] = serde_json::Value::Null;
        fs::write(
            dir.join("PIPELINE-20260202T000000.json"),
            serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();

        assert_eq!(warn_on_invalid_completion(&dir), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn warn_on_missing_dir_is_silent() {
        let dir = std::env::temp_dir().join("epic_orbit_inv_absent_dir");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(warn_on_invalid_completion(&dir), 0);
    }
}
