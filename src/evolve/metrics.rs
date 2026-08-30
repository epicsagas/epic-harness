use crate::config::CONFIG;
use crate::shared::{evolution::*, helpers::*, paths::*};

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
        compute_rolling_avg(
            &metrics.score_history,
            CONFIG.evolution.stagnation_rolling_window,
        )
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
        if degradation > CONFIG.evolution.rollback_degradation
            || best < CONFIG.evolution.rollback_best_floor
        {
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

/// Slope threshold for classifying a trend as improving or regressing.
const EPOCH_SLOPE_THRESHOLD: f64 = 0.02;
/// Average score below this is classified as persistent failure.
const EPOCH_PERSISTENT_FAILURE_THRESHOLD: f64 = 0.4;
/// Average score above this (with low variance) is stable success.
const EPOCH_STABLE_SUCCESS_THRESHOLD: f64 = 0.8;
/// Variance below this indicates stable scores.
const EPOCH_VARIANCE_THRESHOLD: f64 = 0.01;
/// Default score threshold for classifying as stable success when trend is flat.
const EPOCH_DEFAULT_THRESHOLD: f64 = 0.6;

/// SkillOpt Slow/Meta Update: classify the current epoch from score history.
///
/// Uses the last 5 sessions' avg_score to determine epoch class:
/// - **Improving**: recent scores consistently rising
/// - **Regressing**: recent scores consistently falling
/// - **PersistentFailure**: recent average below EPOCH_PERSISTENT_FAILURE_THRESHOLD
/// - **StableSuccess**: recent average above EPOCH_STABLE_SUCCESS_THRESHOLD with minimal variance
pub fn classify_epoch(history: &[SessionScoreEntry]) -> EpochClass {
    let window = 5;
    let start = history.len().saturating_sub(window);
    let recent = &history[start..];
    if recent.len() < 2 {
        return EpochClass::InsufficientData;
    }

    let avg: f64 = recent.iter().map(|e| e.avg_score).sum::<f64>() / recent.len() as f64;

    // Persistent failure: average score below threshold
    if avg < EPOCH_PERSISTENT_FAILURE_THRESHOLD {
        return EpochClass::PersistentFailure;
    }

    // Check trend direction via linear regression slope
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
    let slope = if denom.abs() > f64::EPSILON {
        (n * sxy - sx * sy) / denom
    } else {
        0.0
    };

    if slope > EPOCH_SLOPE_THRESHOLD {
        return EpochClass::Improving;
    }
    if slope < -EPOCH_SLOPE_THRESHOLD {
        return EpochClass::Regressing;
    }

    // Stable success: high average with low variance
    if avg > EPOCH_STABLE_SUCCESS_THRESHOLD {
        let variance: f64 = recent
            .iter()
            .map(|e| (e.avg_score - avg).powi(2))
            .sum::<f64>()
            / recent.len() as f64;
        if variance < EPOCH_VARIANCE_THRESHOLD {
            return EpochClass::StableSuccess;
        }
    }

    // Default: if not clearly anything, classify based on score level
    if avg > EPOCH_DEFAULT_THRESHOLD {
        EpochClass::StableSuccess
    } else {
        EpochClass::PersistentFailure
    }
}

/// Least-squares slope of a scalar series over x = [0, 1, .. n-1].
/// Returns 0.0 when the denominator is degenerate (all-equal x) or when the
/// series contains any non-finite value, so callers never see NaN.
fn series_slope(values: &[f64]) -> f64 {
    if values.iter().any(|v| !v.is_finite()) {
        return 0.0;
    }
    let n = values.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let (mut sx, mut sy, mut sxy, mut sxx) = (0.0, 0.0, 0.0, 0.0);
    for (i, y) in values.iter().enumerate() {
        let x = i as f64;
        sx += x;
        sy += *y;
        sxy += x * *y;
        sxx += x * x;
    }
    let denom = n * sxx - sx * sx;
    if !denom.is_finite() || denom.abs() < f64::EPSILON {
        return 0.0;
    }
    (n * sxy - sx * sy) / denom
}

/// HarnessX Tier 2.2: reward-hacking detection.
///
/// Examines the last `evolution.reward_hacking_window` `SessionScoreEntry`s in
/// `score_history` and computes the least-squares slope of `output_quality` and
/// of `execution_cost` (each taken from `dimension_averages`).
///
/// **Semantics (critical):** in this codebase `execution_cost` is an
/// *efficiency proxy* where **HIGHER = BETTER**. Reward hacking is therefore
/// flagged when the *efficiency proxy is rising* (`execution_cost` slope >
/// `reward_hacking_cost_rise`) **while the quality proxy is falling**
/// (`output_quality` slope < `-(reward_hacking_quality_drop)`). The agent is
/// gaming the cheap efficiency dimension while real output quality regresses.
///
/// Returns `false` when there are fewer than `reward_hacking_window` entries
/// (insufficient data), and guards against NaN in all derived slopes.
pub fn detect_reward_hacking(metrics: &Metrics) -> bool {
    let window = CONFIG.evolution.reward_hacking_window;
    if window == 0 || metrics.score_history.len() < window {
        return false;
    }
    let start = metrics.score_history.len().saturating_sub(window);
    let recent = &metrics.score_history[start..];

    let quality: Vec<f64> = recent
        .iter()
        .map(|e| e.dimension_averages.output_quality)
        .collect();
    let cost: Vec<f64> = recent
        .iter()
        .map(|e| e.dimension_averages.execution_cost)
        .collect();

    let quality_slope = series_slope(&quality);
    let cost_slope = series_slope(&cost);

    if !quality_slope.is_finite() || !cost_slope.is_finite() {
        return false;
    }

    quality_slope < -CONFIG.evolution.reward_hacking_quality_drop
        && cost_slope > CONFIG.evolution.reward_hacking_cost_rise
}

/// Stable FNV-1a over `skill|date` — deterministic across processes and
/// builds, so resume (session start) and reflect (session end) independently
/// compute the same holdout assignment for the day.
fn holdout_hash(skill: &str, date: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in skill.as_bytes().iter().chain(b"|").chain(date.as_bytes()) {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Whether `skill` is assigned to the holdout arm on `date` (YYYY-MM-DD).
/// Pure date-keyed rotation: ~1/modulus of days are holdout days.
pub fn is_holdout_day(skill: &str, date: &str) -> bool {
    let modulus = CONFIG.evolution.attribution_holdout_modulus;
    modulus > 0 && holdout_hash(skill, date).is_multiple_of(modulus)
}

/// Split the evolved-skill list into (active, holdout) for `date`.
///
/// A skill rotates into holdout only while it is still under evaluation
/// (active + holdout sessions < attribution_eval_sessions). Once the A/B
/// window is spent the verdict is settled: survivors stay active every
/// session, losers are evicted by `update_skill_attribution`.
pub fn partition_holdout(
    skills: &[String],
    metrics: &Metrics,
    date: &str,
) -> (Vec<String>, Vec<String>) {
    let eval_window = CONFIG.evolution.attribution_eval_sessions;
    let mut active = Vec::new();
    let mut holdout = Vec::new();
    for skill in skills {
        let under_evaluation = metrics
            .skill_attribution
            .get(skill)
            .map(|a| a.sessions_active + a.sessions_holdout < eval_window)
            .unwrap_or(true);
        if under_evaluation && is_holdout_day(skill, date) {
            holdout.push(skill.clone());
        } else {
            active.push(skill.clone());
        }
    }
    (active, holdout)
}

/// Update per-skill A/B attribution from one session.
///
/// `active` are skills that were exposed (injected at session start);
/// `holdout` are skills deliberately withheld this session. The session score
/// updates avg_score_with for the former and avg_score_without for the latter
/// — a genuine counterfactual. The legacy scheme (credit every skill on disk,
/// derive "without" from pre-creation history) was confounded by regression
/// to the mean: skills are created after bad sessions, so any recovery looked
/// like skill effectiveness.
pub fn update_skill_attribution(
    metrics: &mut Metrics,
    analysis: &crate::shared::evolution::SessionAnalysis,
    active: &[String],
    holdout: &[String],
) {
    for skill in active {
        let attr = entry_for(metrics, skill);
        attr.sessions_active += 1;
        attr.avg_score_with = super::analysis::round3(
            ((attr.avg_score_with * (attr.sessions_active - 1) as f64) + analysis.avg_score)
                / attr.sessions_active as f64,
        );
    }

    for skill in holdout {
        let attr = entry_for(metrics, skill);
        attr.sessions_holdout += 1;
        // Running average over holdout sessions only. On the first holdout
        // sample (count 1) this replaces any legacy derived value outright.
        attr.avg_score_without = super::analysis::round3(
            ((attr.avg_score_without * (attr.sessions_holdout - 1) as f64) + analysis.avg_score)
                / attr.sessions_holdout as f64,
        );
    }

    // Attribution verdicts are REPORTED, not enforced: a skill whose
    // avg_score_with trails its holdout avg_score_without beyond the eviction
    // thresholds is flagged in dashboards (verdict is derivable from the
    // skill_attribution state below — with/without averages and session
    // counts) but is no longer deleted from disk nor registered in the
    // rejected buffer. 3-vs-2 samples with a 0.02 delta carry less evidence
    // than session-to-session difficulty variance, so statistical eviction
    // was destroying healthy skills on easy-holdout-day noise.
    // The settled-verdict path (attribution_eval_sessions) is unchanged:
    // after the eval window the skill simply stays active every session.

    metrics
        .skill_attribution
        .retain(|name, _| active.contains(name) || holdout.contains(name));
}

fn entry_for<'m>(metrics: &'m mut Metrics, skill: &str) -> &'m mut SkillAttribution {
    metrics
        .skill_attribution
        .entry(skill.to_string())
        .or_insert_with(|| SkillAttribution {
            skill_name: skill.to_string(),
            sessions_active: 0,
            avg_score_with: 0.0,
            avg_score_without: 0.0,
            first_seen: now_iso(),
            sessions_holdout: 0,
        })
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
    fn skill_attribution_active_updates_with_only() {
        let mut metrics = default_metrics();
        let analysis = crate::shared::evolution::SessionAnalysis {
            avg_score: 0.70,
            ..Default::default()
        };
        let active = vec!["evo-test".to_string()];
        update_skill_attribution(&mut metrics, &analysis, &active, &[]);

        let attr = metrics
            .skill_attribution
            .get("evo-test")
            .expect("attribution entry missing");
        assert_eq!(attr.sessions_active, 1);
        assert_eq!(attr.sessions_holdout, 0);
        assert!(
            (attr.avg_score_with - 0.70).abs() < 0.01,
            "avg_score_with should be 0.70, got {}",
            attr.avg_score_with
        );
        assert_eq!(
            attr.avg_score_without, 0.0,
            "no holdout sample yet — avg_score_without must stay untouched"
        );
    }

    #[test]
    fn skill_attribution_holdout_updates_without_only() {
        let mut metrics = default_metrics();
        let analysis = crate::shared::evolution::SessionAnalysis {
            avg_score: 0.55,
            ..Default::default()
        };
        let holdout = vec!["evo-test".to_string()];
        update_skill_attribution(&mut metrics, &analysis, &[], &holdout);

        let attr = metrics.skill_attribution.get("evo-test").unwrap();
        assert_eq!(attr.sessions_active, 0);
        assert_eq!(attr.sessions_holdout, 1);
        assert!((attr.avg_score_without - 0.55).abs() < 0.01);
        assert_eq!(attr.avg_score_with, 0.0);
    }

    #[test]
    fn skill_attribution_first_holdout_replaces_legacy_without() {
        // Legacy metrics carry a derived avg_score_without with no holdout
        // samples. The first genuine holdout session must replace it, not
        // average into it.
        let mut metrics = default_metrics();
        metrics.skill_attribution.insert(
            "evo-legacy".into(),
            SkillAttribution {
                skill_name: "evo-legacy".into(),
                sessions_active: 5,
                avg_score_with: 0.80,
                avg_score_without: 0.33, // legacy derived value
                first_seen: "2026-06-01T00:00:00Z".into(),
                sessions_holdout: 0,
            },
        );
        let analysis = crate::shared::evolution::SessionAnalysis {
            avg_score: 0.90,
            ..Default::default()
        };
        update_skill_attribution(&mut metrics, &analysis, &[], &["evo-legacy".to_string()]);

        let attr = metrics.skill_attribution.get("evo-legacy").unwrap();
        assert!(
            (attr.avg_score_without - 0.90).abs() < 0.01,
            "first holdout sample must replace the legacy value, got {}",
            attr.avg_score_without
        );
    }

    #[test]
    fn skill_attribution_eviction_requires_holdout_sample() {
        // with < without - delta, enough active sessions, but NO holdout
        // sessions → must not evict (legacy noise is not evidence).
        let mut metrics = default_metrics();
        metrics.skill_attribution.insert(
            "evo-suspect".into(),
            SkillAttribution {
                skill_name: "evo-suspect".into(),
                sessions_active: 5,
                avg_score_with: 0.40,
                avg_score_without: 0.90,
                first_seen: "2026-06-01T00:00:00Z".into(),
                sessions_holdout: 0,
            },
        );
        let analysis = crate::shared::evolution::SessionAnalysis {
            avg_score: 0.40,
            ..Default::default()
        };
        update_skill_attribution(&mut metrics, &analysis, &["evo-suspect".to_string()], &[]);
        assert!(
            metrics.skill_attribution.contains_key("evo-suspect"),
            "eviction without a holdout counterfactual is forbidden"
        );
    }

    #[test]
    fn holdout_day_is_deterministic() {
        let a = is_holdout_day("evo-fix-test-fail", "2026-07-03");
        let b = is_holdout_day("evo-fix-test-fail", "2026-07-03");
        assert_eq!(a, b, "same skill+date must always agree");
    }

    #[test]
    fn holdout_rotation_covers_both_arms() {
        // Over a month of dates a skill must land in both arms at least once
        // (modulus 3 → expected ~10 holdout days in 30).
        let days: Vec<String> = (1..=30).map(|d| format!("2026-06-{d:02}")).collect();
        let holdouts = days
            .iter()
            .filter(|d| is_holdout_day("evo-fix-test-fail", d))
            .count();
        assert!(
            holdouts > 0 && holdouts < days.len(),
            "rotation must produce both active and holdout days, got {holdouts}/30"
        );
    }

    #[test]
    fn partition_holdout_settles_after_eval_window() {
        // A skill past the evaluation window never rotates into holdout.
        let mut metrics = default_metrics();
        metrics.skill_attribution.insert(
            "evo-proven".into(),
            SkillAttribution {
                skill_name: "evo-proven".into(),
                sessions_active: 20,
                avg_score_with: 0.85,
                avg_score_without: 0.70,
                first_seen: "2026-05-01T00:00:00Z".into(),
                sessions_holdout: 5,
            },
        );
        let skills = vec!["evo-proven".to_string()];
        for d in 1..=30 {
            let date = format!("2026-06-{d:02}");
            let (active, holdout) = partition_holdout(&skills, &metrics, &date);
            assert_eq!(active.len(), 1, "settled skill must always be active");
            assert!(holdout.is_empty());
        }
    }

    #[test]
    fn partition_holdout_is_stable_for_same_date() {
        // resume (SessionStart) and reflect (SessionEnd) call partition_holdout
        // independently with the same date — the arm each skill lands in must
        // be identical, or attribution credits an active-injected skill to the
        // holdout baseline (regression of the bug this guards).
        let metrics = default_metrics();
        let skills: Vec<String> = (0..6).map(|i| format!("evo-skill-{i}")).collect();
        let date = "2026-07-03";
        let (active1, holdout1) = partition_holdout(&skills, &metrics, date);
        let (active2, holdout2) = partition_holdout(&skills, &metrics, date);
        assert_eq!(active1, active2, "same date must yield the same active arm");
        assert_eq!(
            holdout1, holdout2,
            "same date must yield the same holdout arm"
        );
    }

    #[test]
    fn classify_epoch_improving() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.5 + i as f64 * 0.1,
                avg_score: 0.5 + i as f64 * 0.1,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(classify_epoch(&history), EpochClass::Improving);
    }

    #[test]
    fn classify_epoch_regressing() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|i| SessionScoreEntry {
                timestamp: format!("2026-04-0{}", i + 1),
                success_rate: 0.9 - i as f64 * 0.1,
                avg_score: 0.9 - i as f64 * 0.1,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(classify_epoch(&history), EpochClass::Regressing);
    }

    #[test]
    fn classify_epoch_persistent_failure() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|_| SessionScoreEntry {
                timestamp: "2026-04-09".into(),
                success_rate: 0.2,
                avg_score: 0.2,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(classify_epoch(&history), EpochClass::PersistentFailure);
    }

    #[test]
    fn classify_epoch_stable_success() {
        let history: Vec<SessionScoreEntry> = (0..5)
            .map(|_| SessionScoreEntry {
                timestamp: "2026-04-09".into(),
                success_rate: 0.95,
                avg_score: 0.90,
                observations: 10,
                dimension_averages: ScoreDimensions::default(),
            })
            .collect();
        assert_eq!(classify_epoch(&history), EpochClass::StableSuccess);
    }

    #[test]
    fn classify_epoch_single_entry_defaults_insufficient_data() {
        let history = vec![SessionScoreEntry {
            timestamp: "2026-04-09".into(),
            success_rate: 0.5,
            avg_score: 0.5,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }];
        // With <2 entries, returns InsufficientData
        assert_eq!(classify_epoch(&history), EpochClass::InsufficientData);
    }

    #[test]
    fn classify_epoch_empty_history_is_insufficient_data() {
        let history: Vec<SessionScoreEntry> = vec![];
        assert_eq!(classify_epoch(&history), EpochClass::InsufficientData);
    }

    // ── reward-hacking detection (HarnessX Tier 2.2) ──

    /// Build a SessionScoreEntry with the given quality (output_quality) and
    /// efficiency proxy (execution_cost) values. tool_success is held flat.
    fn entry(quality: f64, cost: f64) -> SessionScoreEntry {
        SessionScoreEntry {
            timestamp: "2026-06-01T00:00:00Z".into(),
            success_rate: 0.9,
            avg_score: 0.5,
            observations: 10,
            dimension_averages: ScoreDimensions {
                tool_success: 0.9,
                output_quality: quality,
                execution_cost: cost,
            },
        }
    }

    /// Linearly interpolate `n` values from `lo` to `hi` inclusive.
    fn ramp(n: usize, lo: f64, hi: f64) -> Vec<f64> {
        if n <= 1 {
            return vec![lo];
        }
        (0..n)
            .map(|i| lo + (hi - lo) * (i as f64) / ((n - 1) as f64))
            .collect()
    }

    #[test]
    fn reward_hacking_empty_history_is_false() {
        let metrics = default_metrics();
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_single_entry_is_false() {
        let mut metrics = default_metrics();
        metrics.score_history.push(entry(0.9, 0.5));
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_insufficient_below_window_is_false() {
        // window defaults to 5; provide only 4 entries that *would* hack.
        let mut metrics = default_metrics();
        let quality = ramp(4, 0.9, 0.5);
        let cost = ramp(4, 0.5, 0.9);
        for (q, c) in quality.into_iter().zip(cost) {
            metrics.score_history.push(entry(q, c));
        }
        assert!(
            !detect_reward_hacking(&metrics),
            "fewer than window entries must never flag"
        );
    }

    #[test]
    fn reward_hacking_clear_case_is_true() {
        // quality 0.9 → 0.5 (declining), cost 0.5 → 0.9 (rising) over 5 sessions.
        let mut metrics = default_metrics();
        let quality = ramp(5, 0.9, 0.5);
        let cost = ramp(5, 0.5, 0.9);
        for (q, c) in quality.into_iter().zip(cost) {
            metrics.score_history.push(entry(q, c));
        }
        assert!(
            detect_reward_hacking(&metrics),
            "quality falling while efficiency proxy rises must flag reward hacking"
        );
    }

    #[test]
    fn reward_hacking_honest_improvement_both_rise_is_false() {
        let mut metrics = default_metrics();
        let quality = ramp(5, 0.5, 0.9);
        let cost = ramp(5, 0.5, 0.9);
        for (q, c) in quality.into_iter().zip(cost) {
            metrics.score_history.push(entry(q, c));
        }
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_honest_degradation_both_fall_is_false() {
        let mut metrics = default_metrics();
        let quality = ramp(5, 0.9, 0.5);
        let cost = ramp(5, 0.9, 0.5);
        for (q, c) in quality.into_iter().zip(cost) {
            metrics.score_history.push(entry(q, c));
        }
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_flat_is_false() {
        let mut metrics = default_metrics();
        for _ in 0..5 {
            metrics.score_history.push(entry(0.7, 0.7));
        }
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_flat_produces_no_nan_result() {
        // Guard: flat series must yield a boolean (never panic on NaN).
        let mut metrics = default_metrics();
        for _ in 0..5 {
            metrics.score_history.push(entry(0.7, 0.7));
        }
        let result = detect_reward_hacking(&metrics);
        // Simply asserting it is a bool; if NaN leaked the call would be fine
        // but the value must be a plain false.
        assert!(!result);
    }

    #[test]
    fn reward_hacking_nan_quality_does_not_flag_or_panic() {
        let mut metrics = default_metrics();
        let quality = ramp(5, 0.9, 0.5);
        let cost = ramp(5, 0.5, 0.9);
        for (i, (q, c)) in quality.into_iter().zip(cost).enumerate() {
            let q = if i == 2 { f64::NAN } else { q };
            metrics.score_history.push(entry(q, c));
        }
        assert!(!detect_reward_hacking(&metrics));
    }

    #[test]
    fn reward_hacking_uses_only_last_window_entries() {
        // First 5 entries are honest (both rising); last 5 are a hacking pattern.
        // Detection must consider only the trailing window and flag.
        let mut metrics = default_metrics();
        let early_quality = ramp(5, 0.5, 0.9);
        let early_cost = ramp(5, 0.5, 0.9);
        for (q, c) in early_quality.into_iter().zip(early_cost) {
            metrics.score_history.push(entry(q, c));
        }
        let late_quality = ramp(5, 0.9, 0.5);
        let late_cost = ramp(5, 0.5, 0.9);
        for (q, c) in late_quality.into_iter().zip(late_cost) {
            metrics.score_history.push(entry(q, c));
        }
        assert!(detect_reward_hacking(&metrics));
    }
}
