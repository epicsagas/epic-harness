//! planner.rs — HarnessX-inspired adaptation landscape
//!
//! Builds an [`AdaptationLandscape`] from evolution history and current task
//! digests, the analog of HarnessX's Planner stage (paper §4.3). The landscape
//! is the primary defense against under-exploration (paper §4.2): by tracking
//! which edit types have been tried and which failures persist, it lets the
//! proposal builder inject exploration beyond trace-conditional local repair.
//!
//! Inputs:
//! - `history`: past `EvolutionRecord`s (what was attempted, what failed)
//! - `digests`: current session's per-task `TaskDigest`s
//! - `persistent_failure_window`: how many sessions back define "persistent"
//!
//! Output drives `inject_exploration_proposals` so the proposal builder can
//! surface untried edit categories when persistent failures remain unresolved.

use std::collections::HashMap;

use crate::shared::evolution::{
    AdaptationLandscape, AttemptedEdit, EditType, EvolutionRecord, PersistentFailure, TaskDigest,
};

/// Minimum number of sessions a failure category must appear in to count as
/// persistent (paper §4.3: "distinguish persistent failures from transient noise").
const PERSISTENT_MIN_SESSIONS: u32 = 2;

/// Build the adaptation landscape from history + current digests.
///
/// `history` should be oldest-first (records are appended chronologically).
pub fn build_landscape(
    history: &[EvolutionRecord],
    digests: &[TaskDigest],
    persistent_failure_window: u32,
) -> AdaptationLandscape {
    let persistent_failures = aggregate_persistent_failures(history, persistent_failure_window);
    let attempted_edits = collect_attempted_edits(history);
    let edit_type_coverage = compute_edit_coverage(&attempted_edits);
    let untried_edit_types = compute_untried(&edit_type_coverage);
    let component_failure_heatmap = build_component_heatmap(digests);

    AdaptationLandscape {
        persistent_failures,
        attempted_edits,
        edit_type_coverage,
        untried_edit_types,
        component_failure_heatmap,
    }
}

/// Decide whether the landscape recommends exploration proposals.
///
/// The paper's under-exploration pathology (§4.2) manifests as repeated local
/// edits while structural edit types go untried. We recommend exploration when
/// there is at least one unresolved persistent failure AND at least one untried
/// edit type — the exact condition where the engine is "plateauing on local
/// edits while structural options remain unexplored."
pub fn recommends_exploration(landscape: &AdaptationLandscape) -> bool {
    let has_unresolved = landscape
        .persistent_failures
        .iter()
        .any(|f| !f.resolved);
    has_unresolved && !landscape.untried_edit_types.is_empty()
}

/// Aggregate failure categories seen across multiple sessions into persistent failures.
fn aggregate_persistent_failures(
    history: &[EvolutionRecord],
    window: u32,
) -> Vec<PersistentFailure> {
    // failure_category → (first_seen, set of distinct session timestamps)
    let mut by_category: HashMap<String, (String, std::collections::HashSet<String>)> =
        HashMap::new();

    for rec in history {
        // Each EvolutionRecord is one session; count its failure categories.
        let session_ts = rec.timestamp.clone();
        // Failure categories appear in two places: error_patterns keys and
        // failure_patterns' pattern_type. Union both.
        for cat in rec.error_patterns.keys() {
            let entry = by_category
                .entry(cat.clone())
                .or_insert_with(|| (session_ts.clone(), Default::default()));
            entry.1.insert(session_ts.clone());
            if session_ts < entry.0 {
                entry.0 = session_ts.clone();
            }
        }
        for pat in &rec.failure_patterns {
            let cat = &pat.pattern_type;
            let entry = by_category
                .entry(cat.clone())
                .or_insert_with(|| (session_ts.clone(), Default::default()));
            entry.1.insert(session_ts.clone());
            if session_ts < entry.0 {
                entry.0 = session_ts.clone();
            }
        }
    }

    let min_sessions = window.max(PERSISTENT_MIN_SESSIONS);
    by_category
        .into_iter()
        .filter_map(|(cat, (first_seen, sessions))| {
            let sessions_seen = sessions.len() as u32;
            if sessions_seen >= min_sessions {
                Some(PersistentFailure {
                    failure_category: cat,
                    first_seen,
                    sessions_seen,
                    attempted_fixes: Vec::new(),
                    resolved: false,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Flatten history into per-edit records. Each evolution record represents one
/// edit attempt; its `edit_type` and a target summary form the AttemptedEdit.
fn collect_attempted_edits(history: &[EvolutionRecord]) -> Vec<AttemptedEdit> {
    history
        .iter()
        .map(|rec| AttemptedEdit {
            edit_type: rec.edit_type.as_str().to_string(),
            target: rec.analysis_summary.clone(),
            timestamp: rec.timestamp.clone(),
            // An edit that seeded skills and didn't roll back is treated as
            // successful; rolled-back edits are failed attempts.
            success: rec.skills_rolled_back == 0 && rec.skills_seeded > 0,
        })
        .collect()
}

/// Count occurrences of each edit type across attempted edits.
fn compute_edit_coverage(attempted: &[AttemptedEdit]) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for e in attempted {
        *counts.entry(e.edit_type.clone()).or_default() += 1;
    }
    counts
}

/// Edit types in the full taxonomy that have never been attempted.
fn compute_untried(coverage: &HashMap<String, u32>) -> Vec<String> {
    EditType::all()
        .iter()
        .map(|t| t.as_str().to_string())
        .filter(|name| !coverage.contains_key(name))
        .collect()
}

/// Component → failure rate from current digests. A component's failure rate is
/// the fraction of digests that implicate it AND have a non-success outcome.
fn build_component_heatmap(digests: &[TaskDigest]) -> HashMap<String, f64> {
    let mut implicated_in_failure: HashMap<String, u32> = HashMap::new();
    let mut total: u32 = 0;

    for d in digests {
        if matches!(d.outcome, crate::shared::evolution::TaskOutcome::Success) {
            continue;
        }
        total += 1;
        for comp in &d.implicated_components {
            *implicated_in_failure.entry(comp.clone()).or_default() += 1;
        }
    }

    if total == 0 {
        return HashMap::new();
    }
    implicated_in_failure
        .into_iter()
        .map(|(comp, count)| (comp, count as f64 / total as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::evolution::{DetectedPattern, TaskOutcome};
    use std::collections::HashMap;

    fn record(ts: &str, error_cats: &[&str], pattern_types: &[&str], edit: EditType) -> EvolutionRecord {
        let mut error_patterns = HashMap::new();
        for c in error_cats {
            error_patterns.insert((*c).to_string(), 1u64);
        }
        EvolutionRecord {
            timestamp: ts.into(),
            observations: 10,
            success_rate: 0.5,
            avg_score: 0.5,
            error_patterns,
            failure_patterns: pattern_types
                .iter()
                .map(|p| DetectedPattern {
                    pattern_type: (*p).into(),
                    description: String::new(),
                    count: 1,
                    involved_files: vec![],
                    suggested_remediation: String::new(),
                    implicated_components: vec![],
                })
                .collect(),
            skills_seeded: 1,
            skills_rolled_back: 0,
            total_evolved: 1,
            analysis_summary: "seeded evo-skill".into(),
            edit_type: edit,
        }
    }

    #[test]
    fn empty_history_yields_empty_landscape_but_all_untried() {
        let landscape = build_landscape(&[], &[], 2);
        assert!(landscape.persistent_failures.is_empty());
        assert!(landscape.attempted_edits.is_empty());
        // Every edit type is untried when nothing has happened.
        assert!(!landscape.untried_edit_types.is_empty());
    }

    #[test]
    fn persistent_failure_detected_across_sessions() {
        // type_error in 3 distinct sessions → persistent.
        let history = vec![
            record("2026-06-01T00:00:00Z", &["type_error"], &[], EditType::AddSkill),
            record("2026-06-02T00:00:00Z", &["type_error"], &[], EditType::AddSkill),
            record("2026-06-03T00:00:00Z", &["type_error"], &[], EditType::AddSkill),
        ];
        let landscape = build_landscape(&history, &[], 2);
        let cats: Vec<&str> = landscape.persistent_failures.iter().map(|f| f.failure_category.as_str()).collect();
        assert!(cats.contains(&"type_error"));
        let pf = landscape.persistent_failures.iter().find(|f| f.failure_category == "type_error").unwrap();
        assert_eq!(pf.sessions_seen, 3);
        assert!(!pf.resolved);
    }

    #[test]
    fn single_session_failure_is_not_persistent() {
        let history = vec![record("2026-06-01T00:00:00Z", &["type_error"], &[], EditType::AddSkill)];
        let landscape = build_landscape(&history, &[], 2);
        assert!(landscape.persistent_failures.is_empty());
    }

    #[test]
    fn edit_coverage_tracks_attempted_types() {
        let history = vec![
            record("2026-06-01T00:00:00Z", &[], &[], EditType::AddSkill),
            record("2026-06-02T00:00:00Z", &[], &[], EditType::AddSkill),
            record("2026-06-03T00:00:00Z", &[], &[], EditType::ModifySkill),
        ];
        let landscape = build_landscape(&history, &[], 2);
        assert_eq!(landscape.edit_type_coverage.get("add_skill"), Some(&2));
        assert_eq!(landscape.edit_type_coverage.get("modify_skill"), Some(&1));
        // add_skill and modify_skill are tried; the rest untried.
        assert!(landscape.untried_edit_types.contains(&"modify_config".to_string()));
        assert!(!landscape.untried_edit_types.contains(&"add_skill".to_string()));
    }

    #[test]
    fn recommends_exploration_when_unresolved_and_untried() {
        let history = vec![
            record("2026-06-01T00:00:00Z", &["type_error"], &[], EditType::AddSkill),
            record("2026-06-02T00:00:00Z", &["type_error"], &[], EditType::AddSkill),
        ];
        let landscape = build_landscape(&history, &[], 2);
        // Persistent failure + untried structural types → explore.
        assert!(recommends_exploration(&landscape));
    }

    #[test]
    fn no_exploration_when_no_persistent_failures() {
        let history = vec![record("2026-06-01T00:00:00Z", &[], &[], EditType::AddSkill)];
        let landscape = build_landscape(&history, &[], 2);
        assert!(!recommends_exploration(&landscape));
    }

    #[test]
    fn component_heatmap_from_digests() {
        let d = TaskDigest {
            task_id: "t1".into(),
            outcome: TaskOutcome::CompleteFailure,
            failure_categories: vec![("type_error".into(), 1)],
            implicated_components: vec!["auth".into(), "db".into()],
            evidence_excerpts: vec![],
            tool_trajectory: vec![],
            iterations_seen: 0,
            token_estimate: 0,
            observation_count: 1,
        };
        let landscape = build_landscape(&[], &[d], 2);
        assert_eq!(landscape.component_failure_heatmap.get("auth"), Some(&1.0));
        assert_eq!(landscape.component_failure_heatmap.get("db"), Some(&1.0));
    }

    #[test]
    fn success_digests_excluded_from_heatmap() {
        let d = TaskDigest {
            task_id: "t1".into(),
            outcome: TaskOutcome::Success,
            failure_categories: vec![],
            implicated_components: vec!["auth".into()],
            evidence_excerpts: vec![],
            tool_trajectory: vec![],
            iterations_seen: 0,
            token_estimate: 0,
            observation_count: 1,
        };
        let landscape = build_landscape(&[], &[d], 2);
        assert!(landscape.component_failure_heatmap.is_empty());
    }
}
