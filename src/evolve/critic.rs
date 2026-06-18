#![allow(dead_code)]

//! critic.rs — HarnessX Critic layer (Tier 2.1)
//!
//! Defends against reward hacking (paper §4.3: "the Critic defends against
//! reward hacking"). In HarnessX the Critic is an LLM that compares each
//! candidate's change manifest against trace evidence and may issue a revision
//! request. epic-harness forbids external LLM calls from production code, so
//! the in-loop Critic here is **deterministic**: it checks whether an edit's
//! predicted impact is consistent with the observed dimension deltas, and it
//! gates seeding when reward hacking is suspected.
//!
//! The LLM version exists only as a registry prompt template
//! (`registry/skills/_critic/SKILL.md`) a meta-agent or human runs out-of-band.
//!
//! ## Relationship to the other gates
//! - Seesaw (R5) → catastrophic forgetting (per-task regression)
//! - Reward-hacking detection (2.2) → reward signal gaming
//! - Critic (2.1) → consumes the reward-hacking flag AND verifies manifests
//!
//! The Critic is the EARLIER, coarser gate: when `reward_hacking_suspected` is
//! true it suppresses ALL new seeding for the round (matching the seesaw block
//! pattern), so a reward-hacking epoch cannot ship skills that game the metric.

use crate::evolve::edits::EditManifest;
use crate::shared::evolution::{Metrics, TaskDigest};

/// The Critic's verdict on a candidate edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticVerdict {
    /// Manifest is consistent with evidence; ship.
    Approve,
    /// Manifest claims an effect the evidence does not support; warn but allow.
    Warn(String),
    /// Manifest contradicts the evidence (e.g. claims quality lift while
    /// quality is regressing); reject.
    Reject(String),
}

/// A deterministic, in-loop Critic. No LLM. Pure function over the manifest +
/// observed evidence (digests) + metrics history.
pub struct Critic;

impl Critic {
    /// Verify an edit's manifest against observed trace evidence.
    ///
    /// The falsifiability contract (paper Table 9): the manifest's
    /// `predicted_impact` must be consistent with the observed dimension
    /// deltas. Today this checks the most common contradiction — a manifest
    /// claiming a quality lift while the recent history shows output_quality
    /// regressing.
    pub fn verify_against_evidence(
        manifest: &EditManifest,
        _digests: &[TaskDigest],
        metrics: &Metrics,
    ) -> CriticVerdict {
        // The reward-hacking detector already computed the dimension slopes.
        // If reward hacking is suspected, any edit claiming a quality
        // improvement is contradicted by the evidence (quality is falling).
        if metrics.reward_hacking_suspected
            && manifest
                .predicted_impact
                .to_lowercase()
                .contains("avg_score")
        {
            return CriticVerdict::Reject(format!(
                "Manifest claims score lift but reward hacking is suspected (quality falling while cost proxy rises): {}",
                manifest.target
            ));
        }
        CriticVerdict::Approve
    }

    /// Should new skill seeding be blocked this round?
    ///
    /// True when reward hacking is suspected — the coarser, earlier gate that
    /// pairs with the seesaw regression gate. Both suppress seeding; the Critic
    /// runs first so the seesaw block can short-circuit when the Critic already
    /// blocked (avoiding doubled hints / duplicate rejected-buffer entries).
    pub fn should_block_seeding(metrics: &Metrics) -> bool {
        metrics.reward_hacking_suspected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::evolution::{EditType, EpochClass, SessionScoreEntry, SkillAttribution};
    use crate::shared::scoring::ScoreDimensions;
    use std::collections::HashMap;

    fn metrics_with_reward_hacking(suspected: bool, n: usize) -> Metrics {
        // Build a history of n entries. When suspected, shape it as reward
        // hacking (quality falling, cost rising).
        let mut score_history = Vec::new();
        for i in 0..n {
            let frac = i as f64 / n.max(1) as f64;
            let (q, c) = if suspected {
                (0.9 - 0.4 * frac, 0.5 + 0.4 * frac)
            } else {
                (0.7 + 0.1 * frac, 0.7 + 0.1 * frac)
            };
            score_history.push(SessionScoreEntry {
                timestamp: format!("2026-06-{:02}T00:00:00Z", 10 + i),
                success_rate: 0.8,
                avg_score: 0.7,
                observations: 10,
                dimension_averages: ScoreDimensions {
                    tool_success: 0.8,
                    output_quality: q,
                    execution_cost: c,
                },
            });
        }
        Metrics {
            total_sessions: n as u64,
            avg_success_rate: 0.8,
            total_evolved_skills: 1,
            last_session: Some("2026-06-16".into()),
            score_history,
            best_score: Some(0.9),
            best_session: "2026-06-10".into(),
            trend: "stable".into(),
            stagnation_count: 0,
            skill_attribution: HashMap::<String, SkillAttribution>::new(),
            epoch_class: Some(EpochClass::StableSuccess),
            last_error_context: None,
            reward_hacking_suspected: suspected,
        }
    }

    fn manifest(predicted: &str) -> EditManifest {
        EditManifest {
            edit_type: EditType::AddSkill,
            target: "evo-test".into(),
            intended_effect: "test".into(),
            predicted_impact: predicted.into(),
        }
    }

    #[test]
    fn approve_when_no_reward_hacking() {
        let m = metrics_with_reward_hacking(false, 6);
        let mani = manifest("Lift avg_score_with for evo-test");
        assert_eq!(
            Critic::verify_against_evidence(&mani, &[], &m),
            CriticVerdict::Approve
        );
    }

    #[test]
    fn reject_quality_claim_under_reward_hacking() {
        let m = metrics_with_reward_hacking(true, 6);
        let mani = manifest("Lift avg_score_with for evo-test");
        match Critic::verify_against_evidence(&mani, &[], &m) {
            CriticVerdict::Reject(_) => {}
            other => panic!("expected Reject, got {other:?}"),
        }
    }

    #[test]
    fn non_score_manifest_passes_under_reward_hacking() {
        // A manifest not claiming a score lift (e.g. a guard rule) is not
        // contradicted by reward hacking.
        let m = metrics_with_reward_hacking(true, 6);
        let mani = manifest("Prevent observed dangerous pattern");
        assert_eq!(
            Critic::verify_against_evidence(&mani, &[], &m),
            CriticVerdict::Approve
        );
    }

    #[test]
    fn block_seeding_when_reward_hacking_suspected() {
        let m = metrics_with_reward_hacking(true, 6);
        assert!(Critic::should_block_seeding(&m));
    }

    #[test]
    fn do_not_block_seeding_when_clean() {
        let m = metrics_with_reward_hacking(false, 6);
        assert!(!Critic::should_block_seeding(&m));
    }
}
