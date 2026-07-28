use super::common::*;
use std::io::{self, BufRead, BufReader, Read};

pub(crate) const SNAPSHOT_SUMMARY_MAX_BYTES: usize = 32 * 1024;
pub(crate) const SNAPSHOT_TASK_MAX_COUNT: usize = 64;
pub(crate) const SNAPSHOT_TASK_MAX_BYTES: usize = 4 * 1024;
pub(crate) const SNAPSHOT_OBS_MAX_FILES: usize = 8;
pub(crate) const SNAPSHOT_OBS_MAX_RECORDS: usize = 512;
pub(crate) const SNAPSHOT_OBS_MAX_TOTAL_BYTES: usize = 256 * 1024;

fn success_label(successes: usize, evaluated: usize) -> String {
    successes
        .saturating_mul(100)
        .checked_div(evaluated)
        .map_or_else(|| "unknown".to_string(), |rate| format!("{rate}%"))
}

fn read_obs_jsonl_bounded(obs: &std::path::Path, today_str: &str) -> io::Result<Vec<ObsRecord>> {
    fn retain_newest(
        files: &mut Vec<(String, std::path::PathBuf)>,
        candidate: (String, std::path::PathBuf),
    ) {
        files.push(candidate);
        files.sort_by(|left, right| right.0.cmp(&left.0));
        files.truncate(SNAPSHOT_OBS_MAX_FILES);
    }

    let mut today_files = Vec::new();
    let mut latest_file: Option<(String, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(obs)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.ends_with(".jsonl") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        if name.contains(today_str) {
            retain_newest(&mut today_files, (name.clone(), entry.path()));
        }
        if latest_file
            .as_ref()
            .is_none_or(|(latest, _)| name > *latest)
        {
            latest_file = Some((name, entry.path()));
        }
    }
    let files = if today_files.is_empty() {
        latest_file.into_iter().collect()
    } else {
        today_files
    };

    let mut records = Vec::new();
    let mut remaining_bytes = SNAPSHOT_OBS_MAX_TOTAL_BYTES;
    for (_, path) in files {
        if remaining_bytes == 0 || records.len() == SNAPSHOT_OBS_MAX_RECORDS {
            break;
        }
        let file = std::fs::File::open(path)?;
        let mut reader = BufReader::new(file).take(remaining_bytes as u64);
        let mut line = String::new();
        loop {
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                break;
            }
            remaining_bytes = remaining_bytes.saturating_sub(bytes);
            let complete_line = line.ends_with('\n') || remaining_bytes > 0;
            if complete_line && let Ok(record) = serde_json::from_str::<ObsRecord>(line.trim()) {
                records.push(record);
                if records.len() == SNAPSHOT_OBS_MAX_RECORDS {
                    break;
                }
            }
            line.clear();
            if remaining_bytes == 0 {
                break;
            }
        }
    }
    Ok(records)
}

fn get_obs_summary() -> Option<String> {
    let today_str = today();

    // Try SQLite first
    if let Ok(stats) = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        let slug = project_slug();
        crate::store::observations::query_obs_stats_scoped_pool(
            &pool,
            &today_str,
            &today_str,
            Some(&slug),
        )
        .await
    }) && stats.total > 0
    {
        // Rate over calls with a determined outcome; `total` would let
        // undetermined calls read as failures.
        let evaluated = stats.evaluated();
        let success_rate = success_label(stats.successes as usize, evaluated as usize);
        let error_str = if !stats.error_stats.is_empty() {
            let parts: Vec<String> = stats
                .error_stats
                .iter()
                .take(3)
                .map(|(c, n)| format!("{}:{}", c, n))
                .collect();
            format!(", errors=[{}]", parts.join(","))
        } else {
            String::new()
        };
        return Some(format!(
            "{} obs, {success_rate} success, avg={:.2}{error_str}",
            stats.total, stats.avg_score
        ));
    }

    // Fallback: read from JSONL files
    let obs = obs_dir();
    if !obs.is_dir() {
        return None;
    }

    let records = match read_obs_jsonl_bounded(&obs, &today_str) {
        Ok(records) => records,
        Err(error) => {
            eprintln!("[snapshot] bounded JSONL fallback read failed: {error}");
            return None;
        }
    };

    if records.is_empty() {
        return None;
    }

    let scored: Vec<_> = records.iter().filter(|r| r.score.is_some()).collect();
    let errors: Vec<_> = scored
        .iter()
        .filter(|r| r.result.as_deref() == Some("error"))
        .collect();
    let total = scored.len();
    let success_rate = success_label(total - errors.len(), total);
    let avg_score = if total > 0 {
        scored.iter().map(|r| r.score.unwrap_or(0.0)).sum::<f64>() / total as f64
    } else {
        0.0
    };

    let mut error_cats: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for e in &errors {
        let cat = e.failure_category.as_deref().unwrap_or("unknown");
        *error_cats.entry(cat).or_default() += 1;
    }
    let mut top_errors: Vec<_> = error_cats.into_iter().collect();
    top_errors.sort_by_key(|b| std::cmp::Reverse(b.1));
    top_errors.truncate(3);

    let error_str = if !top_errors.is_empty() {
        let parts: Vec<String> = top_errors.iter().map(|(c, n)| format!("{c}:{n}")).collect();
        format!(", errors=[{}]", parts.join(","))
    } else {
        String::new()
    };

    Some(format!(
        "{} obs, {success_rate} success, avg={avg_score:.2}{error_str}",
        records.len()
    ))
}

fn sanitize_snapshot_text(value: &str, max_bytes: usize) -> String {
    let sanitized = crate::shared::sanitize::sanitize_skill_content(value);
    let redacted = crate::shared::sanitize::mask_secrets_keep_paths(&sanitized);
    truncate_utf8(&redacted, max_bytes).to_string()
}

fn bounded_snapshot_content(input: &HookInput, obs_summary: Option<&str>) -> (String, Vec<String>) {
    let summary = match obs_summary {
        Some(obs) => {
            let conversation = input
                .conversation_summary
                .as_deref()
                .unwrap_or("Context compaction");
            format!("{conversation}. Eval: {obs}")
        }
        None => input
            .conversation_summary
            .clone()
            .unwrap_or_else(|| "Context compaction triggered".into()),
    };
    let summary = sanitize_snapshot_text(&summary, SNAPSHOT_SUMMARY_MAX_BYTES);
    let pending_tasks = input
        .pending_tasks
        .as_deref()
        .unwrap_or_default()
        .iter()
        .take(SNAPSHOT_TASK_MAX_COUNT)
        .map(|task| sanitize_snapshot_text(task, SNAPSHOT_TASK_MAX_BYTES))
        .collect();
    (summary, pending_tasks)
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_SNAPSHOT) {
        return 0;
    }
    if !harness_exists() {
        return 0;
    }
    ensure_dir(&sessions_dir());

    let obs_summary = get_obs_summary();

    let (summary, pending_tasks) = bounded_snapshot_content(input, obs_summary.as_deref());

    let snapshot = SessionSnapshot {
        timestamp: now_iso(),
        snap_type: "pre-compact".into(),
        summary,
        pending_tasks,
        context_usage: input.context_usage,
        pipeline_state: super::common::read_active_orbit_state(),
    };

    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Write to SQLite (primary)
    let sqlite_result = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        crate::store::sessions::insert_snapshot_pool(
            &pool,
            &snapshot,
            millis,
            &crate::shared::paths::project_slug(),
        )
        .await
    });

    // Also write JSONL file for backward compatibility
    let filename = format!("snapshot_{}.json", millis);
    let path = sessions_dir().join(&filename);
    let file_result = serde_json::to_string_pretty(&snapshot)
        .map_err(io::Error::other)
        .and_then(|json| crate::shared::helpers::write_private_file(&path, json));

    if let Err(error) = &sqlite_result {
        eprintln!("[snapshot] primary database write failed: {error}");
    }
    if let Err(error) = &file_result {
        eprintln!(
            "[snapshot] private fallback write failed for {}: {error}",
            path.display()
        );
    }
    if sqlite_result.is_err() && file_result.is_err() {
        eprintln!("[snapshot] no durable snapshot was written");
        return 1;
    }

    let saved_location = if file_result.is_ok() {
        filename
    } else {
        "database".to_string()
    };
    hint(
        "snapshot",
        &format!(
            "Saved: {saved_location}{}",
            obs_summary.map(|s| format!(" ({s})")).unwrap_or_default()
        ),
    );
    0
}

#[cfg(test)]
mod tests {
    use super::{
        SNAPSHOT_OBS_MAX_FILES, SNAPSHOT_OBS_MAX_RECORDS, SNAPSHOT_OBS_MAX_TOTAL_BYTES,
        SNAPSHOT_SUMMARY_MAX_BYTES, SNAPSHOT_TASK_MAX_BYTES, SNAPSHOT_TASK_MAX_COUNT,
        bounded_snapshot_content, read_obs_jsonl_bounded, success_label,
    };
    use crate::hooks::common::HookInput;
    use std::fs;

    fn write_record(path: &std::path::Path, id: usize) {
        let record = serde_json::json!({
            "timestamp": format!("2026-07-28T00:00:{id:02}Z"),
            "tool": "Bash",
            "tool_category": "bash",
            "action": format!("record-{id}"),
            "result": "success",
            "score": 1.0,
            "dimensions": null,
            "failure_category": null,
            "error_snippet": null,
            "file_ext": null,
            "sequence_id": id,
            "pipeline_id": null
        });
        fs::write(path, format!("{record}\n")).unwrap();
    }

    #[test]
    fn unknown_only_snapshot_summary_does_not_invent_success() {
        assert_eq!(success_label(0, 0), "unknown");
    }

    #[test]
    fn snapshot_content_is_redacted_and_bounded() {
        let secret = "dXNlcjpzZWNyZXQ=";
        let input = HookInput {
            conversation_summary: Some(format!(
                "Authorization: Basic {secret} {}",
                "s".repeat(SNAPSHOT_SUMMARY_MAX_BYTES * 2)
            )),
            pending_tasks: Some(
                (0..SNAPSHOT_TASK_MAX_COUNT + 10)
                    .map(|index| {
                        format!(
                            "task {index} token=ghp_0123456789abcdefghijklmnopqrstuvwxyz {}",
                            "t".repeat(SNAPSHOT_TASK_MAX_BYTES * 2)
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        };

        let (summary, tasks) = bounded_snapshot_content(&input, None);

        assert!(summary.len() <= SNAPSHOT_SUMMARY_MAX_BYTES);
        assert!(!summary.contains(secret));
        assert!(tasks.len() <= SNAPSHOT_TASK_MAX_COUNT);
        assert!(
            tasks
                .iter()
                .all(|task| task.len() <= SNAPSHOT_TASK_MAX_BYTES)
        );
        assert!(tasks.iter().all(|task| !task.contains("ghp_")));
    }

    #[test]
    fn fallback_observation_file_count_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..=SNAPSHOT_OBS_MAX_FILES {
            write_record(
                &dir.path()
                    .join(format!("session_20260728_{index:03}.jsonl")),
                index,
            );
        }

        let records = read_obs_jsonl_bounded(dir.path(), "20260728").unwrap();

        assert_eq!(records.len(), SNAPSHOT_OBS_MAX_FILES);
    }

    #[test]
    fn fallback_observation_record_count_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session_20260728_many.jsonl");
        let records = (0..=SNAPSHOT_OBS_MAX_RECORDS)
            .map(|id| {
                serde_json::json!({
                    "timestamp": format!("2026-07-28T00:00:{:02}Z", id % 60),
                    "tool": "Bash",
                    "tool_category": "bash",
                    "action": format!("record-{id}"),
                    "result": "success",
                    "score": 1.0,
                    "dimensions": null,
                    "failure_category": null,
                    "error_snippet": null,
                    "file_ext": null,
                    "sequence_id": id,
                    "pipeline_id": null
                })
            })
            .map(|record| format!("{record}\n"))
            .collect::<String>();
        fs::write(path, records).unwrap();

        let records = read_obs_jsonl_bounded(dir.path(), "20260728").unwrap();

        assert_eq!(records.len(), SNAPSHOT_OBS_MAX_RECORDS);
    }

    #[test]
    fn fallback_observation_total_bytes_are_bounded_before_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session_20260728_large.jsonl");
        let valid = serde_json::json!({
            "timestamp": "2026-07-28T00:00:00Z",
            "tool": "Bash",
            "tool_category": "bash",
            "action": "must-not-be-read",
            "result": "success",
            "score": 1.0,
            "dimensions": null,
            "failure_category": null,
            "error_snippet": null,
            "file_ext": null,
            "sequence_id": 1,
            "pipeline_id": null
        });
        fs::write(
            path,
            format!(
                "{}\n{valid}\n",
                "x".repeat(SNAPSHOT_OBS_MAX_TOTAL_BYTES + 1)
            ),
        )
        .unwrap();

        let records = read_obs_jsonl_bounded(dir.path(), "20260728").unwrap();

        assert!(records.is_empty());
    }
}
