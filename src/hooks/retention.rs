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
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::{collections::HashSet, fs::OpenOptions};

use serde::Deserialize;

use super::common::*;

/// Runtime files are dropped once they are older than this. Well past any live
/// session, so an in-flight lock is never removed.
const RUNTIME_FILE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const GLOBAL_RETENTION_CADENCE: Duration = Duration::from_secs(60 * 60);
const GLOBAL_RETENTION_LEASE_MAX_AGE: Duration = Duration::from_secs(15 * 60);

struct RetentionLease {
    path: PathBuf,
}

impl Drop for RetentionLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Deserialize)]
struct QueuedReflectionJob {
    session_id: String,
}

/// Run retention without deleting the session whose SessionEnd reflection is
/// about to read it. Long-running sessions can legitimately predate the
/// configured cutoff.
pub(crate) fn run_preserving_session(
    active_session: Option<(&str, &str)>,
) -> io::Result<(u64, usize)> {
    let mut files = sweep_runtime_files(&harness_dir(), &obs_dir(), SystemTime::now());
    let projects = harness_projects_root();
    let now = SystemTime::now();
    let Some(_lease) = try_acquire_global_retention_lease(&projects, now)? else {
        return Ok((0, files));
    };

    let active_sessions = active_reflection_sessions(&projects, active_session)?;
    let days = crate::config::CONFIG.db.retention_days;
    if days == 0 {
        record_global_retention(&projects)?;
        return Ok((0, files));
    }

    let cutoff_day = days_ago(days);
    let rows = delete_old_rows(&cutoff_day, &active_sessions)?;
    files += prune_observation_jsonl_excluding(&projects, &cutoff_day, &active_sessions)?;
    files += prune_completed_reflection_jobs(
        &projects,
        Duration::from_secs(days.saturating_mul(24 * 60 * 60)),
        now,
    )?;
    record_global_retention(&projects)?;
    Ok((rows, files))
}

fn delete_old_rows(
    cutoff_day: &str,
    active_sessions: &HashSet<(String, String)>,
) -> io::Result<u64> {
    // `timestamp` holds ISO text and is compared lexicographically, so the
    // cutoff has to be in the same shape the column stores.
    let cutoff = format!("{}T00:00:00", iso_day(cutoff_day));
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        let sessions = active_sessions.iter().cloned().collect::<Vec<_>>();
        crate::store::observations::delete_obs_older_than_except_sessions_pool(
            &pool, &cutoff, &sessions,
        )
        .await
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

/// Remove fallback observation logs older than `cutoff_day` across every
/// project. Session filenames begin with `session_YYYYMMDD_`; names with any
/// other shape are unrelated data and remain untouched.
#[cfg(test)]
pub(crate) fn prune_observation_jsonl(
    projects: &Path,
    cutoff_day: &str,
    active_session: Option<(&str, &str)>,
) -> io::Result<usize> {
    let active_sessions = active_session
        .map(|(session_id, project)| HashSet::from([(session_id.to_string(), project.to_string())]))
        .unwrap_or_default();
    prune_observation_jsonl_excluding(projects, cutoff_day, &active_sessions)
}

fn prune_observation_jsonl_excluding(
    projects: &Path,
    cutoff_day: &str,
    active_sessions: &HashSet<(String, String)>,
) -> io::Result<usize> {
    let mut removed = 0;
    let entries = match fs::read_dir(projects) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    for project in entries {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        let obs = project.path().join("obs");
        if obs
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        let obs_entries = match fs::read_dir(&obs) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in obs_entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(day) = name
                .strip_prefix("session_")
                .and_then(|rest| rest.get(..8))
                .filter(|day| day.chars().all(|c| c.is_ascii_digit()))
            else {
                continue;
            };
            if !name.ends_with(".jsonl") || day >= cutoff_day {
                continue;
            }
            let project_name = project.file_name().to_string_lossy().into_owned();
            let session_id = name
                .strip_prefix("session_")
                .and_then(|value| value.strip_suffix(".jsonl"))
                .unwrap_or_default();
            if active_sessions.contains(&(session_id.to_string(), project_name)) {
                continue;
            }
            match fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(removed)
}

fn try_acquire_global_retention_lease(
    projects: &Path,
    now: SystemTime,
) -> io::Result<Option<RetentionLease>> {
    if projects
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("retention root is a symlink: {}", projects.display()),
        ));
    }
    fs::create_dir_all(projects)?;
    if !projects.symlink_metadata()?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("retention root is not a directory: {}", projects.display()),
        ));
    }

    let marker = projects.join("retention.last");
    if marker
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("retention marker is a symlink: {}", marker.display()),
        ));
    }
    if marker
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age < GLOBAL_RETENTION_CADENCE)
    {
        return Ok(None);
    }

    let lock = projects.join("retention.lock");
    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(crate::shared::helpers::now_iso().as_bytes())?;
                file.sync_all()?;
                return Ok(Some(RetentionLease { path: lock }));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let stale = lock
                    .symlink_metadata()
                    .map(|metadata| metadata.file_type().is_file())
                    .unwrap_or(false)
                    && lock
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| now.duration_since(modified).ok())
                        .is_some_and(|age| age > GLOBAL_RETENTION_LEASE_MAX_AGE);
                if !stale {
                    return Ok(None);
                }
                match fs::remove_file(&lock) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

fn record_global_retention(projects: &Path) -> io::Result<()> {
    let marker = projects.join("retention.last");
    if marker
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("retention marker is a symlink: {}", marker.display()),
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(marker)?;
    use std::io::Write;
    file.write_all(crate::shared::helpers::now_iso().as_bytes())?;
    file.sync_all()
}

fn active_reflection_sessions(
    projects: &Path,
    active_session: Option<(&str, &str)>,
) -> io::Result<HashSet<(String, String)>> {
    let mut active = HashSet::new();
    if let Some((session_id, project)) = active_session {
        active.insert((session_id.to_string(), project.to_string()));
    }
    let entries = match fs::read_dir(projects) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(active),
        Err(error) => return Err(error),
    };
    for project in entries {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        let project_name = project.file_name().to_string_lossy().into_owned();
        let queue = project.path().join("reflect-queue");
        if queue
            .symlink_metadata()
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        let jobs = match fs::read_dir(queue) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for job in jobs {
            let job = job?;
            if !job.file_type()?.is_file() {
                continue;
            }
            let path = job.path();
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("pending") | Some("claimed")
            ) {
                continue;
            }
            let queued: QueuedReflectionJob =
                serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
            if queued.session_id.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("reflection job has empty session id: {}", path.display()),
                ));
            }
            active.insert((queued.session_id, project_name.clone()));
        }
    }
    Ok(active)
}

pub(crate) fn prune_completed_reflection_jobs(
    projects: &Path,
    max_age: Duration,
    now: SystemTime,
) -> io::Result<usize> {
    let entries = match fs::read_dir(projects) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut removed = 0;
    for project in entries {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        let queue = project.path().join("reflect-queue");
        if queue
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        let jobs = match fs::read_dir(queue) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for job in jobs {
            let job = job?;
            if !job.file_type()?.is_file() {
                continue;
            }
            let name = job.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("job_") || !name.ends_with(".completed") {
                continue;
            }
            let modified = job.metadata()?.modified()?;
            if now
                .duration_since(modified)
                .map(|age| age > max_age)
                .unwrap_or(false)
            {
                match fs::remove_file(job.path()) {
                    Ok(()) => removed += 1,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(removed)
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

    #[test]
    fn fallback_jsonl_retention_prunes_all_projects_at_cutoff() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        for slug in ["project-a", "project-b"] {
            fs::create_dir_all(projects.join(slug).join("obs")).unwrap();
            File::create(
                projects
                    .join(slug)
                    .join("obs")
                    .join("session_20260430_session.jsonl"),
            )
            .unwrap();
        }
        let boundary = projects
            .join("project-a")
            .join("obs")
            .join("session_20260501_session.jsonl");
        let unrelated = projects.join("project-a").join("obs").join("notes.jsonl");
        File::create(&boundary).unwrap();
        File::create(&unrelated).unwrap();

        let removed = prune_observation_jsonl(&projects, "20260501", None).unwrap();

        assert_eq!(removed, 2);
        assert!(boundary.exists(), "cutoff day is retained");
        assert!(
            unrelated.exists(),
            "non-session JSONL is not retention data"
        );
    }

    #[test]
    fn completed_reflection_markers_expire_but_pending_jobs_remain() {
        let dir = tempdir().unwrap();
        let queue = dir.path().join("project-a").join("reflect-queue");
        fs::create_dir_all(&queue).unwrap();
        let completed = queue.join("job_session.completed");
        let pending = queue.join("job_session.pending");
        File::create(&completed).unwrap();
        File::create(&pending).unwrap();

        let removed =
            prune_completed_reflection_jobs(dir.path(), Duration::from_secs(24 * 60 * 60), later())
                .unwrap();

        assert_eq!(removed, 1);
        assert!(!completed.exists());
        assert!(pending.exists(), "unfinished work must remain durable");
    }

    #[test]
    fn fallback_retention_preserves_ending_session_and_skips_symlinked_obs() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let project = projects.join("project-a");
        let obs = project.join("obs");
        fs::create_dir_all(&obs).unwrap();
        let active = obs.join("session_20260101_active.jsonl");
        let stale = obs.join("session_20260101_stale.jsonl");
        File::create(&active).unwrap();
        File::create(&stale).unwrap();

        let removed = prune_observation_jsonl(
            &projects,
            "20260501",
            Some(("20260101_active", "project-a")),
        )
        .unwrap();

        assert_eq!(removed, 1);
        assert!(active.exists());
        assert!(!stale.exists());
    }

    #[test]
    fn global_retention_cadence_allows_only_one_owner_per_interval() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let lease = try_acquire_global_retention_lease(&projects, SystemTime::now())
            .unwrap()
            .expect("first caller owns the lease");
        assert!(
            try_acquire_global_retention_lease(&projects, SystemTime::now())
                .unwrap()
                .is_none(),
            "a concurrent caller must not run a second global sweep"
        );
        drop(lease);
        record_global_retention(&projects).unwrap();
        assert!(
            try_acquire_global_retention_lease(&projects, SystemTime::now())
                .unwrap()
                .is_none(),
            "the cadence marker suppresses repeated global scans"
        );
    }

    #[test]
    fn queued_sessions_protect_long_running_peer_projects() {
        let dir = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let queue = projects.join("project-b").join("reflect-queue");
        fs::create_dir_all(&queue).unwrap();
        fs::write(
            queue.join("job_20260101_long.pending"),
            r#"{"session_id":"20260101_long","project":"project-b","created_at":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();

        let active =
            active_reflection_sessions(&projects, Some(("20260101_ending", "project-a"))).unwrap();
        assert!(active.contains(&("20260101_ending".into(), "project-a".into())));
        assert!(active.contains(&("20260101_long".into(), "project-b".into())));
    }

    #[cfg(unix)]
    #[test]
    fn retention_never_follows_a_project_obs_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let external = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let project = projects.join("project-a");
        fs::create_dir_all(&project).unwrap();
        let external_file = external.path().join("session_20260101_external.jsonl");
        File::create(&external_file).unwrap();
        symlink(external.path(), project.join("obs")).unwrap();

        assert_eq!(
            prune_observation_jsonl(&projects, "20260501", None).unwrap(),
            0
        );
        assert!(external_file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn retention_never_follows_a_reflection_queue_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let external = tempdir().unwrap();
        let projects = dir.path().join("projects");
        let project = projects.join("project-a");
        fs::create_dir_all(&project).unwrap();
        let external_job = external.path().join("job_stale.completed");
        File::create(&external_job).unwrap();
        symlink(external.path(), project.join("reflect-queue")).unwrap();

        assert_eq!(
            prune_completed_reflection_jobs(
                &projects,
                Duration::from_secs(0),
                SystemTime::now() + Duration::from_secs(1),
            )
            .unwrap(),
            0
        );
        assert!(external_job.exists());
    }
}
