# Operational Mirror — RL Analogy for the Evolution Engine

> **Status:** Design checklist (HarnessX paper §7.3 calls the mirror "a design heuristic, not a formal framework… a design checklist rather than a predictive theory"). This document maps epic-harness's existing evolution mechanisms onto RL vocabulary so future tuning is principled. It formalizes what the code already does — it adds no new machinery.
>
> **Source:** HarnessX paper (arXiv:2606.14249v1) §4.1–4.2; gap-analysis `ref-010`.

## The mirror in one table

| RL concept | epic-harness realization | Code location |
|---|---|---|
| **State** `s_t` | `SessionAnalysis` snapshot + score history | `src/evolve/analysis.rs::analyze_session`, `Metrics.score_history` |
| **Action** `a_t` | A typed `HarnessEdit` (add skill, modify skill, add guard rule, …) | `src/evolve/edits.rs::HarnessEdit` |
| **Reward** `r_t` | Composite score per tool call (success + quality + cost) | `src/shared/scoring.rs::ScoreDimensions`, weights in `common.rs::SCORE_WEIGHTS` |
| **Transition** | `SolvedTaskRegistry::update` + `manifests.jsonl` append (state grows) | `src/evolve/seesaw.rs`, `reflect.rs` manifest append |
| **Policy** | `plan_skill_edits` (proposes) → `apply_skill_edits` (Critic-verifies + applies) | `src/evolve/skills.rs` |

## The three RL pathologies → epic-harness defenses

The paper (§4.2) predicts three failure modes once harness adaptation is cast as an MDP. Each maps to a concrete epic-harness defense.

### 1. Reward hacking
Gaming the reward signal without genuine improvement (e.g. fewer tool calls inflate the cost score while quality drops).

- **Detector:** `detect_reward_hacking(metrics)` — least-squares slope of `output_quality` vs `execution_cost` over the window. Cost is an *efficiency proxy* (higher = better), so hacking = cost-proxy rising while quality-proxy falls. `src/evolve/metrics.rs`.
- **Gate:** `Critic::should_block_seeding` suppresses ALL new seeding when suspected. `src/evolve/critic.rs`, wired in `reflect.rs`.
- **Per-edit gate:** `Critic::verify_against_evidence` rejects manifests claiming a score lift under hacking. `src/evolve/skills.rs::apply_skill_edits`.

### 2. Catastrophic forgetting
An edit that improves one task distribution regresses another.

- **Coarse gate (per-task):** `seesaw_check` rejects rounds that regress a previously-solved task beyond tolerance. `src/evolve/seesaw.rs`. Deliberately coarse (per-task, not per-dimension — the paper §6.6 proves per-dimension gating misses sub-threshold coupling).
- **Real defense (variant isolation):** `VariantPool::fork_if_needed` forks a sibling variant on regression rather than overwriting. `src/evolve/variants.rs`. The paper §6.3 validates this lifts GAIA from Δ=0.0 stagnation to +13.6%.
- **Aggregate backstop:** `check_stagnation` rolls back the whole evolved set after N scoreless sessions. `src/evolve/metrics.rs`.

### 3. Under-exploration
Bias toward low-risk local edits (prompt tweaks) while structural edit types go untried.

- **Detector:** `build_landscape` tracks `edit_type_coverage` + `untried_edit_types` + persistent failures. `src/evolve/planner.rs`.
- **Signal:** `recommends_exploration()` fires when an unresolved persistent failure coexists with untried structural edit types. `src/evolve/planner.rs`.

## What the mirror does NOT do (paper §7.3)

- It is **not a convergence guarantee.** RL convergence requires sufficient state-action exploration, unattainable when states are symbolic configs and actions are open-ended code edits.
- It does **not predict which pathology dominates** or when. Order/timing/severity are empirical.
- It does **not bound sub-threshold drift.** Even per-task seesaw misses coupling below the detection threshold (paper §7.6). Variant isolation is the practical mitigation, not a proof.

## Tuning guide

When adjusting thresholds, map each to its pathology role:

| Threshold | Location | Pathology | Trade-off |
|---|---|---|---|
| `reward_hacking_window` / `quality_drop` / `cost_rise` | `EvolutionConfig` | reward hacking | too tight → false positives suppress legit evolution; too loose → hacking undetected |
| `seesaw` tolerance (`DEFAULT_TOLERANCE`) | `seesaw.rs` | forgetting | too tight → blocks good edits on noise; too loose → regressions ship |
| `MAX_VARIANTS` | `variants.rs` | forgetting | too low → fork churn on polyglot projects; too high → dilutes signal |
| `stagnation_limit` | `EvolutionConfig` | (backstop) | too low → premature rollback; too high → stalls |

## Cross-references

- HarnessX paper: §4.1 (operational mirror), §4.2 (pathologies), §4.3 (AEGIS defenses), §6.6/§7.6 (seesaw limits).
- gap-analysis: `ref-010-harnessx-gap-analysis` (vault), §4 (RL pathology framework) and §5.1.
- Implementation modules: `src/evolve/{metrics,critic,seesaw,variants,planner,edits}.rs`.
