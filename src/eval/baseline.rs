//! eval/baseline.rs — Load and compare evaluation baselines

use std::path::Path;

/// Load the latest baseline from the baselines directory.
///
/// Looks for `latest.json` symlink/copy in the given directory. `Ok(None)` means
/// no baseline exists yet; an existing but unreadable or invalid file is an `Err`.
pub fn load_latest(baseline_dir: &Path) -> Result<Option<serde_json::Value>, String> {
    let latest = baseline_dir.join("latest.json");
    let latest_exists = latest
        .try_exists()
        .map_err(|e| format!("stat {}: {e}", latest.display()))?;
    if latest_exists {
        return load_json(&latest).map(Some);
    }
    // Try to find any BASELINE-*.json if latest.json missing
    match find_newest_baseline(baseline_dir)? {
        Some(path) => load_json(&path).map(Some),
        None => Ok(None),
    }
}

fn load_json(path: &Path) -> Result<serde_json::Value, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&contents).map_err(|e| format!("parse {}: {e}", path.display()))
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
