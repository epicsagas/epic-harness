//! seesaw.rs — per-task regression gate (HarnessX §4.1 seesaw constraint)
//!
//! Before the evolution loop commits an edit, the seesaw gate verifies that
//! the edit does not regress any previously solved task. After each round, the
//! registry is updated with the new per-task scores.
//!
//! ## Critic fix (vs the original per-dimension draft)
//! The original scaffolding tracked `(task_id, dimension)` pairs. The HarnessX
//! paper (§6.6, §7.6) proves aggregate/per-dimension gating fails on
//! sub-threshold coupling. We therefore track a single best outcome score per
//! task and only catch *gross* regressions. This is a deliberate, documented
//! limitation — variant isolation (R6) is the real catastrophic-forgetting
//! defense; seesaw is the cheap coarse gate.
//!
//! ## "Task" definition in epic-harness
//! epic-harness has no pass@2 benchmark, so "task" = a digest segment
//! (`TaskDigest::task_id`, produced by the Digester from pipeline_id or idle
//! gaps). A task's score is the fraction of its observations without a
//! failure_category.

use std::collections::HashMap;
use std::io;

use crate::shared::evolution::{SolvedTaskRegistry, TaskDigest, TaskOutcome};

/// Default tolerance: a task regresses if its new score drops more than this
/// below its best. Generous, because per-task scores are noisy at small N.
pub const DEFAULT_TOLERANCE: f64 = 0.1;

/// Load the solved-task registry from disk, or return an empty one.
pub fn load_registry() -> SolvedTaskRegistry {
    load_registry_for(None)
}

/// Project-scoped load: reads `seesaw.json` from the requested project's dir
/// (None/empty = CWD project, the pre-existing behavior).
pub fn load_registry_for(project: Option<&str>) -> SolvedTaskRegistry {
    let path = crate::shared::paths::project_seesaw_path_for(project);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => SolvedTaskRegistry::default(),
    }
}

/// Persist the registry to disk.
pub fn save_registry(reg: &SolvedTaskRegistry) -> io::Result<()> {
    let path = crate::shared::paths::project_seesaw_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(reg).unwrap_or_else(|_| "{}".into());
    std::fs::write(&path, json)
}

/// Derive per-task scores from digests: success fraction per task.
pub fn scores_from_digests(digests: &[TaskDigest]) -> HashMap<String, f64> {
    digests
        .iter()
        .map(|d| {
            (
                d.task_id.clone(),
                outcome_score(&d.outcome, d.observation_count),
            )
        })
        .collect()
}

/// Check a candidate round's digests against the registry.
/// Returns the list of regressed task_ids (empty = passes the gate).
pub fn check(reg: &SolvedTaskRegistry, digests: &[TaskDigest], tolerance: f64) -> Vec<String> {
    let new_scores = scores_from_digests(digests);
    reg.check_seesaw(&new_scores, tolerance)
}

/// Convert a TaskOutcome to a 0.0–1.0 score.
///
/// `observation_count` is accepted for API symmetry with `TaskDigest` but not
/// currently used in the scoring (the success fraction from `total_steps`
/// already captures the outcome). Kept on the signature so future refinements
/// can weight by sample size without a breaking change.
pub fn outcome_score(outcome: &TaskOutcome, _observation_count: u64) -> f64 {
    match outcome {
        TaskOutcome::Success => 1.0,
        TaskOutcome::CompleteFailure => 0.0,
        TaskOutcome::PartialFailure {
            failed_steps,
            total_steps,
        } => {
            if *total_steps == 0 {
                return 0.0;
            }
            // Fraction of steps that succeeded. Clamped to [0,1].
            let succeeded = *total_steps as f64 - *failed_steps as f64;
            (succeeded / *total_steps as f64).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(id: &str, outcome: TaskOutcome, count: u64) -> TaskDigest {
        TaskDigest {
            task_id: id.into(),
            outcome,
            failure_categories: vec![],
            implicated_components: vec![],
            evidence_excerpts: vec![],
            tool_trajectory: vec![],
            iterations_seen: 0,
            token_estimate: 0,
            observation_count: count,
        }
    }

    #[test]
    fn outcome_score_extremes() {
        assert_eq!(outcome_score(&TaskOutcome::Success, 5), 1.0);
        assert_eq!(outcome_score(&TaskOutcome::CompleteFailure, 5), 0.0);
    }

    #[test]
    fn outcome_score_partial_in_between() {
        // 2 failed of 4 → 0.5 success fraction.
        let s = outcome_score(
            &TaskOutcome::PartialFailure {
                failed_steps: 2,
                total_steps: 4,
            },
            4,
        );
        assert!(
            (s - 0.5).abs() < 1e-9,
            "partial score should be 0.5, got {s}"
        );
    }

    #[test]
    fn check_flags_regression_below_tolerance() {
        let mut reg = SolvedTaskRegistry::default();
        // Task A previously solved perfectly (score 1.0).
        reg.update(&HashMap::from([("A".to_string(), 1.0)]));

        // New round: A dropped to 0.5 — regression (1.0 - 0.5 = 0.5 > tolerance 0.1).
        let digests = vec![digest(
            "A",
            TaskOutcome::PartialFailure {
                failed_steps: 2,
                total_steps: 4,
            },
            4,
        )];
        let regressed = check(&reg, &digests, DEFAULT_TOLERANCE);
        assert!(regressed.contains(&"A".to_string()));
    }

    #[test]
    fn check_passes_within_tolerance() {
        let mut reg = SolvedTaskRegistry::default();
        reg.update(&HashMap::from([("A".to_string(), 1.0)]));

        // A still fully solved — no regression.
        let digests = vec![digest("A", TaskOutcome::Success, 4)];
        let regressed = check(&reg, &digests, DEFAULT_TOLERANCE);
        assert!(regressed.is_empty());
    }

    #[test]
    fn new_tasks_do_not_regress() {
        let mut reg = SolvedTaskRegistry::default();
        reg.update(&HashMap::from([("A".to_string(), 1.0)]));

        // Brand-new task B not in registry — never a regression.
        let digests = vec![digest("B", TaskOutcome::CompleteFailure, 4)];
        let regressed = check(&reg, &digests, DEFAULT_TOLERANCE);
        assert!(regressed.is_empty());
    }

    #[test]
    fn update_only_improves_best() {
        let mut reg = SolvedTaskRegistry::default();
        reg.update(&HashMap::from([("A".to_string(), 0.8)]));
        // A worse score must not lower the best.
        reg.update(&HashMap::from([("A".to_string(), 0.5)]));
        assert_eq!(reg.solved.get("A"), Some(&0.8));
        // A better score raises it.
        reg.update(&HashMap::from([("A".to_string(), 0.95)]));
        assert_eq!(reg.solved.get("A"), Some(&0.95));
        assert_eq!(reg.total_solved, 1);
    }

    #[test]
    fn scores_from_digests_maps_each_task() {
        let digests = vec![
            digest("A", TaskOutcome::Success, 3),
            digest("B", TaskOutcome::CompleteFailure, 2),
        ];
        let scores = scores_from_digests(&digests);
        assert_eq!(scores.len(), 2);
        assert_eq!(scores.get("A"), Some(&1.0));
        assert_eq!(scores.get("B"), Some(&0.0));
    }
}
