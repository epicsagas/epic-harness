//! digester.rs — HarnessX-inspired trace compression
//!
//! Reduces a session's raw `ObsRecord` stream into structured per-task
//! [`TaskDigest`] summaries: binary outcome, ranked failure categories,
//! implicated logical components, curated evidence excerpts, the tool
//! trajectory, and a cross-iteration counter.
//!
//! This is epic-harness's analog of HarnessX's Digester stage (paper §4.3).
//! The raw observation stream is session-scoped; the digester re-segments it
//! into task-scoped narratives that the Planner (§4.3) and proposal builder
//! consume. Unlike the paper's 10M-token scale, epic-harness hooks fire
//! per-tool-call so traces are naturally bounded — session-level compression
//! suffices.
//!
//! ## Segmentation strategy
//! 1. If observations carry a `pipeline_id` (orbit runs), group by it.
//! 2. Otherwise, segment by an idle gap exceeding `SEGMENT_GAP_SECS` seconds.
//! 3. Fall back to a single whole-session segment.

use std::collections::HashMap;

use crate::shared::evolution::{TaskDigest, TaskOutcome};
use crate::shared::obs::ObsRecord;

/// Idle gap (seconds) that splits a session into separate task segments when
/// no pipeline_id is present. Tuned to match typical inter-task pauses.
const SEGMENT_GAP_SECS: i64 = 300; // 5 minutes

/// Compress a session's observations into per-task digests.
///
/// `prev_digest_task_ids` carries the set of task IDs seen in prior sessions so
/// each digest's `iterations_seen` reflects cross-iteration persistence (paper
/// §4.3: "each task's summary links to its history of prior outcomes"). Pass
/// an empty slice for a cold start.
pub fn digest_session(
    observations: &[ObsRecord],
    prev_digest_task_ids: &[String],
) -> Vec<TaskDigest> {
    if observations.is_empty() {
        return Vec::new();
    }

    let segments = segment_observations(observations);
    let prev_seen: HashMap<&str, u32> = prev_digest_task_ids
        .iter()
        .map(|id| (id.as_str(), 1u32))
        .collect();

    segments
        .into_iter()
        .map(|(task_id, seg)| build_digest(&task_id, &seg, &prev_seen))
        .collect()
}

/// Partition observations into ordered (task_id, records) segments.
fn segment_observations(observations: &[ObsRecord]) -> Vec<(String, Vec<&ObsRecord>)> {
    // Prefer explicit pipeline grouping when any observation carries a pipeline_id.
    let has_pipeline = observations.iter().any(|o| o.pipeline_id.is_some());
    if has_pipeline {
        return group_by_pipeline(observations);
    }
    segment_by_time_gap(observations)
}

/// Group by `pipeline_id`; observations without one fall into a "session" bucket.
fn group_by_pipeline(observations: &[ObsRecord]) -> Vec<(String, Vec<&ObsRecord>)> {
    let mut buckets: HashMap<String, Vec<&ObsRecord>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for o in observations {
        let key = o
            .pipeline_id
            .clone()
            .unwrap_or_else(|| "session".to_string());
        if !buckets.contains_key(&key) {
            order.push(key.clone());
        }
        buckets.entry(key).or_default().push(o);
    }
    order
        .into_iter()
        .map(|k| {
            let seg = buckets.remove(&k).unwrap_or_default();
            (k, seg)
        })
        .collect()
}

/// Split on idle gaps > SEGMENT_GAP_SECS; label segments by ordinal.
fn segment_by_time_gap(observations: &[ObsRecord]) -> Vec<(String, Vec<&ObsRecord>)> {
    let mut segments: Vec<(String, Vec<&ObsRecord>)> = Vec::new();
    let mut current: Vec<&ObsRecord> = Vec::new();
    let mut last_ts: Option<i64> = None;
    let mut idx = 0u32;

    for o in observations {
        let ts = parse_epoch(&o.timestamp);
        if let (Some(prev), Some(now)) = (last_ts, ts) {
            if now - prev > SEGMENT_GAP_SECS && !current.is_empty() {
                let label = format!("segment-{idx}");
                idx += 1;
                segments.push((label, std::mem::take(&mut current)));
            }
        }
        last_ts = ts.or(last_ts);
        current.push(o);
    }
    if !current.is_empty() {
        let label = if segments.is_empty() {
            "session".to_string()
        } else {
            format!("segment-{idx}")
        };
        segments.push((label, current));
    }
    segments
}

/// Build a single digest from a segment's records.
fn build_digest(task_id: &str, seg: &[&ObsRecord], prev_seen: &HashMap<&str, u32>) -> TaskDigest {
    let total = seg.len() as u32;
    // Success = no failure_category (matches analysis.rs convention). A record
    // with a failure_category counts as a failed step regardless of its score.
    let failures = seg.iter().filter(|o| o.failure_category.is_some()).count() as u32;
    let successes = total - failures;

    let outcome = if failures == 0 {
        TaskOutcome::Success
    } else if successes == 0 {
        TaskOutcome::CompleteFailure
    } else {
        TaskOutcome::PartialFailure {
            failed_steps: failures,
            total_steps: total,
        }
    };

    let failure_categories = rank_failure_categories(seg);
    let implicated_components = derive_components(seg);
    let evidence_excerpts = curate_excerpts(seg);
    let tool_trajectory = tool_sequence(seg);
    let token_estimate = estimate_tokens(seg);
    let iterations_seen = prev_seen.get(task_id).copied().unwrap_or(0);

    TaskDigest {
        task_id: task_id.to_string(),
        outcome,
        failure_categories,
        implicated_components,
        evidence_excerpts,
        tool_trajectory,
        iterations_seen,
        token_estimate,
        observation_count: total as u64,
    }
}

/// Rank failure categories by frequency (descending). Top entries feed the Planner.
fn rank_failure_categories(seg: &[&ObsRecord]) -> Vec<(String, u32)> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for o in seg {
        if let Some(cat) = &o.failure_category {
            *counts.entry(cat.clone()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, u32)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

/// Map file paths to logical component names (e.g. "src/auth/login.ts" → "auth").
/// Deduplicated, preserving first-seen order.
fn derive_components(seg: &[&ObsRecord]) -> Vec<String> {
    let mut components: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for o in seg {
        // The observation does not carry a direct file path; use the action field,
        // which often contains the edited path, falling back to error_snippet.
        for candidate in o.action.iter().chain(o.error_snippet.iter()) {
            if let Some(c) = extract_component(candidate) {
                if seen.insert(c.clone()) {
                    components.push(c);
                }
            }
        }
    }
    components
}

/// Extract a component label from a path-like string.
/// "src/auth/login.rs" → "auth"; "auth/login.rs" → "auth"; bare file → None.
fn extract_component(s: &str) -> Option<String> {
    let normalized = s.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty()).collect();
    // Heuristic: skip leading "src"/"crates"/"lib"; take the next segment if a
    // file extension is present later in the path (i.e. this looks like a path).
    let looks_like_path = parts.iter().any(|p| p.contains('.'));
    if !looks_like_path || parts.len() < 2 {
        return None;
    }
    let skip = |p: &str| matches!(p, "src" | "crates" | "lib" | "app" | "tests");
    for p in &parts[..parts.len() - 1] {
        if !skip(p) {
            return Some((*p).to_string());
        }
    }
    None
}

/// Select up to 3 representative error excerpts (shortest distinct snippets).
fn curate_excerpts(seg: &[&ObsRecord]) -> Vec<String> {
    let mut excerpts: Vec<String> = seg
        .iter()
        .filter_map(|o| o.error_snippet.clone())
        .filter(|s| !s.trim().is_empty())
        .collect();
    // Dedup while preserving order.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    excerpts.retain(|s| seen.insert(s.clone()));
    // Prefer shorter, more focused excerpts.
    excerpts.sort_by_key(|s| s.len());
    excerpts.truncate(3);
    excerpts
}

/// Ordered sequence of distinct tool categories used in the segment.
fn tool_sequence(seg: &[&ObsRecord]) -> Vec<String> {
    let mut seq: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for o in seg {
        let cat = if o.tool_category.is_empty() {
            o.tool.clone()
        } else {
            o.tool_category.clone()
        };
        if !cat.is_empty() && seen.insert(cat.clone()) {
            seq.push(cat);
        }
    }
    seq
}

/// Rough token estimate: ~4 chars per token across action + result + snippet text.
fn estimate_tokens(seg: &[&ObsRecord]) -> usize {
    let chars: usize = seg
        .iter()
        .map(|o| {
            o.action.as_deref().map_or(0, str::len)
                + o.result.as_deref().map_or(0, str::len)
                + o.error_snippet.as_deref().map_or(0, str::len)
        })
        .sum();
    chars / 4
}

/// Parse an ISO-8601 timestamp to epoch seconds. Returns None on failure.
fn parse_epoch(ts: &str) -> Option<i64> {
    // Accept "YYYY-MM-DDTHH:MM:SS" or "YYYY-MM-DDTHH:MM:SSZ" or with fractional.
    let ts = ts.trim_end_matches('Z');
    let (date, time) = ts.split_once('T')?;
    let dparts: Vec<&str> = date.split('-').collect();
    if dparts.len() != 3 {
        return None;
    }
    let y: i64 = dparts[0].parse().ok()?;
    let mo: i64 = dparts[1].parse().ok()?;
    let d: i64 = dparts[2].parse().ok()?;
    let time_main = time.split('.').next().unwrap_or(time);
    let tparts: Vec<&str> = time_main.split(':').collect();
    if tparts.len() < 2 {
        return None;
    }
    let h: i64 = tparts[0].parse().ok()?;
    let mi: i64 = tparts[1].parse().ok()?;
    let s: i64 = tparts.get(2).and_then(|x| x.parse().ok()).unwrap_or(0);

    // Days since Unix epoch (proleptic Gregorian, UTC). Formula from Howard Hinnant.
    let y = if mo <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + h * 3600 + mi * 60 + s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::scoring::ScoreDimensions;

    fn rec(tool: &str, cat: &str, score: Option<f64>, fail: Option<&str>) -> ObsRecord {
        ObsRecord {
            timestamp: "2026-06-16T10:00:00Z".into(),
            tool: tool.into(),
            tool_category: cat.into(),
            action: None,
            result: None,
            score,
            dimensions: score.map(|_| ScoreDimensions {
                tool_success: 1.0,
                output_quality: 1.0,
                execution_cost: 1.0,
            }),
            failure_category: fail.map(String::from),
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        }
    }

    #[test]
    fn empty_session_yields_no_digests() {
        assert!(digest_session(&[], &[]).is_empty());
    }

    #[test]
    fn all_success_segment_classified_success() {
        let obs = vec![rec("Read", "read", Some(1.0), None), rec("Edit", "edit", Some(1.0), None)];
        let digests = digest_session(&obs, &[]);
        assert_eq!(digests.len(), 1);
        assert!(matches!(digests[0].outcome, TaskOutcome::Success));
        assert_eq!(digests[0].observation_count, 2);
    }

    #[test]
    fn mixed_outcome_is_partial_failure() {
        let obs = vec![
            rec("Read", "read", Some(1.0), None),
            rec("Bash", "bash", Some(0.0), Some("type_error")),
            rec("Bash", "bash", Some(0.0), Some("type_error")),
        ];
        let digests = digest_session(&obs, &[]);
        assert!(matches!(
            digests[0].outcome,
            TaskOutcome::PartialFailure { failed_steps: 2, total_steps: 3 }
        ));
        // type_error ranked first (count 2).
        assert_eq!(digests[0].failure_categories[0], ("type_error".to_string(), 2));
    }

    #[test]
    fn pipeline_id_groups_into_separate_segments() {
        let mut a = rec("Read", "read", Some(1.0), None);
        a.pipeline_id = Some("PIPE-1".into());
        let mut b = rec("Read", "read", Some(0.0), Some("syntax_error"));
        b.pipeline_id = Some("PIPE-2".into());
        let digests = digest_session(&[a, b], &[]);
        assert_eq!(digests.len(), 2);
        let ids: Vec<&str> = digests.iter().map(|d| d.task_id.as_str()).collect();
        assert!(ids.contains(&"PIPE-1"));
        assert!(ids.contains(&"PIPE-2"));
    }

    #[test]
    fn time_gap_splits_segments() {
        let mut early = rec("Read", "read", Some(1.0), None);
        early.timestamp = "2026-06-16T10:00:00Z".into();
        let mut late = rec("Read", "read", Some(0.0), Some("type_error"));
        late.timestamp = "2026-06-16T11:00:00Z".into(); // 1h gap > 5min
        let digests = digest_session(&[early, late], &[]);
        assert_eq!(digests.len(), 2);
    }

    #[test]
    fn iterations_seen_reflects_prior_history() {
        let obs = vec![rec("Read", "read", Some(0.0), Some("type_error"))];
        let digests = digest_session(&obs, &["session".to_string()]);
        // The cold-start segment is labeled "session"; prior history bumps it.
        assert_eq!(digests[0].iterations_seen, 1);
    }

    #[test]
    fn component_extraction_from_path() {
        assert_eq!(extract_component("src/auth/login.rs"), Some("auth".into()));
        assert_eq!(extract_component("auth/login.rs"), Some("auth".into()));
        assert_eq!(extract_component("login.rs"), None);
        assert_eq!(extract_component("not a path"), None);
    }

    #[test]
    fn excerpts_dedup_and_truncate() {
        let mut o = rec("Bash", "bash", Some(0.0), Some("type_error"));
        o.error_snippet = Some("type mismatch".into());
        let mut o2 = rec("Bash", "bash", Some(0.0), Some("type_error"));
        o2.error_snippet = Some("type mismatch".into()); // dup
        let mut o3 = rec("Bash", "bash", Some(0.0), Some("type_error"));
        o3.error_snippet = Some("a much longer and more verbose error message than the first".into());
        let digests = digest_session(&[o, o2, o3], &[]);
        // 2 distinct excerpts (dup removed), shortest first.
        assert_eq!(digests[0].evidence_excerpts.len(), 2);
        assert_eq!(digests[0].evidence_excerpts[0], "type mismatch");
    }

    #[test]
    fn epoch_parser_basic() {
        // 2026-06-16T10:00:00Z = 1781604000 (verified against `date`).
        assert_eq!(parse_epoch("2026-06-16T10:00:00Z"), Some(1_781_604_000));
        assert_eq!(parse_epoch("2026-06-16T10:00:00"), Some(1_781_604_000));
        assert_eq!(parse_epoch("garbage"), None);
    }
}
