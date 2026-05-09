use crate::config::CONFIG;
use crate::shared::{
    evolution::*, helpers::*, paths::*,
};

/// Clamp avg_score to a finite f64 so it serialises as a valid JSON number.
/// NaN and ±Infinity are both invalid JSON; replace them with 0.0.
pub fn safe_avg_score(score: f64) -> f64 {
    if score.is_finite() { score } else { 0.0 }
}

/// Compute the rolling average of `avg_score` from the last `window` entries
/// in `score_history`. Returns 0.0 when history is empty.
pub fn compute_rolling_avg(history: &[SessionScoreEntry], window: usize) -> f64 {
    if history.is_empty() || window == 0 {
        return 0.0;
    }
    let start = history.len().saturating_sub(window);
    let slice = &history[start..];
    slice.iter().map(|e| e.avg_score).sum::<f64>() / slice.len() as f64
}

pub fn check_stagnation(metrics: &mut Metrics, current_score: f64) -> (bool, bool, u64) {
    // Returns (should_rollback, improved, rolled_back_count)
    if metrics.total_sessions == 0 || metrics.best_score.is_none() {
        // First session or genuinely uninitialized — set best_score and treat as improved.
        metrics.best_score = Some(current_score);
        return (false, true, 0);
    }

    // Use rolling 3-session average instead of absolute best_score for the
    // improvement check. This prevents nearly every session from being flagged
    // as stagnant when the all-time best was an outlier.
    // Guard: if score_history is empty (e.g. after migration), fall back to
    // best_score comparison to avoid treating any positive score as improvement.
    let rolling_avg = if metrics.score_history.is_empty() {
        metrics.best_score.unwrap_or(0.0)
    } else {
        compute_rolling_avg(&metrics.score_history, CONFIG.evolution.stagnation_rolling_window)
    };
    let improvement = current_score - rolling_avg;
    if improvement >= CONFIG.evolution.improvement_threshold {
        // Improved over rolling average! Backup evolved skills
        let evolved = evolved_dir();
        let backup = evolved_backup_dir();
        if evolved.is_dir() {
            rm_dir(&backup);
            copy_dir(&evolved, &backup);
        }
        return (false, true, 0);
    }

    // No improvement
    metrics.stagnation_count += 1;
    if metrics.stagnation_count >= CONFIG.evolution.stagnation_limit {
        // Rollback decision still uses absolute best_score
        let best = metrics.best_score.unwrap_or(0.0);
        let degradation = best - current_score;
        if degradation > CONFIG.evolution.rollback_degradation || best < CONFIG.evolution.rollback_best_floor {
            let backup = evolved_backup_dir();
            if backup.is_dir() {
                let evolved = evolved_dir();
                let before_count = list_dirs(&evolved).len() as u64;
                rm_dir(&evolved);
                copy_dir(&backup, &evolved);
                metrics.stagnation_count = 0;
                hint(
                    "reflect",
                    &format!(
                        "Stagnation detected ({} sessions). Rolled back evolved skills.",
                        CONFIG.evolution.stagnation_limit
                    ),
                );
                return (true, false, before_count);
            }
        }
    }

    (false, false, 0)
}

#[cfg(test)]
fn seeding_scope(avg_score: f64) -> &'static str {
    if avg_score >= CONFIG.pattern.graduated_scope_skip {
        "skip"
    } else if avg_score >= CONFIG.pattern.graduated_scope_moderate {
        "moderate"
    } else {
        "full"
    }
}

pub fn compute_trend(history: &[SessionScoreEntry]) -> &'static str {
    let start = history.len().saturating_sub(5);
    let recent = &history[start..];
    if recent.len() < 2 {
        return "stable";
    }
    let n = recent.len() as f64;
    let (mut sx, mut sy, mut sxy, mut sxx) = (0.0, 0.0, 0.0, 0.0);
    for (i, e) in recent.iter().enumerate() {
        let x = i as f64;
        sx += x;
        sy += e.avg_score;
        sxy += x * e.avg_score;
        sxx += x * x;
    }
    let denom = n * sxx - sx * sx;
    if denom.abs() < f64::EPSILON {
        return "stable";
    }
    let slope = (n * sxy - sx * sy) / denom;
    if slope > 0.01 {
        "improving"
    } else if slope < -0.01 {
        "declining"
    } else {
        "stable"
    }
}

pub fn update_skill_attribution(
    metrics: &mut Metrics,
    analysis: &crate::shared::evolution::SessionAnalysis,
    evolved_skills: &[String],
) {
    for skill in evolved_skills {
        let attr = metrics
            .skill_attribution
            .entry(skill.clone())
            .or_insert(SkillAttribution {
                skill_name: skill.clone(),
                sessions_active: 0,
                avg_score_with: 0.0,
                avg_score_without: 0.0,
                first_seen: now_iso(),
            });
        attr.sessions_active += 1;
        attr.avg_score_with = super::analysis::round3(
            ((attr.avg_score_with * (attr.sessions_active - 1) as f64) + analysis.avg_score)
                / attr.sessions_active as f64,
        );
    }

    let total_sessions = metrics.total_sessions + 1;
    // Sum of all composite avg_scores across all sessions (history + current).
    // Use score_history (avg_score field, not avg_success_rate) for historical sessions.
    let all_scores_sum = metrics
        .score_history
        .iter()
        .map(|e| e.avg_score)
        .sum::<f64>()
        + analysis.avg_score;
    for attr in metrics.skill_attribution.values_mut() {
        let without = total_sessions.saturating_sub(attr.sessions_active);
        if without > 0 {
            attr.avg_score_without = super::analysis::round3(
                (all_scores_sum - (attr.avg_score_with * attr.sessions_active as f64))
                    / without as f64,
            );
        }
    }

    metrics
        .skill_attribution
        .retain(|name, _| evolved_skills.contains(name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::evolution::default_metrics;
    use crate::shared::scoring::ScoreDimensions;

    #[test]
    fn stagnation_first_session() {
        let mut metrics = default_metrics();
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.8);
        assert!(!rollback);
        assert!(improved);
    }

    #[test]
    fn stagnation_improvement_resets() {
        let mut metrics = default_metrics();
        metrics.total_sessions = 5;
        metrics.best_score = Some(0.7);
        metrics.stagnation_count = 2;
        // Populate last 3 score_history entries with avg_score 0.70 so the
        // rolling average is 0.70. current_score=0.80 must beat that by >= threshold.
        for _ in 0..3 {
            metrics.score_history.push(SessionScoreEntry {
                timestamp: "2026-04-09T00:00:00Z".into(),
                success_rate: 0.70,
                avg_score: 0.70,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            });
        }
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.80);
        assert!(!rollback);
        assert!(improved);
    }

    #[test]
    fn stagnation_increments_on_no_improvement() {
        let mut metrics = default_metrics();
        metrics.total_sessions = 5;
        metrics.best_score = Some(0.8);
        metrics.stagnation_count = 0;
        // Rolling avg from last 3 entries = 0.80. current_score=0.78 is below
        // that, so stagnation_count should increment.
        for _ in 0..3 {
            metrics.score_history.push(SessionScoreEntry {
                timestamp: "2026-04-09T00:00:00Z".into(),
                success_rate: 0.80,
                avg_score: 0.80,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            });
        }
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.78);
        assert!(!rollback);
        assert!(!improved);
        assert_eq!(metrics.stagnation_count, 1);
    }

    #[test]
    fn stagnation_zero_score_is_not_sentinel() {
        let mut metrics = default_metrics();
        metrics.total_sessions = 3;
        metrics.best_score = Some(0.0);
        metrics.stagnation_count = 0;
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.0);
        assert!(!rollback);
        assert!(!improved);
        assert_eq!(
            metrics.stagnation_count, 1,
            "stagnation_count must increment for score=0.0"
        );
    }

    #[test]
    fn stagnation_first_session_zero_score_initializes_best() {
        let mut metrics = default_metrics();
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.0);
        assert!(!rollback);
        assert!(improved);
        assert_eq!(metrics.best_score, Some(0.0));
    }

    #[test]
    fn trend_stable_with_one_entry() {
        let history = vec![SessionScoreEntry {
            timestamp: "2026-04-09".into(),
            success_rate: 0.8,
            avg_score: 0.8,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }];
        assert_eq!(compute_trend(&history), "stable");
    }

    #[test]
    fn trend_improving() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.5 + i as f64 * 0.1,
                avg_score: 0.5 + i as f64 * 0.1,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(compute_trend(&history), "improving");
    }

    #[test]
    fn trend_declining() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.9 - i as f64 * 0.1,
                avg_score: 0.9 - i as f64 * 0.1,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(compute_trend(&history), "declining");
    }

    #[test]
    fn trend_stable_flat() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.75,
                avg_score: 0.75,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(compute_trend(&history), "stable");
    }

    #[test]
    fn trend_no_nan_with_zero_denominator() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.80,
                avg_score: 0.80,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        let trend = compute_trend(&history);
        assert!(
            trend == "stable" || trend == "improving" || trend == "declining",
            "trend must be a valid value, got: {:?}",
            trend
        );
        assert_eq!(trend, "stable");
    }

    #[test]
    fn safe_avg_score_clamps_nan_to_zero() {
        assert_eq!(safe_avg_score(f64::NAN), 0.0);
    }

    #[test]
    fn safe_avg_score_clamps_pos_inf_to_zero() {
        assert_eq!(safe_avg_score(f64::INFINITY), 0.0);
    }

    #[test]
    fn safe_avg_score_clamps_neg_inf_to_zero() {
        assert_eq!(safe_avg_score(f64::NEG_INFINITY), 0.0);
    }

    #[test]
    fn safe_avg_score_passes_through_finite_value() {
        assert_eq!(safe_avg_score(0.75), 0.75);
    }

    #[test]
    fn safe_avg_score_passes_through_zero() {
        assert_eq!(safe_avg_score(0.0), 0.0);
    }

    #[test]
    fn seeding_scope_skip_when_excellent() {
        assert_eq!(seeding_scope(0.95), "skip");
        assert_eq!(seeding_scope(0.97), "skip");
        assert_eq!(seeding_scope(1.00), "skip");
    }

    #[test]
    fn seeding_scope_moderate_in_middle_range() {
        assert_eq!(seeding_scope(0.80), "moderate");
        assert_eq!(seeding_scope(0.90), "moderate");
        assert_eq!(seeding_scope(0.94), "moderate");
    }

    #[test]
    fn seeding_scope_full_when_low_score() {
        assert_eq!(seeding_scope(0.79), "full");
        assert_eq!(seeding_scope(0.50), "full");
        assert_eq!(seeding_scope(0.0), "full");
    }

    #[test]
    fn skill_attribution_uses_avg_score_not_success_rate() {
        let mut metrics = default_metrics();
        metrics.avg_success_rate = 0.99;
        metrics.score_history.push(SessionScoreEntry {
            timestamp: "2026-04-09T00:00:00Z".into(),
            success_rate: 0.99,
            avg_score: 0.60,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        });
        metrics.total_sessions = 1;

        let analysis = crate::shared::evolution::SessionAnalysis {
            avg_score: 0.70,
            ..Default::default()
        };
        let evolved = vec!["evo-test".to_string()];
        update_skill_attribution(&mut metrics, &analysis, &evolved);

        let attr = metrics
            .skill_attribution
            .get("evo-test")
            .expect("attribution entry missing");
        assert!(
            (attr.avg_score_with - 0.70).abs() < 0.01,
            "avg_score_with should be 0.70, got {}",
            attr.avg_score_with
        );
        assert!(
            (attr.avg_score_without - 0.60).abs() < 0.05,
            "avg_score_without should be ~0.60 (from score_history avg_score), got {}",
            attr.avg_score_without
        );
    }
}
