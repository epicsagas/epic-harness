//! retention.rs — Age out observation rows and the runtime files keyed on them.
//!
//! Nothing deleted observations before this: `delete_obs_older_than_pool` existed
//! but had no caller outside its own test, so the table only ever grew. The
//! per-session runtime files had the same problem, made worse by the old
//! PID-derived session id — a hook runs in its own process, so every tool call
//! could leave a fresh one-byte telemetry counter and a stale resume lock.
//!
//! Runs at session end, after reflect has analyzed the day.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::common::*;

/// Runtime files are dropped once they are older than this. Well past any live
/// session, so an in-flight lock is never removed.
const RUNTIME_FILE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Delete observations past the configured cutoff and sweep stale runtime files.
///
/// Returns `(rows_deleted, files_deleted)`. A `retention_days` of `0` keeps all
/// history but still sweeps runtime files — those are pure scratch state.
pub fn run() -> (u64, usize) {
    let days = crate::config::CONFIG.db.retention_days;
    let rows = if days == 0 { 0 } else { delete_old_rows(days) };
    let files = sweep_runtime_files(&harness_dir(), &obs_dir(), SystemTime::now());
    (rows, files)
}

fn delete_old_rows(days: u64) -> u64 {
    // `timestamp` holds ISO text and is compared lexicographically, so the
    // cutoff has to be in the same shape the column stores.
    let cutoff = format!("{}T00:00:00", iso_day(&days_ago(days)));
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        crate::store::observations::delete_obs_older_than_pool(&pool, &cutoff).await
    })
    .unwrap_or_else(|e| {
        eprintln!("[retention] observation cleanup failed: {e}");
        0
    })
}

/// `YYYYMMDD` → `YYYY-MM-DD`. Any other shape passes through unchanged.
fn iso_day(raw: &str) -> String {
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..8])
    } else {
        raw.to_string()
    }
}

/// Remove per-session scratch files that no live session can still be using.
///
/// Covers the two that accumulate one-per-hook-process: telemetry error
/// counters in `obs/` and resume locks in the harness root. `now` is a parameter
/// so a test can move time forward instead of back-dating files.
pub(crate) fn sweep_runtime_files(harness: &Path, obs: &Path, now: SystemTime) -> usize {
    let stale = |p: &Path| -> bool {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .map(|t| {
                now.duration_since(t)
                    .map(|age| age > RUNTIME_FILE_MAX_AGE)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    };

    let mut removed = 0;
    let targets = [
        (harness, "resume.", ".lock"),
        (obs, "telemetry_error_count_", ".txt"),
    ];
    for (dir, prefix, suffix) in targets {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(prefix) || !name.ends_with(suffix) {
                continue;
            }
            let path = entry.path();
            // Never follow a symlink out of the harness directory.
            let is_regular = path
                .symlink_metadata()
                .map(|m| m.file_type().is_file())
                .unwrap_or(false);
            if !is_regular {
                continue;
            }
            if stale(&path) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    /// Two days past the files just created, so they read as stale.
    fn later() -> SystemTime {
        SystemTime::now() + Duration::from_secs(48 * 60 * 60)
    }

    #[test]
    fn iso_day_expands_compact_dates() {
        assert_eq!(iso_day("20260727"), "2026-07-27");
        assert_eq!(iso_day("2026-07-27"), "2026-07-27");
    }

    #[test]
    fn sweep_removes_stale_per_session_scratch_files() {
        let dir = tempdir().unwrap();
        let harness = dir.path();
        let obs = harness.join("obs");
        fs::create_dir(&obs).unwrap();

        let lock = harness.join("resume.20260101_1234.lock");
        let counter = obs.join("telemetry_error_count_20260101_1234.txt");
        let unrelated = harness.join("config.toml");
        for p in [&lock, &counter, &unrelated] {
            File::create(p).unwrap();
        }

        assert_eq!(sweep_runtime_files(harness, &obs, later()), 2);
        assert!(!lock.exists());
        assert!(!counter.exists());
        assert!(unrelated.exists(), "unrelated files must not be touched");
    }

    #[test]
    fn sweep_keeps_files_a_live_session_may_hold() {
        let dir = tempdir().unwrap();
        let harness = dir.path();
        let obs = harness.join("obs");
        fs::create_dir(&obs).unwrap();

        let lock = harness.join("resume.20260727_9999.lock");
        File::create(&lock).unwrap();

        assert_eq!(sweep_runtime_files(harness, &obs, SystemTime::now()), 0);
        assert!(lock.exists());
    }

    #[test]
    fn sweep_tolerates_missing_directories() {
        let dir = tempdir().unwrap();
        assert_eq!(
            sweep_runtime_files(
                &dir.path().join("nope"),
                &dir.path().join("also-nope"),
                later()
            ),
            0
        );
    }
}
