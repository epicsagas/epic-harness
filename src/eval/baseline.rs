//! eval/baseline.rs — Load and compare evaluation baselines

use std::path::Path;

/// Load the latest baseline from the baselines directory.
///
/// Looks for `latest.json` symlink/copy in the given directory.
pub fn load_latest(baseline_dir: &Path) -> Result<Option<serde_json::Value>, String> {
    let latest = baseline_dir.join("latest.json");
    if !latest.exists() {
        // Try to find any BASELINE-*.json if latest.json missing
        let fallback = find_newest_baseline(baseline_dir)?;
        if let Some(path) = fallback {
            return load_json(&path).map(Some);
        }
        return Ok(None);
    }
    load_json(&latest).map(Some)
}

fn load_json(path: &Path) -> Result<serde_json::Value, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("parse baseline {}: {e}", path.display()))?;
    let dimensions = value
        .get("dimensions")
        .and_then(serde_json::Value::as_object)
        .filter(|m| !m.is_empty())
        .ok_or_else(|| {
            format!(
                "invalid baseline {}: expected nonempty dimensions",
                path.display()
            )
        })?;
    if dimensions.values().any(|d| {
        d.get("score")
            .and_then(serde_json::Value::as_f64)
            .is_none_or(|s| !s.is_finite() || !(0.0..=1.0).contains(&s))
    }) {
        return Err(format!(
            "invalid baseline {}: scores must be numbers between 0 and 1",
            path.display()
        ));
    }
    Ok(value)
}

/// Find the most recent BASELINE-*.json file in a directory.
fn find_newest_baseline(dir: &Path) -> Result<Option<std::path::PathBuf>, String> {
    if !dir.exists() {
        return Ok(None);
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;

    let mut newest: Option<(std::path::PathBuf, String)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("BASELINE-") && name_str.ends_with(".json") {
            // Timestamp is embedded in filename: BASELINE-YYYYMMDDTHHMMSS.json
            let ts = name_str
                .trim_start_matches("BASELINE-")
                .trim_end_matches(".json")
                .to_string();
            match &newest {
                Some((_, best_ts)) if &ts <= best_ts => {}
                _ => newest = Some((entry.path(), ts)),
            }
        }
    }
    Ok(newest.map(|(p, _)| p))
}
