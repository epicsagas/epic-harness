use super::common::*;

fn get_obs_summary() -> Option<String> {
    let obs = obs_dir();
    if !obs.is_dir() {
        return None;
    }

    let today_str = today();
    let mut files: Vec<String> = list_files(&obs, ".jsonl")
        .into_iter()
        .filter(|f| f.contains(&today_str))
        .collect();

    // Merge all today's sessions
    let mut records: Vec<ObsRecord> = vec![];
    for f in &files {
        let recs: Vec<ObsRecord> = read_jsonl_typed(&obs.join(f));
        records.extend(recs);
    }

    // Fallback: try latest file
    if records.is_empty() {
        files = list_files(&obs, ".jsonl");
        files.sort();
        if let Some(last) = files.last() {
            records = read_jsonl_typed(&obs.join(last));
        }
    }

    if records.is_empty() {
        return None;
    }

    let scored: Vec<_> = records.iter().filter(|r| r.score.is_some()).collect();
    let errors: Vec<_> = scored
        .iter()
        .filter(|r| r.result.as_deref() == Some("error"))
        .collect();
    let total = scored.len();
    let success_rate = if total > 0 {
        ((total - errors.len()) as f64 / total as f64 * 100.0) as u32
    } else {
        100
    };
    let avg_score = if total > 0 {
        scored.iter().map(|r| r.score.unwrap_or(0.0)).sum::<f64>() / total as f64
    } else {
        1.0
    };

    let mut error_cats: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for e in &errors {
        let cat = e.failure_category.as_deref().unwrap_or("unknown");
        *error_cats.entry(cat).or_default() += 1;
    }
    let mut top_errors: Vec<_> = error_cats.into_iter().collect();
    top_errors.sort_by(|a, b| b.1.cmp(&a.1));
    top_errors.truncate(3);

    let error_str = if !top_errors.is_empty() {
        let parts: Vec<String> = top_errors.iter().map(|(c, n)| format!("{c}:{n}")).collect();
        format!(", errors=[{}]", parts.join(","))
    } else {
        String::new()
    };

    Some(format!(
        "{} obs, {success_rate}% success, avg={avg_score:.2}{error_str}",
        records.len()
    ))
}

pub fn run(input: &HookInput) -> i32 {
    if !harness_exists() {
        return 0;
    }
    ensure_dir(&sessions_dir());

    let obs_summary = get_obs_summary();

    let summary = if let Some(ref obs) = obs_summary {
        let conv = input
            .conversation_summary
            .as_deref()
            .unwrap_or("Context compaction");
        format!("{conv}. Eval: {obs}")
    } else {
        input
            .conversation_summary
            .clone()
            .unwrap_or_else(|| "Context compaction triggered".into())
    };

    let snapshot = SessionSnapshot {
        timestamp: now_iso(),
        snap_type: "pre-compact".into(),
        summary,
        pending_tasks: input.pending_tasks.clone().unwrap_or_default(),
        context_usage: input.context_usage,
    };

    let filename = format!(
        "snapshot_{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let path = sessions_dir().join(&filename);
    if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write(&path, json);
    }

    hint(
        "snapshot",
        &format!(
            "Saved: {filename}{}",
            obs_summary.map(|s| format!(" ({s})")).unwrap_or_default()
        ),
    );
    0
}
