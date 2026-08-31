//! evolve_regression_test.rs — hermetic regression harness for the evolution engine.
//!
//! Deterministic, hermetic integration tests for the R5–R7 evolution machinery
//! (digester → seesaw → variant isolation → planner). No live benchmark, no
//! SQLite, no network, no HOME redirect. Fixtures are embedded via
//! `include_str!` and parsed with `serde_json`.
//!
//! ## What these tests lock down
//! - **seesaw (R5)**: a previously-solved task (`pipeline_id="PIPE-1"` at score
//!   1.0) that regresses in the next session is flagged. Negative control: the
//!   same seed session passes the gate.
//! - **variant fork (R6)**: `fork_if_needed` with a regression signal spawns a
//!   sibling, pool stays within `MAX_VARIANTS`, and the parent survives. Locks
//!   the fork contract BEFORE R6 wiring to reflect.
//! - **planner (R7)**: a failure category persisting across two distinct
//!   session timestamps drives `recommends_exploration` to true. Negative
//!   controls: a single session, or a resolved failure, yield false.
//! - **outcome_score (R5)**: `Success`=1.0, `CompleteFailure`=0.0, partial in (0,1).
//!
//! ## Hermeticity
//! Task identity comes exclusively from `pipeline_id` (stable), not timestamp
//! gaps — the digester prefers pipeline grouping, so the seed and regression
//! fixtures collapse to the same `task_id="PIPE-1"` regardless of wall-clock.

use std::collections::HashMap;

use epic_harness::evolve;
use epic_harness::evolve::seesaw::DEFAULT_TOLERANCE;
use epic_harness::evolve::variants::{MAX_VARIANTS, VariantPool};
use epic_harness::shared::evolution::{
    EditType, EvolutionRecord, SkillVariant, SolvedTaskRegistry, TaskDigest, TaskOutcome,
};
use epic_harness::shared::obs::ObsRecord;

// ── Fixtures (embedded at compile time, hermetic) ───────────────────────────

const SESSION_N_SEED: &str = include_str!("fixtures/evolve/session_n_seed.jsonl");
const SESSION_REGRESSION: &str = include_str!("fixtures/evolve/session_regression.jsonl");
const HISTORY_TWO_SESSIONS: &str = include_str!("fixtures/evolve/history_two_sessions.json");

// ── Fixture loaders ─────────────────────────────────────────────────────────

/// Parse a JSONL fixture into a Vec<ObsRecord>. Each non-empty line is one record.
fn load_jsonl(jsonl: &str) -> Vec<ObsRecord> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("fixture parse failed ({e}) for line: {l}"))
        })
        .collect()
}

/// Parse the two-session history JSON array into Vec<EvolutionRecord>.
fn load_history(json: &str) -> Vec<EvolutionRecord> {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("history fixture parse failed: {e}"))
}

// ── Scenario 1: seesaw catches a per-task regression ────────────────────────

/// The seed session solves PIPE-1 perfectly (score 1.0); the regression session
/// breaks the SAME pipeline task. The seesaw gate must flag PIPE-1.
///
/// Negative control: the seed session itself passes the gate (no regression
/// against an empty/seeded registry).
#[test]
fn scenario_seesaw_catches_regression() {
    let seed_obs = load_jsonl(SESSION_N_SEED);
    let regression_obs = load_jsonl(SESSION_REGRESSION);

    // Task identity is driven by pipeline_id, so both sessions digest to the
    // SAME task_id="PIPE-1" — the precondition for a seesaw regression.
    let seed_digests = evolve::digest_session(&seed_obs, &[]);
    let regression_digests = evolve::digest_session(&regression_obs, &[]);

    // Sanity: exactly one segment each, both keyed on the stable pipeline id.
    assert_eq!(
        seed_digests.len(),
        1,
        "seed should produce one pipeline segment"
    );
    assert_eq!(
        regression_digests.len(),
        1,
        "regression should produce one pipeline segment"
    );
    assert_eq!(seed_digests[0].task_id, "PIPE-1");
    assert_eq!(regression_digests[0].task_id, "PIPE-1");

    // Seed task fully solved; regression task dropped.
    assert!(
        matches!(seed_digests[0].outcome, TaskOutcome::Success),
        "seed PIPE-1 should be fully solved"
    );
    let regressed_score = evolve::seesaw::scores_from_pipeline_digests(&regression_digests)
        .get("PIPE-1")
        .copied();
    assert!(
        regressed_score.unwrap_or(1.0) < 1.0,
        "regression PIPE-1 score must be below 1.0"
    );

    // Build the registry exactly as R5 reflect wiring does: seed with the
    // best-known per-task scores from session N.
    let mut reg = SolvedTaskRegistry::default();
    reg.update(&evolve::seesaw::scores_from_pipeline_digests(&seed_digests));
    assert_eq!(reg.solved.get("PIPE-1"), Some(&1.0));

    // The regression session must be caught.
    let regressed = evolve::seesaw_check(&reg, &regression_digests, DEFAULT_TOLERANCE);
    assert!(
        regressed.contains(&"PIPE-1".to_string()),
        "seesaw must flag PIPE-1 as regressed, got {regressed:?}"
    );

    // ── Negative control ──
    // The seed session against the seeded registry must NOT regress: its score
    // matches the best. This guards against a vacuously-true positive branch.
    let seed_regressed = evolve::seesaw_check(&reg, &seed_digests, DEFAULT_TOLERANCE);
    assert!(
        seed_regressed.is_empty(),
        "seed session must not regress against the seeded registry, got {seed_regressed:?}"
    );
}

// ── Scenario 2: variant fork-on-regression contract (locks R6 BEFORE wiring)

/// A `VariantPool` with a single rust variant, given a regression signal, must
/// spawn a sibling variant. The pool must stay within `MAX_VARIANTS`, and the
/// parent variant must survive (fork, not replace).
///
/// Negative control: the same call with `would_regress=false` returns the
/// original id and does NOT grow the pool.
#[test]
fn scenario_variant_forks_on_regression() {
    let parent = SkillVariant {
        id: "rust-backend".into(),
        domain_tags: vec!["rust".into()],
        skills: vec!["evo-rust-types".into()],
        avg_score: 0.7,
        task_routing: vec![],
    };
    let mut pool = VariantPool {
        variants: vec![parent],
        routing_stats: HashMap::new(),
    };
    assert_eq!(pool.variants.len(), 1);

    let fork_id = pool.fork_if_needed("rust-backend", true);

    // A sibling was created with a forked id derived from the parent.
    assert_ne!(
        fork_id, "rust-backend",
        "fork must return a NEW id, not the parent"
    );
    assert!(
        fork_id.starts_with("rust-backend-fork-"),
        "fork id should derive from parent, got {fork_id}"
    );

    // Both the parent and the sibling exist in the pool.
    assert_eq!(
        pool.variants.len(),
        2,
        "fork should add exactly one sibling"
    );
    assert!(
        pool.variants.iter().any(|v| v.id == "rust-backend"),
        "parent variant must survive the fork"
    );
    assert!(
        pool.variants.iter().any(|v| v.id == fork_id),
        "sibling variant must be present after fork"
    );

    // The sibling inherits the parent's domain shape (catastrophic-forgetting
    // defense: the sibling starts where the parent left off, then diverges).
    let sibling = pool
        .variants
        .iter()
        .find(|v| v.id == fork_id)
        .expect("sibling must exist");
    assert_eq!(
        sibling.domain_tags,
        vec!["rust".to_string()],
        "sibling should inherit parent domain tags"
    );

    // ── Pool-size invariant: never exceed MAX_VARIANTS ──
    // Fork repeatedly until the pool would overflow; the contract is that the
    // lowest-scoring variant is retired to make room, keeping size bounded.
    let mut overflow_pool = VariantPool {
        variants: vec![
            SkillVariant {
                id: "v1".into(),
                domain_tags: vec!["rust".into()],
                skills: vec![],
                avg_score: 0.9,
                task_routing: vec![],
            },
            SkillVariant {
                id: "v2".into(),
                domain_tags: vec!["python".into()],
                skills: vec![],
                avg_score: 0.5, // lowest — retirement candidate
                task_routing: vec![],
            },
            SkillVariant {
                id: "v3".into(),
                domain_tags: vec!["go".into()],
                skills: vec![],
                avg_score: 0.8,
                task_routing: vec![],
            },
        ],
        routing_stats: HashMap::new(),
    };
    // Reference MAX_VARIANTS by symbol — never hardcode the literal.
    assert_eq!(overflow_pool.variants.len(), MAX_VARIANTS);
    let _ = overflow_pool.fork_if_needed("v1", true);
    assert!(
        overflow_pool.variants.len() <= MAX_VARIANTS,
        "fork at capacity must retire to stay within MAX_VARIANTS={}, got {}",
        MAX_VARIANTS,
        overflow_pool.variants.len()
    );
    // The lowest-scoring variant (v2) was retired to make room.
    assert!(
        !overflow_pool.variants.iter().any(|v| v.id == "v2"),
        "lowest-scoring variant should be retired on a capacity fork"
    );

    // ── Negative control ──
    let mut clean_pool = VariantPool {
        variants: vec![SkillVariant {
            id: "rust-backend".into(),
            domain_tags: vec!["rust".into()],
            skills: vec![],
            avg_score: 0.7,
            task_routing: vec![],
        }],
        routing_stats: HashMap::new(),
    };
    let no_fork_id = clean_pool.fork_if_needed("rust-backend", false);
    assert_eq!(
        no_fork_id, "rust-backend",
        "no regression signal must return the original id"
    );
    assert_eq!(
        clean_pool.variants.len(),
        1,
        "pool must not grow without a regression signal"
    );
}

// ── Scenario 3: planner recommends exploration on persistent failure ────────

/// With two evolution records carrying the SAME failure category (`type_error`)
/// at two distinct session timestamps, the planner detects a persistent failure.
/// Combined with untried structural edit types, it recommends exploration.
///
/// Negative controls:
/// - A single session (no persistence) → no exploration.
/// - A resolved persistent failure → no exploration.
#[test]
fn scenario_planner_recommends_exploration() {
    let history = load_history(HISTORY_TWO_SESSIONS);

    // Sanity: two distinct session timestamps, same failure category.
    assert_eq!(history.len(), 2, "fixture must carry two sessions");
    assert_ne!(
        history[0].timestamp, history[1].timestamp,
        "sessions must be distinct in time"
    );
    assert!(
        history[0].error_patterns.contains_key("type_error")
            && history[1].error_patterns.contains_key("type_error"),
        "both sessions must carry type_error"
    );

    // A current failing digest (regression session) feeds the component heatmap.
    let regression_obs = load_jsonl(SESSION_REGRESSION);
    let digests = evolve::digest_session(&regression_obs, &[]);
    assert!(!digests.is_empty(), "regression must produce a digest");

    let landscape = evolve::build_landscape(&history, &digests, 2);

    // type_error persisted across 2 distinct sessions → persistent failure.
    let cats: Vec<&str> = landscape
        .persistent_failures
        .iter()
        .map(|f| f.failure_category.as_str())
        .collect();
    assert!(
        cats.contains(&"type_error"),
        "type_error should be flagged persistent across two sessions, got {cats:?}"
    );
    let pf = landscape
        .persistent_failures
        .iter()
        .find(|f| f.failure_category == "type_error")
        .expect("persistent type_error must be present");
    assert!(
        pf.sessions_seen >= 2,
        "persistent failure must span >=2 sessions, got {}",
        pf.sessions_seen
    );
    assert!(
        !pf.resolved,
        "unresolved failure must not be marked resolved"
    );

    // Persistent failure + untried structural edit types → explore.
    assert!(
        !landscape.untried_edit_types.is_empty(),
        "there must be untried edit types to justify exploration"
    );
    assert!(
        evolve::recommends_exploration(&landscape),
        "planner should recommend exploration on persistent + untried"
    );

    // ── Negative control A: single session → not persistent ──
    let single = vec![history[0].clone()];
    let single_landscape = evolve::build_landscape(&single, &digests, 2);
    assert!(
        single_landscape.persistent_failures.is_empty(),
        "a single session cannot produce a persistent failure"
    );
    assert!(
        !evolve::recommends_exploration(&single_landscape),
        "no persistent failure → no exploration recommendation"
    );

    // ── Negative control B: resolved persistent failure → no exploration ──
    let mut resolved_landscape = evolve::build_landscape(&history, &digests, 2);
    for f in resolved_landscape.persistent_failures.iter_mut() {
        f.resolved = true;
    }
    assert!(
        !evolve::recommends_exploration(&resolved_landscape),
        "all-resolved failures must not trigger exploration"
    );
}

// ── Scenario 4: outcome_score bounds (locks the scoring contract) ───────────

/// `outcome_score` must map `Success`→1.0, `CompleteFailure`→0.0, and any
/// partial failure strictly into the open interval (0,1). No vacuous truth:
/// each bound is asserted explicitly, including the partial being neither
/// endpoint.
#[test]
fn scenario_outcome_score_bounds() {
    // Success is the ceiling.
    let s = evolve::seesaw::scores_from_pipeline_digests(&[TaskDigest {
        task_id: "t".into(),
        synthetic: false,
        outcome: TaskOutcome::Success,
        failure_categories: vec![],
        implicated_components: vec![],
        evidence_excerpts: vec![],
        tool_trajectory: vec![],
        iterations_seen: 0,
        token_estimate: 0,
        observation_count: 5,
    }]);
    assert_eq!(s.get("t"), Some(&1.0), "Success must score 1.0");
    assert_eq!(
        evolve::seesaw::scores_from_pipeline_digests(&[TaskDigest {
            task_id: "t".into(),
            synthetic: false,
            outcome: TaskOutcome::CompleteFailure,
            failure_categories: vec![],
            implicated_components: vec![],
            evidence_excerpts: vec![],
            tool_trajectory: vec![],
            iterations_seen: 0,
            token_estimate: 0,
            observation_count: 5,
        }])
        .get("t"),
        Some(&0.0),
        "CompleteFailure must score 0.0"
    );

    // Partial: 1 failed of 4 → 0.75 success fraction, strictly inside (0,1).
    let partial = evolve::seesaw::scores_from_pipeline_digests(&[TaskDigest {
        task_id: "t".into(),
        synthetic: false,
        outcome: TaskOutcome::PartialFailure {
            failed_steps: 1,
            total_steps: 4,
        },
        failure_categories: vec![],
        implicated_components: vec![],
        evidence_excerpts: vec![],
        tool_trajectory: vec![],
        iterations_seen: 0,
        token_estimate: 0,
        observation_count: 4,
    }]);
    let p = partial.get("t").copied().unwrap_or(-1.0);
    assert!(
        p > 0.0 && p < 1.0,
        "partial outcome must be strictly inside (0,1), got {p}"
    );
    assert!(
        (p - 0.75).abs() < 1e-9,
        "1-of-4 failed partial should be 0.75, got {p}"
    );

    // Edge: zero total steps in a partial must collapse to 0.0 (defensive),
    // never a divide-by-zero panic. This is a real contract guarantee.
    let zero_total = evolve::seesaw::scores_from_pipeline_digests(&[TaskDigest {
        task_id: "t".into(),
        synthetic: false,
        outcome: TaskOutcome::PartialFailure {
            failed_steps: 0,
            total_steps: 0,
        },
        failure_categories: vec![],
        implicated_components: vec![],
        evidence_excerpts: vec![],
        tool_trajectory: vec![],
        iterations_seen: 0,
        token_estimate: 0,
        observation_count: 0,
    }]);
    assert_eq!(
        zero_total.get("t"),
        Some(&0.0),
        "partial with zero total steps must collapse to 0.0, not panic"
    );

    // ── Negative control against vacuous bounds ──
    // If the `outcome` matcher were ever collapsed to a catch-all, the Success
    // and CompleteFailure scores would no longer pin the extremes and the
    // partial would not be distinguishable. Order the three computed scores so
    // a matcher regression flips this assertion instead of silently passing.
    let success_score = *s.get("t").expect("success score must exist");
    let failure_score = *zero_total.get("t").expect("failure score must exist");
    assert!(
        success_score > p,
        "Success ({success_score}) must outrank partial ({p})"
    );
    assert!(
        p > failure_score,
        "partial ({p}) must outrank CompleteFailure ({failure_score})"
    );
    assert!(
        success_score > failure_score,
        "Success ({success_score}) must outrank CompleteFailure ({failure_score})"
    );
}

// ── Scenario 5: task identity is pipeline-driven (hermeticity guard) ────────

/// Guards the load-bearing design invariant: task identity comes from
/// `pipeline_id`, NOT timestamp gaps. If a future change made the digester fall
/// back to time-gap segmentation, the seed and regression sessions would no
/// longer share a task_id and the seesaw regression would silently stop firing.
/// This test fails loudly in that case.
#[test]
fn scenario_task_identity_is_pipeline_stable() {
    let seed_obs = load_jsonl(SESSION_N_SEED);
    let regression_obs = load_jsonl(SESSION_REGRESSION);

    let seed_digests = evolve::digest_session(&seed_obs, &[]);
    let regression_digests = evolve::digest_session(&regression_obs, &[]);

    // Every observation in both fixtures carries pipeline_id="PIPE-1"; the
    // digester MUST prefer pipeline grouping and emit that exact task_id.
    assert_eq!(seed_digests.len(), 1);
    assert_eq!(regression_digests.len(), 1);
    assert_eq!(
        seed_digests[0].task_id, regression_digests[0].task_id,
        "seed and regression must share a task_id (stable pipeline identity)"
    );
    assert_eq!(
        seed_digests[0].task_id, "PIPE-1",
        "shared task_id must be the pipeline id"
    );

    // The fixtures deliberately span different wall-clock days; if segmentation
    // were time-driven, the seed's 4 records (10:00–10:03, <5min apart) would
    // still be ONE segment but the regression's would too — so this assertion
    // alone is not enough. The real guard is the shared-id assertion above.
    // Confirm no fixture observation is missing a pipeline_id (which would
    // silently switch segmentation strategy).
    for (i, o) in seed_obs.iter().enumerate() {
        assert!(
            o.pipeline_id.as_deref() == Some("PIPE-1"),
            "seed obs[{i}] must carry pipeline_id=PIPE-1, got {:?}",
            o.pipeline_id
        );
    }
    for (i, o) in regression_obs.iter().enumerate() {
        assert!(
            o.pipeline_id.as_deref() == Some("PIPE-1"),
            "regression obs[{i}] must carry pipeline_id=PIPE-1, got {:?}",
            o.pipeline_id
        );
    }
}

// ── Scenario 6: edit-type coverage feeds the planner (locks planner inputs) ─

/// The planner's exploration recommendation depends on `untried_edit_types`
/// being computed from `EditType::all()`. This locks that the two edit types
/// present in the history fixture (`add_skill`, `modify_skill`) are marked
/// tried, and the remaining structural types are untried — the precondition
/// that makes `recommends_exploration` non-vacuous.
#[test]
fn scenario_edit_coverage_marks_attempted_types() {
    let history = load_history(HISTORY_TWO_SESSIONS);
    let landscape = evolve::build_landscape(&history, &[], 2);

    // The two edit types used in the fixture are marked as tried.
    assert_eq!(
        landscape.edit_type_coverage.get("add_skill"),
        Some(&1),
        "add_skill appears once in history"
    );
    assert_eq!(
        landscape.edit_type_coverage.get("modify_skill"),
        Some(&1),
        "modify_skill appears once in history"
    );

    // The other structural edit types remain untried — this is what justifies
    // the exploration recommendation in scenario 3.
    for untried in &[
        EditType::AddInstinct,
        EditType::ModifyConfig,
        EditType::AddGuardRule,
        EditType::ModifyPrompt,
    ] {
        assert!(
            landscape
                .untried_edit_types
                .contains(&untried.as_str().to_string()),
            "{:?} ({}) should be untried",
            untried,
            untried.as_str()
        );
        assert!(
            !landscape.edit_type_coverage.contains_key(untried.as_str()),
            "{:?} should not appear in coverage",
            untried
        );
    }

    // ── Negative control ──
    // Tried types must NOT appear in the untried list (catches a coverage bug
    // where everything is reported as untried, making the recommendation vacuous).
    assert!(
        !landscape
            .untried_edit_types
            .contains(&"add_skill".to_string()),
        "add_skill was attempted — must not be in untried list"
    );
}
