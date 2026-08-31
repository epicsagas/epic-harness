use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::evolve::edits::HarnessEdit;
use crate::shared::{evolution::*, helpers::*, paths::*, sanitize::sanitize_skill_content};

// ── Negative Feedback Buffer (SkillOpt §4) ────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct RejectedBuffer {
    pub(crate) entries: Vec<RejectedEntry>,
}

fn rejected_buffer_file() -> std::path::PathBuf {
    evolved_dir().join("rejected_buffer.json")
}

pub(crate) fn load_rejected_buffer() -> RejectedBuffer {
    read_json(&rejected_buffer_file(), RejectedBuffer::default())
}

fn save_rejected_buffer(buf: &RejectedBuffer) {
    if let Ok(json) = serde_json::to_string_pretty(buf) {
        let _ = fs::write(rejected_buffer_file(), json);
    }
}

/// Add an entry to the rejected buffer.
pub fn add_rejected(name: &str, reason: &str, confidence: f64, origin: &str) {
    let mut buf = load_rejected_buffer();
    // Dedup: update existing entry's timestamp rather than appending a duplicate.
    if let Some(existing) = buf.entries.iter_mut().find(|e| e.name == name) {
        existing.reason = reason.into();
        existing.timestamp = now_iso();
        existing.confidence = confidence;
        existing.origin = origin.into();
    } else {
        buf.entries.push(RejectedEntry {
            name: name.into(),
            reason: reason.into(),
            timestamp: now_iso(),
            confidence,
            origin: origin.into(),
        });
    }
    save_rejected_buffer(&buf);
}

/// Prune expired entries from the rejected buffer.
/// Each reflect call increments a "session_seen" counter stored alongside entries.
/// An entry is expired when the number of sessions since it was added exceeds `rejected_buffer_ttl`.
pub fn prune_rejected_buffer() {
    let mut buf = load_rejected_buffer();
    let ttl = CONFIG.evolution.rejected_buffer_ttl;
    let before = buf.entries.len();
    // Parse timestamp to count sessions. We approximate by keeping entries whose
    // timestamp is within the last `ttl` days (one session per day is typical).
    // For precise tracking, we store the session count in the reason field.
    buf.entries.retain(|e| {
        // Entries newer than ttl days from now are kept.
        // Approximation: parse timestamp and check age in days.
        let age_days = days_since(&e.timestamp);
        age_days <= ttl as f64
    });
    if buf.entries.len() != before {
        save_rejected_buffer(&buf);
    }
}

/// Approximate number of days since an ISO timestamp.
/// Returns f64::MAX for malformed timestamps so they get pruned (fail-open).
fn days_since(iso: &str) -> f64 {
    let date_part = match iso.get(..10) {
        Some(d) => d,
        None => return f64::MAX,
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return f64::MAX;
    }
    let y: i32 = parts[0].parse().unwrap_or(2026);
    let m: u32 = parts[1].parse().unwrap_or(1);
    let d: u32 = parts[2].parse().unwrap_or(1);

    // Simple ordinal since epoch (good enough for day-level comparison)
    let ordinal = date_to_ordinal(y, m, d);
    // "Now" from the helper's perspective — use UTC date
    let now_iso = now_iso();
    let now_parts: Vec<&str> = now_iso
        .get(..10)
        .unwrap_or("2026-01-01")
        .split('-')
        .collect();
    let ny: i32 = now_parts
        .first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2026);
    let nm: u32 = now_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let nd: u32 = now_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let now_ordinal = date_to_ordinal(ny, nm, nd);

    (now_ordinal - ordinal) as f64
}

/// Convert (year, month, day) to a day ordinal for arithmetic.
fn date_to_ordinal(y: i32, m: u32, d: u32) -> i64 {
    let y = y as i64;
    let m = m as i64;
    let d = d as i64;
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let moy_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * moy_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

// ── Slow/Meta Update (SkillOpt §4) ────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowUpdateEntry {
    epoch_class: String,
    score: f64,
    timestamp: String,
}

const MAX_SLOW_UPDATES: usize = 20;

/// Append a slow update entry to each evolved skill's meta.json.
/// The `slow_updates` array is capped at `MAX_SLOW_UPDATES` entries.
///
/// Reads and rewrites the FULL `SkillMeta` — this file has two writers
/// (seeding writes all fields; this appends slow_updates). Rewriting only a
/// partial struct used to erase `name`/`active`/`synthesized` on every
/// SessionEnd, silently reverting `evolve accept-synth` flags.
pub fn update_meta_field(skills: &[String], epoch: &EpochClass, score: f64) {
    let epoch_str = match epoch {
        EpochClass::Improving => "improving",
        EpochClass::Regressing => "regressing",
        EpochClass::PersistentFailure => "persistent_failure",
        EpochClass::StableSuccess => "stable_success",
        EpochClass::InsufficientData => return, // skip meta update when data is insufficient
    };
    let entry = SlowUpdateEntry {
        epoch_class: epoch_str.into(),
        score,
        timestamp: now_iso(),
    };

    for skill_name in skills {
        let dir = evolved_dir().join(skill_name);
        if !dir.is_dir() {
            continue;
        }
        let meta_path = dir.join("meta.json");
        let mut meta: SkillMeta = read_json(&meta_path, SkillMeta::default());
        meta.slow_updates.push(entry.clone());
        // Cap at MAX_SLOW_UPDATES — prune oldest
        if meta.slow_updates.len() > MAX_SLOW_UPDATES {
            let start = meta.slow_updates.len() - MAX_SLOW_UPDATES;
            meta.slow_updates = meta.slow_updates[start..].to_vec();
        }
        if let Ok(json) = serde_json::to_string_pretty(&meta) {
            let _ = fs::write(&meta_path, json);
        }
    }
}

/// Check if a skill name is in the rejected buffer.
pub(crate) fn is_rejected(name: &str, buf: &RejectedBuffer) -> bool {
    buf.entries.iter().any(|e| e.name == name)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PromotionCounter {
    /// Map of skill name -> number of sessions that observed this pattern
    pub(crate) counts: HashMap<String, u64>,
}

fn promotion_file() -> std::path::PathBuf {
    evolved_dir().join("promotion_counters.json")
}

fn load_promotion_counters() -> PromotionCounter {
    read_json(&promotion_file(), PromotionCounter::default())
}

fn save_promotion_counters(counters: &PromotionCounter) {
    if let Ok(json) = serde_json::to_string_pretty(counters) {
        let _ = fs::write(promotion_file(), json);
    }
}

/// Check if a skill name has enough support to be promoted.
/// Increments the counter and returns true if threshold is met.
pub(crate) fn check_promotion(name: &str, counters: &mut PromotionCounter) -> bool {
    let count = counters.counts.entry(name.into()).or_insert(0);
    *count += 1;
    *count >= CONFIG.evolution.gated_promotion_min
}

// -- R14: Solver-Proposes + Curator Pattern --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ProposalAction {
    Accept,
    Merge,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SkillProposal {
    pub(crate) name: String,
    pub(crate) content: String,
    pub(crate) origin: String,
    pub(crate) confidence: f64,
    pub(crate) rationale: String,
}

/// Curator: decides whether to accept a proposal based on masked feedback.
/// The curator only sees pass/fail signals (existing names, confidence thresholds),
/// not raw observation scores. Also checks the negative feedback buffer (SkillOpt §4).
pub(crate) fn curate_proposal(
    proposal: &SkillProposal,
    existing: &[String],
    buf: &RejectedBuffer,
) -> ProposalAction {
    // Rule 0 (SkillOpt): If in rejected buffer, skip
    if is_rejected(&proposal.name, buf) {
        return ProposalAction::Skip;
    }

    // Rule 1: If skill already exists, skip (don't overwrite)
    if existing.contains(&proposal.name) {
        return ProposalAction::Skip;
    }

    // Rule 2: If confidence is too low, skip
    if proposal.confidence < 0.2 {
        return ProposalAction::Skip;
    }

    // Rule 3: If origin is a well-known strong signal, accept directly
    if proposal.confidence >= 0.7 {
        return ProposalAction::Accept;
    }

    // Rule 4: Medium confidence — accept but mark for monitoring
    ProposalAction::Merge
}

/// Solver: generates proposals from session analysis.
/// Replaces direct skill writing with a proposal-first approach.
pub(crate) fn build_proposals(analysis: &SessionAnalysis) -> Vec<SkillProposal> {
    let mut proposals = Vec::new();

    // Proposals from failure patterns
    for pattern in &analysis.failure_patterns {
        let name = format!("evo-{}", pattern.pattern_type);
        let confidence = (pattern.count as f64 / 10.0).min(1.0);
        proposals.push(SkillProposal {
            name,
            content: build_pattern_skill(pattern),
            origin: "pattern".into(),
            confidence,
            rationale: format!(
                "{}x {} pattern detected",
                pattern.count, pattern.pattern_type
            ),
        });
    }

    // Proposals from weak tools
    for stats in analysis.per_tool_stats.values() {
        let rate = if stats.total > 0 {
            stats.successes as f64 / stats.total as f64
        } else {
            1.0
        };
        if rate >= CONFIG.pattern.weak_tool_rate || stats.total < CONFIG.pattern.weak_tool_min_obs {
            continue;
        }
        let name = format!("evo-{}-discipline", stats.tool_category);
        proposals.push(SkillProposal {
            name,
            content: build_tool_skill(stats),
            origin: "weak_tool".into(),
            confidence: 1.0 - rate,
            rationale: format!(
                "{} tool success rate was {:.0}%",
                stats.tool_category,
                rate * 100.0
            ),
        });
    }

    // Proposals from weak extensions (only in full mode - handled by caller)
    for (ext, stats) in &analysis.per_ext_stats {
        if stats.success_rate >= CONFIG.pattern.weak_ext_rate
            || stats.total < CONFIG.pattern.weak_ext_min_obs
            || ext == "unknown"
        {
            continue;
        }
        let clean = ext.trim_start_matches('.');
        let name = format!("evo-{clean}-care");
        proposals.push(SkillProposal {
            name,
            content: build_ext_skill(ext, stats),
            origin: "weak_ext".into(),
            confidence: 1.0 - stats.success_rate,
            rationale: format!(
                "{} files had {:.0}% success rate",
                ext,
                stats.success_rate * 100.0
            ),
        });
    }

    // Proposals from high-frequency errors
    for (category, count) in &analysis.per_error_stats {
        if *count < CONFIG.pattern.high_freq_error_min {
            continue;
        }
        let name = format!("evo-fix-{}", category.replace('_', "-"));
        let confidence = (*count as f64 / 20.0).min(1.0);
        proposals.push(SkillProposal {
            name,
            content: build_failure_skill(category, *count),
            origin: "high_freq_error".into(),
            confidence,
            rationale: format!("{}x {} errors detected", count, category),
        });
    }

    // Proposals from minibatch insights (SkillOpt §4 backward pass)
    for mb in &analysis.minibatch_insights {
        if !mb.reusable || mb.success_rate >= 0.8 {
            continue;
        }
        // Only generate a proposal if this batch had a clear error category
        if let Some(ref err_cat) = mb.dominant_error_category {
            let name = format!("evo-batch{}-{}", mb.batch_index, err_cat.replace('_', "-"));
            let files = mb.file_cluster.join(", ");
            proposals.push(SkillProposal {
                name,
                content: format!(
                    "# Minibatch {} Insight\n\n\
Batch success rate: {:.0}%\n\n\
Dominant error: {err_cat}\n\
Dominant tool: {}\n\
Files: {files}\n\n\
## Guidance\n\
When encountering {err_cat} errors with {} operations on {files}:\n\
1. Read the full error message before acting\n\
2. Check surrounding context (50+ lines)\n\
3. Verify the fix compiles before moving on\n",
                    mb.batch_index + 1,
                    mb.success_rate * 100.0,
                    mb.dominant_tool,
                    mb.dominant_tool,
                ),
                origin: "minibatch".into(),
                confidence: 1.0 - mb.success_rate,
                rationale: format!(
                    "Batch {} had {:.0}% success ({} errors on {})",
                    mb.batch_index + 1,
                    mb.success_rate * 100.0,
                    err_cat,
                    mb.dominant_tool,
                ),
            });
        }
    }

    proposals
}

/// A typed plan of edits produced by the planner role, applied by the executor.
///
/// `counters` is returned so the caller persists promotion-counter state at
/// exactly the same point it did before the plan/apply split.
#[derive(Debug, Default)]
pub(crate) struct SkillEditPlan {
    pub edits: Vec<HarnessEdit>,
    pub counters: PromotionCounter,
    pub promoted_count: u64,
}

/// Planner role (pure): decide which proposals become typed `HarnessEdit`s,
/// applying graduated-scope + curation + gated-promotion exactly as the legacy
/// loop did. Writes nothing, prints nothing. Returns the plan + counters so the
/// thin wrapper can save counters and emit hints at the original points.
pub(crate) fn plan_skill_edits(
    analysis: &SessionAnalysis,
    existing: &[String],
    counters: &mut PromotionCounter,
) -> SkillEditPlan {
    let avg_score = analysis.avg_score;
    let full_seeding = avg_score < CONFIG.pattern.graduated_scope_moderate;
    let cap = CONFIG.evolution.max_skills.saturating_sub(existing.len());
    let rejected_buf = load_rejected_buffer();

    let proposals = build_proposals(analysis);
    let mut edits: Vec<HarnessEdit> = Vec::new();
    let mut promoted_count = 0u64;

    for proposal in &proposals {
        if edits.len() >= cap {
            break;
        }
        if existing.contains(&proposal.name) {
            continue;
        }
        if !full_seeding && (proposal.origin == "weak_ext" || proposal.origin == "high_freq_error")
        {
            continue;
        }
        let action = curate_proposal(proposal, existing, &rejected_buf);
        match action {
            ProposalAction::Skip => continue,
            ProposalAction::Merge | ProposalAction::Accept => {
                if !check_promotion(&proposal.name, counters) {
                    continue;
                }
                edits.push(HarnessEdit::AddSkill {
                    name: proposal.name.clone(),
                    content: proposal.content.clone(),
                    origin: proposal.origin.clone(),
                    confidence: proposal.confidence,
                });
                promoted_count += 1;
            }
        }
    }

    SkillEditPlan {
        edits,
        counters: std::mem::take(counters),
        promoted_count,
    }
}

/// Executor role: apply a plan's edits via the typed `HarnessEdit::apply()`
/// path. Each edit is validated before apply. Returns the count applied.
/// Outcome of applying a skill-edit plan: how many edits landed, plus the
/// falsifiable manifest for each APPLIED edit (HarnessX Table 9). Manifests of
/// edits the Critic rejected or that failed validation are excluded. The
/// caller persists these (reflect → manifests.jsonl); a Critic that READS
/// them to falsify prior predictions is a deferred follow-up (today the
/// per-edit Critic gate only consults the in-round reward-hacking flag — see
/// `Critic::verify_against_evidence`).
#[derive(Debug, Clone, Default)]
pub struct ApplyOutcome {
    pub applied: u64,
    pub manifests: Vec<crate::evolve::edits::EditManifest>,
}

/// Executor role: validate → Critic-verify → apply each typed edit.
///
/// Edits whose manifest the Critic Rejects are skipped (not applied, no
/// manifest emitted). The Critic receives the same `metrics` the round-level
/// block decision used, so the verdict is consistent with the seeding gate.
pub(crate) fn apply_skill_edits(
    plan: &SkillEditPlan,
    metrics: &crate::shared::evolution::Metrics,
) -> ApplyOutcome {
    let mut outcome = ApplyOutcome::default();
    for edit in &plan.edits {
        if edit.validate().is_err() {
            continue;
        }
        let manifest = edit.manifest();
        // Critic falsifiability gate (HarnessX §4.3): reject edits whose
        // claimed impact contradicts observed evidence. The most common
        // contradiction today: claiming a score lift while reward hacking is
        // suspected (quality falling). This is defense-in-depth — the
        // round-level block already suppressed seeding under reward hacking,
        // so this per-edit gate is the hook for FUTURE non-reward-hacking
        // contradictions.
        if let crate::evolve::critic::CriticVerdict::Reject(_) =
            crate::evolve::critic::Critic::verify_against_evidence(&manifest, &[], metrics)
        {
            continue;
        }
        if matches!(edit.apply(), crate::evolve::edits::EditOutcome::Applied) {
            outcome.applied += 1;
            outcome.manifests.push(manifest);
        }
    }
    outcome
}

pub fn seed_smart_skills(
    analysis: &SessionAnalysis,
    existing: &[String],
    metrics: &crate::shared::evolution::Metrics,
) -> ApplyOutcome {
    let avg_score = analysis.avg_score;

    // Graduated Scope: skip seeding for excellent sessions
    if avg_score >= CONFIG.pattern.graduated_scope_skip {
        let skip = CONFIG.pattern.graduated_scope_skip;
        hint(
            "reflect",
            &format!(
                "Graduated Scope: skipping skill seeding (avg_score={avg_score:.3} >= {skip})"
            ),
        );
        return ApplyOutcome::default();
    }

    let full_seeding = avg_score < CONFIG.pattern.graduated_scope_moderate;
    if !full_seeding {
        hint(
            "reflect",
            &format!("Graduated Scope: moderate seeding (avg_score={avg_score:.3})"),
        );
    }

    let mut counters = load_promotion_counters();
    let mut plan = plan_skill_edits(analysis, existing, &mut counters);

    // Synthesis manifest pass: emit pending-synthesis manifests (failure
    // evidence + template body) for up to N seeded skills. A host agent
    // upgrades them later via `epic-harness evolve accept-synth`. Runs between
    // the planner and the executor; skills keep template bodies until upgraded.
    let emitted = crate::evolve::synthesis::upgrade_edits(&mut plan.edits, analysis);
    if emitted > 0 {
        hint(
            "reflect",
            &format!("Synthesis: {emitted} pending manifest(s) emitted for host-agent synthesis"),
        );
    }

    let outcome = apply_skill_edits(&plan, metrics);

    save_promotion_counters(&plan.counters);
    if plan.promoted_count > 0 {
        let min_obs = CONFIG.evolution.gated_promotion_min;
        hint(
            "reflect",
            &format!(
                "Gated Promotion: {} skill(s) promoted after {min_obs}+ observations",
                plan.promoted_count
            ),
        );
    }
    outcome
}

pub fn sanitize_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.starts_with('.')
        && name.len() < 64
}

pub fn write_skill_with_meta(
    name: &str,
    content: &str,
    origin: &str,
    confidence: f64,
    synthesized: bool,
) {
    if !sanitize_skill_name(name) {
        hint("reflect", &format!("Rejected invalid skill name: {name}"));
        return;
    }
    let dir = evolved_dir().join(name);
    ensure_dir(&dir);

    let meta = SkillMeta {
        name: name.into(),
        origin: origin.into(),
        confidence,
        project: project_slug(),
        created: now_iso(),
        updated: now_iso(),
        active: true,
        synthesized,
        slow_updates: Vec::new(),
    };
    let json = match serde_json::to_string_pretty(&meta) {
        Ok(j) => j,
        Err(e) => {
            hint(
                "reflect",
                &format!("Failed to serialize meta.json for {name}: {e}"),
            );
            return;
        }
    };
    if let Err(e) = fs::write(dir.join("meta.json"), json) {
        hint(
            "reflect",
            &format!("Failed to write meta.json for {name}: {e}"),
        );
        return;
    }
    if let Err(e) = fs::write(dir.join("SKILL.md"), content) {
        hint(
            "reflect",
            &format!("Failed to write SKILL.md for {name}: {e}"),
        );
        // Clean up orphaned meta.json
        let _ = fs::remove_file(dir.join("meta.json"));
    }
}

/// Whether a skill's body was upgraded by a host agent (`evolve accept-synth`).
/// Missing meta.json = seeded template = not synthesized.
pub fn is_synthesized(name: &str) -> bool {
    let meta_path = evolved_dir().join(name).join("meta.json");
    fs::read_to_string(meta_path)
        .ok()
        .and_then(|s| serde_json::from_str::<SkillMeta>(&s).ok())
        .map(|m| m.synthesized)
        .unwrap_or(false)
}

pub fn write_workspace_manifest() {
    let evolved = evolved_dir();
    if !evolved.is_dir() {
        return;
    }

    let dirs = list_dirs(&evolved);
    let mut skills = Vec::new();

    for name in &dirs {
        let skill_file = evolved.join(name).join("SKILL.md");
        let meta_file = evolved.join(name).join("meta.json");

        let active = skill_file.is_file();
        let meta: SkillMeta = if meta_file.is_file() {
            read_json(&meta_file, SkillMeta::default())
        } else {
            SkillMeta {
                name: name.clone(),
                origin: "unknown".into(),
                confidence: 0.5,
                project: project_slug(),
                created: now_iso(),
                updated: now_iso(),
                active,
                synthesized: false,
                slow_updates: Vec::new(),
            }
        };
        skills.push(SkillMeta { active, ..meta });
    }

    let manifest = WorkspaceManifest {
        version: "1.0".into(),
        updated: now_iso(),
        skills,
    };

    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = fs::write(evolved.join("manifest.json"), json);
    }
}

pub fn build_pattern_skill(p: &DetectedPattern) -> String {
    sanitize_skill_content(&format!(
        "---\nname: {}\ndescription: \"Auto-evolved from {}x {} pattern.\"\n---\n\n# {}\n\n**Detected**: {}\n**Files**: {}\n\n## Remediation\n{}\n\n## Red Flags\n- Retrying the same approach that already failed\n- Not reading the full error context\n- Patching symptoms instead of root cause\n",
        p.pattern_type,
        p.count,
        p.pattern_type,
        p.pattern_type,
        p.description,
        if p.involved_files.is_empty() {
            "various".into()
        } else {
            p.involved_files.join(", ")
        },
        p.suggested_remediation,
    ))
}

pub fn build_tool_skill(stats: &ToolStats) -> String {
    let rate = if stats.total > 0 {
        (stats.successes as f64 / stats.total as f64 * 100.0) as u32
    } else {
        0
    };
    let mut top: Vec<_> = stats.failure_categories.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    top.truncate(3);
    let failures = top
        .iter()
        .map(|(c, n)| format!("- {c}: {n} occurrences"))
        .collect::<Vec<_>>()
        .join("\n");

    sanitize_skill_content(&format!(
        "---\nname: {cat}-discipline\ndescription: \"Auto-evolved. {cat} tool success rate was {rate}%.\"\n---\n\n# {cat} discipline\n\nSuccess rate: {rate}% ({s}/{t})\n\n## Top Failure Types\n{failures}\n\n## Process\n1. Before using {cat}: verify preconditions\n2. Check the expected output format\n3. Validate paths and arguments exist\n4. On failure: read the FULL error, don't retry blindly\n\n## Red Flags\n- {cat} success rate still below 60%\n- Same error type repeating\n",
        cat = stats.tool_category,
        s = stats.successes,
        t = stats.total,
        failures = if failures.is_empty() {
            "- various errors".into()
        } else {
            failures
        },
    ))
}

pub fn build_ext_skill(ext: &str, stats: &ExtStats) -> String {
    let rate = (stats.success_rate * 100.0) as u32;
    sanitize_skill_content(&format!(
        "---\nname: {clean}-care\ndescription: \"Auto-evolved. {ext} files had {rate}% success rate.\"\n---\n\n# {ext} file care\n\nSuccess rate: {rate}% across {t} operations\n\n## Process\n1. Before editing {ext} files: run type-check / lint / build\n2. After editing: immediately verify\n3. If error: read the full diagnostic before re-editing\n\n## Red Flags\n- Editing {ext} files without verifying afterward\n- Ignoring compiler/linter warnings\n",
        clean = ext.trim_start_matches('.'),
        t = stats.total,
    ))
}

pub fn build_failure_skill(category: &str, count: u64) -> String {
    let remediation = match category {
        "type_error" => {
            "Check variable types. Read the full type signature. Use explicit type annotations."
        }
        "syntax_error" => {
            "Check brackets, commas, semicolons. Ensure template literals are properly closed."
        }
        "test_fail" => {
            "Read the assertion message carefully. Check expected vs actual. Run the failing test in isolation."
        }
        "lint_fail" => {
            "Run the linter and fix all warnings. Configure the editor to show lint errors inline."
        }
        "build_fail" => {
            "Run `tsc --noEmit` (or equivalent) to see all errors. Fix type errors before runtime testing."
        }
        "permission_denied" => "Check file permissions. Don't write to system directories.",
        "timeout" => "Command took too long. Consider a more targeted approach.",
        "not_found" => "File or command not found. Verify the path exists. Use glob to locate.",
        _ => "Read the full error message. Identify root cause. Fix the cause, not the symptom.",
    };
    let display = category.replace('_', " ");
    sanitize_skill_content(&format!(
        "---\nname: fix-{dash}\ndescription: \"Auto-evolved from {count}x {category} failures.\"\n---\n\n# Fix {display}\n\nDetected {count} occurrences.\n\n## Remediation\n{remediation}\n\n## Process\n1. Stop — do not retry blindly\n2. Read the full error message and stack trace\n3. Form a hypothesis about root cause\n4. Fix the root cause, not the symptom\n5. Verify with a test or build\n\n## Red Flags\n- Retrying the same approach unchanged\n- Patching symptoms instead of root cause\n",
        dash = category.replace('_', "-"),
    ))
}

pub fn gate_skills() {
    let evolved = evolved_dir();
    if !evolved.is_dir() {
        return;
    }

    for name in list_dirs(&evolved) {
        let skill_file = evolved.join(&name).join("SKILL.md");
        if !skill_file.is_file() {
            add_rejected(&name, "gate_missing_skill_file", 0.0, "gate");
            rm_dir(&evolved.join(&name));
            continue;
        }
        let content = fs::read_to_string(&skill_file).unwrap_or_default();
        let body = content.splitn(3, "---").nth(2).unwrap_or("").trim();
        if !content.starts_with("---") || body.len() < 20 {
            add_rejected(&name, "gate_invalid_format", 0.0, "gate");
            rm_dir(&evolved.join(&name));
        }
    }

    // Enforce cap — oldest skills (sorted alphabetically) are removed
    let mut remaining = list_dirs(&evolved);
    remaining.sort();
    if remaining.len() > CONFIG.evolution.max_skills {
        let excess = &remaining[..remaining.len() - CONFIG.evolution.max_skills];
        for name in excess {
            add_rejected(name, "gate_cap_exceeded", 0.0, "gate");
            rm_dir(&evolved.join(name));
        }
    }
}

pub fn export_to_global(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) {
    if !cross_project_file().is_file() {
        return;
    }
    ensure_dir(&global_harness_dir());

    let project_name = cwd()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let weak_tools: Vec<String> = analysis
        .per_tool_stats
        .iter()
        .filter(|(_, s)| {
            s.total >= CONFIG.pattern.weak_tool_min_obs
                && (s.successes as f64 / s.total as f64) < CONFIG.pattern.weak_tool_rate
        })
        .map(|(cat, _)| cat.clone())
        .collect();

    let record = serde_json::json!({
        "timestamp": now_iso(),
        "project": project_name,
        "success_rate": analysis.success_rate,
        "avg_score": analysis.avg_score,
        "per_error_stats": analysis.per_error_stats,
        "failure_patterns": patterns.iter().map(|p| serde_json::json!({
            "pattern_type": p.pattern_type,
            "count": p.count,
            "remediation": p.suggested_remediation,
        })).collect::<Vec<_>>(),
        "weak_tools": weak_tools,
    });

    // Write to SQLite pool (primary) + JSONL (fallback)
    if let Ok(pool) = crate::store::runtime::block_on(crate::store::pool::harness_pool()) {
        let per_error_json = serde_json::to_string(&analysis.per_error_stats).unwrap_or_default();
        let failure_json = serde_json::to_string(patterns).unwrap_or_default();
        let weak_json = serde_json::to_string(&weak_tools).unwrap_or_default();
        if let Err(e) = crate::store::runtime::block_on(crate::store::global::insert_pattern_pool(
            &pool,
            record["timestamp"].as_str().unwrap_or(""),
            &project_name,
            analysis.success_rate,
            analysis.avg_score,
            &per_error_json,
            &failure_json,
            &weak_json,
        )) {
            eprintln!("[skills] SQLite global pattern write failed: {e}");
        }
    }
    append_jsonl(&global_patterns_file(), &record);
}

// -- Phase 9: Workspace Contract (R13) --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMeta {
    pub name: String,
    pub origin: String, // "pattern", "weak_tool", "weak_ext", "high_freq_error"
    pub confidence: f64,
    pub project: String,
    pub created: String,
    pub updated: String,
    pub active: bool,
    /// True once a host agent upgraded the body via `evolve accept-synth`.
    /// Template (un-synthesized) skills are withheld from injection for
    /// frontier-class models — generic advice is noise there.
    #[serde(default)]
    pub synthesized: bool,
    /// Slow/meta update history (SkillOpt §4), capped at MAX_SLOW_UPDATES.
    /// Lives on SkillMeta so the file has exactly one schema — a second
    /// partial writer here used to erase every other field on rewrite.
    #[serde(default)]
    pub slow_updates: Vec<SlowUpdateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceManifest {
    pub version: String,
    pub updated: String,
    pub skills: Vec<SkillMeta>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG;

    #[test]
    fn pattern_skill_has_frontmatter() {
        let p = DetectedPattern {
            pattern_type: "repeated_same_error".into(),
            description: "test".into(),
            count: 5,
            involved_files: vec!["/src/main.ts".into()],
            suggested_remediation: "stop".into(),
            implicated_components: vec![],
        };
        let skill = build_pattern_skill(&p);
        assert!(skill.starts_with("---\n"));
        assert!(skill.contains("Remediation"));
        assert!(skill.contains("Red Flags"));
    }

    #[test]
    fn tool_skill_has_process() {
        let stats = ToolStats {
            tool_category: "bash".into(),
            total: 10,
            successes: 4,
            errors: 6,
            avg_score: 0.4,
            failure_categories: [("type_error".into(), 4)].into(),
        };
        let skill = build_tool_skill(&stats);
        assert!(skill.contains("Process"));
        assert!(skill.contains("40%"));
    }

    #[test]
    fn ext_skill_has_ext_name() {
        let stats = ExtStats {
            total: 10,
            errors: 5,
            success_rate: 0.5,
        };
        let skill = build_ext_skill(".ts", &stats);
        assert!(skill.contains(".ts"));
        assert!(skill.contains("50%"));
    }

    #[test]
    fn failure_skill_has_remediation() {
        let skill = build_failure_skill("type_error", 8);
        assert!(skill.contains("type"));
        assert!(skill.contains("8 occurrences"));
    }

    #[test]
    fn sanitize_skill_name_rejects_traversal() {
        assert!(
            !sanitize_skill_name("../etc/passwd"),
            "path traversal with .. must be rejected"
        );
        assert!(
            !sanitize_skill_name("foo/../../../etc"),
            "path traversal with / must be rejected"
        );
        assert!(
            !sanitize_skill_name("foo\\bar"),
            "backslash must be rejected"
        );
        assert!(!sanitize_skill_name(".."), "bare .. must be rejected");
        assert!(
            !sanitize_skill_name(".hidden"),
            "dot-prefixed name must be rejected"
        );
        assert!(!sanitize_skill_name(""), "empty name must be rejected");
        let long_name = "x".repeat(64);
        assert!(
            !sanitize_skill_name(&long_name),
            "name >= 64 chars must be rejected"
        );
    }

    #[test]
    fn sanitize_skill_name_accepts_valid_names() {
        assert!(
            sanitize_skill_name("evo-ts-care"),
            "evo-ts-care should be valid"
        );
        assert!(
            sanitize_skill_name("evo-fix-syntax-error"),
            "evo-fix-syntax-error should be valid"
        );
        assert!(
            sanitize_skill_name("evo-bash-discipline"),
            "evo-bash-discipline should be valid"
        );
        assert!(
            sanitize_skill_name("evo-repeated_same_error"),
            "name with underscores should be valid"
        );
        assert!(sanitize_skill_name("a"), "single char name should be valid");
    }

    #[test]
    fn check_promotion_increments() {
        let mut counters = PromotionCounter::default();
        let min = CONFIG.evolution.gated_promotion_min;
        // First min-1 calls must return false
        for i in 0..min.saturating_sub(1) {
            assert!(
                !check_promotion("evo-test", &mut counters),
                "call {} must not be promoted (min={})",
                i + 1,
                min
            );
            assert_eq!(counters.counts["evo-test"], (i + 1));
        }
        // The min-th call must return true
        assert!(
            check_promotion("evo-test", &mut counters),
            "call {} must be promoted (min={})",
            min,
            min
        );
        assert_eq!(counters.counts["evo-test"], min);
    }

    #[test]
    fn check_promotion_requires_min_observations() {
        let mut counters = PromotionCounter::default();
        for _ in 0..CONFIG.evolution.gated_promotion_min - 1 {
            assert!(
                !check_promotion("evo-fix-type-error", &mut counters),
                "should not be promoted before reaching gated_promotion_min"
            );
        }
        assert!(
            check_promotion("evo-fix-type-error", &mut counters),
            "should be promoted once gated_promotion_min is reached"
        );
    }

    #[test]
    fn promotion_counter_persists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("promotion_counters.json");

        let counters = PromotionCounter {
            counts: {
                let mut m = HashMap::new();
                m.insert("evo-test".into(), 3);
                m.insert("evo-other".into(), 1);
                m
            },
        };
        let json = serde_json::to_string_pretty(&counters).expect("serialize");
        let _ = fs::write(&path, &json);

        let content = fs::read_to_string(&path).expect("read");
        let loaded: PromotionCounter = serde_json::from_str(&content).expect("deserialize");
        assert_eq!(loaded.counts["evo-test"], 3);
        assert_eq!(loaded.counts["evo-other"], 1);
    }

    #[test]
    fn gated_promotion_prevents_single_success() {
        let mut counters = PromotionCounter::default();
        let promoted = check_promotion("evo-once-seen", &mut counters);
        assert!(
            !promoted,
            "skill seen only once must not be promoted (count={})",
            counters.counts.get("evo-once-seen").unwrap_or(&0)
        );
        assert_eq!(counters.counts["evo-once-seen"], 1);
    }

    #[test]
    fn curate_proposal_skips_existing() {
        let proposal = SkillProposal {
            name: "evo-test".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.8,
            rationale: "test".into(),
        };
        let existing = vec!["evo-test".into()];
        assert!(matches!(
            curate_proposal(&proposal, &existing, &RejectedBuffer::default()),
            ProposalAction::Skip
        ));
    }

    #[test]
    fn curate_proposal_skips_low_confidence() {
        let proposal = SkillProposal {
            name: "evo-new".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.1,
            rationale: "test".into(),
        };
        assert!(matches!(
            curate_proposal(&proposal, &[], &RejectedBuffer::default()),
            ProposalAction::Skip
        ));
    }

    #[test]
    fn curate_proposal_accepts_high_confidence() {
        let proposal = SkillProposal {
            name: "evo-new".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.7,
            rationale: "test".into(),
        };
        assert!(matches!(
            curate_proposal(&proposal, &[], &RejectedBuffer::default()),
            ProposalAction::Accept
        ));
    }

    #[test]
    fn curate_proposal_merges_medium_confidence() {
        let proposal = SkillProposal {
            name: "evo-new".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.5,
            rationale: "test".into(),
        };
        assert!(matches!(
            curate_proposal(&proposal, &[], &RejectedBuffer::default()),
            ProposalAction::Merge
        ));
    }

    #[test]
    fn build_proposals_from_failure_patterns() {
        let analysis = SessionAnalysis {
            failure_patterns: vec![DetectedPattern {
                pattern_type: "repeated_same_error".into(),
                description: "test".into(),
                count: 5,
                involved_files: vec![],
                suggested_remediation: "stop".into(),
                implicated_components: vec![],
            }],
            ..Default::default()
        };
        let proposals = build_proposals(&analysis);
        assert!(
            proposals
                .iter()
                .any(|p| p.name == "evo-repeated_same_error"),
            "should contain proposal from failure pattern"
        );
    }

    #[test]
    fn build_proposals_respects_thresholds() {
        let mut per_tool = HashMap::new();
        per_tool.insert(
            "bash".into(),
            ToolStats {
                tool_category: "bash".into(),
                total: 10,
                successes: 6,
                errors: 4,
                avg_score: 0.6,
                failure_categories: HashMap::new(),
            },
        );
        let analysis = SessionAnalysis {
            per_tool_stats: per_tool,
            ..Default::default()
        };
        let proposals = build_proposals(&analysis);
        assert!(
            !proposals.iter().any(|p| p.origin == "weak_tool"),
            "tools with rate >= 0.6 should not generate proposals"
        );
    }

    #[test]
    fn gate_skills_body_extraction_with_embedded_dashes() {
        let content = "---\nname: test\n---\n\n# Body content here, long enough\n\n---\n\nmore content below\n";
        let body_splitn = content.splitn(3, "---").nth(2).unwrap_or("").trim();
        let body_unlimited = content.split("---").nth(2).unwrap_or("").trim();

        assert!(
            body_splitn.starts_with("# Body"),
            "splitn body: {:?}",
            body_splitn
        );
        assert!(
            body_splitn.contains("more content"),
            "splitn must preserve full body: {:?}",
            body_splitn
        );
        assert!(
            !body_unlimited.contains("more content"),
            "unlimited split truncates body: {:?}",
            body_unlimited
        );
        assert!(body_splitn.len() >= 20);
    }

    #[test]
    fn workspace_manifest_has_version() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let evolved = tmp.path().join("evolved");
        crate::shared::helpers::ensure_dir(&evolved);

        let skill_dir = evolved.join("evo-test-skill");
        crate::shared::helpers::ensure_dir(&skill_dir);
        let _ = fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test\n---\n\n# Test skill content that is long enough to pass gate\n",
        );

        let dirs = crate::shared::helpers::list_dirs(&evolved);
        let mut skills = Vec::new();
        for name in &dirs {
            let active = evolved.join(name).join("SKILL.md").is_file();
            skills.push(SkillMeta {
                name: name.clone(),
                origin: "pattern".into(),
                confidence: 0.5,
                project: "test-project".into(),
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                active,
                synthesized: false,
                slow_updates: Vec::new(),
            });
        }
        let manifest = WorkspaceManifest {
            version: "1.0".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            skills,
        };
        let json = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        let manifest_path = evolved.join("manifest.json");
        let _ = fs::write(&manifest_path, &json);

        let content = fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: WorkspaceManifest = serde_json::from_str(&content).expect("parse manifest");
        assert_eq!(parsed.version, "1.0");
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "evo-test-skill");
        assert!(parsed.skills[0].active);
    }

    #[test]
    fn skill_meta_serializes_correctly() {
        let meta = SkillMeta {
            name: "evo-test".into(),
            origin: "pattern".into(),
            confidence: 0.75,
            project: "my-project".into(),
            created: "2026-01-01T00:00:00Z".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            active: true,
            synthesized: false,
            slow_updates: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&meta).expect("serialize");
        let roundtrip: SkillMeta = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(roundtrip.name, "evo-test");
        assert_eq!(roundtrip.origin, "pattern");
        assert!((roundtrip.confidence - 0.75).abs() < f64::EPSILON);
        assert_eq!(roundtrip.project, "my-project");
        assert!(roundtrip.active);
    }

    #[test]
    fn write_skill_with_meta_creates_both_files() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let skill_dir = tmp.path().join("evo-test-skill");
        crate::shared::helpers::ensure_dir(&skill_dir);
        let _ = fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test\n---\n\n# Content\n",
        );

        let meta = SkillMeta {
            name: "evo-test-skill".into(),
            origin: "weak_tool".into(),
            confidence: 0.6,
            project: "test-project".into(),
            created: "2026-01-01T00:00:00Z".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            active: true,
            synthesized: false,
            slow_updates: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&meta).expect("serialize meta");
        let _ = fs::write(skill_dir.join("meta.json"), &json);

        assert!(skill_dir.join("SKILL.md").is_file(), "SKILL.md must exist");
        assert!(
            skill_dir.join("meta.json").is_file(),
            "meta.json must exist"
        );

        let meta_content = fs::read_to_string(skill_dir.join("meta.json")).expect("read meta");
        let parsed: SkillMeta = serde_json::from_str(&meta_content).expect("parse meta");
        assert_eq!(parsed.name, "evo-test-skill");
        assert_eq!(parsed.origin, "weak_tool");
        assert!((parsed.confidence - 0.6).abs() < f64::EPSILON);
        assert!(parsed.active);
    }

    // ── R1: Negative Feedback Buffer tests ──────────────

    #[test]
    fn rejected_buffer_load_save_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("rejected_buffer.json");
        let buf = RejectedBuffer {
            entries: vec![RejectedEntry {
                name: "evo-test".into(),
                reason: "low_confidence".into(),
                timestamp: "2026-06-08T12:00:00Z".into(),
                confidence: 0.1,
                origin: "pattern".into(),
            }],
        };
        let json = serde_json::to_string_pretty(&buf).expect("serialize");
        let _ = fs::write(&path, &json);
        let loaded: RejectedBuffer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].name, "evo-test");
        assert_eq!(loaded.entries[0].reason, "low_confidence");
    }

    #[test]
    fn curate_proposal_skips_rejected() {
        let proposal = SkillProposal {
            name: "evo-some-rejected-skill".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.8,
            rationale: "test".into(),
        };
        let buf = RejectedBuffer {
            entries: vec![RejectedEntry {
                name: "evo-some-rejected-skill".into(),
                reason: "low_confidence".into(),
                timestamp: "2026-06-08T12:00:00Z".into(),
                confidence: 0.1,
                origin: "pattern".into(),
            }],
        };
        let action = curate_proposal(&proposal, &[], &buf);
        assert!(
            matches!(action, ProposalAction::Skip),
            "rejected skill should be skipped"
        );
    }

    #[test]
    fn date_to_ordinal_basic() {
        // 2026-01-01 should give a consistent ordinal
        let o1 = date_to_ordinal(2026, 1, 1);
        let o2 = date_to_ordinal(2026, 1, 2);
        assert_eq!(o2 - o1, 1, "consecutive days should differ by 1");
    }

    #[test]
    fn date_to_ordinal_month_boundary() {
        let jan31 = date_to_ordinal(2026, 1, 31);
        let feb1 = date_to_ordinal(2026, 2, 1);
        assert_eq!(feb1 - jan31, 1, "Jan 31 → Feb 1 should be 1 day");
    }

    #[test]
    fn rejected_buffer_dedup_updates_existing() {
        let mut buf = RejectedBuffer::default();
        buf.entries.push(RejectedEntry {
            name: "evo-test".into(),
            reason: "original".into(),
            timestamp: "2026-06-01T00:00:00Z".into(),
            confidence: 0.3,
            origin: "pattern".into(),
        });
        // Simulate add_rejected with same name — should update, not duplicate
        if let Some(existing) = buf.entries.iter_mut().find(|e| e.name == "evo-test") {
            existing.reason = "updated".into();
            existing.timestamp = "2026-06-08T00:00:00Z".into();
        }
        assert_eq!(buf.entries.len(), 1);
        assert_eq!(buf.entries[0].reason, "updated");
    }

    #[test]
    fn apply_outcome_carries_manifest_for_applied_edit() {
        // A: an AddSkill edit that validates + applies (clean metrics) emits
        // exactly one manifest, and applied == 1.
        let _ = std::fs::remove_dir_all(
            crate::shared::paths::evolved_dir().join("evo-unit-test-skill"),
        );
        let edit = HarnessEdit::AddSkill {
            name: "evo-unit-test-skill".into(),
            content:
                "---\nname: evo-unit-test-skill\n---\nbody that is long enough to pass gating\n"
                    .into(),
            origin: "type_error".into(),
            confidence: 0.8,
        };
        let plan = SkillEditPlan {
            edits: vec![edit],
            counters: crate::evolve::skills::PromotionCounter::default(),
            promoted_count: 1,
        };
        let metrics = crate::shared::evolution::Metrics::default();
        let outcome = apply_skill_edits(&plan, &metrics);
        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.manifests.len(), 1);
        assert_eq!(outcome.manifests[0].target, "evo-unit-test-skill");
        // cleanup the skill dir we wrote
        let _ = std::fs::remove_dir_all(
            crate::shared::paths::evolved_dir().join("evo-unit-test-skill"),
        );
    }

    #[test]
    fn critic_reject_skips_edit_and_emits_no_manifest() {
        // A: when reward hacking is suspected, a manifest claiming a score lift
        // is rejected by the Critic → the edit is skipped, no manifest emitted.
        let _ = std::fs::remove_dir_all(
            crate::shared::paths::evolved_dir().join("evo-unit-test-rejected"),
        );
        let edit = HarnessEdit::AddSkill {
            name: "evo-unit-test-rejected".into(),
            content: "---\nname: evo-unit-test-rejected\n---\nbody\n".into(),
            origin: "type_error".into(),
            confidence: 0.8,
        };
        let plan = SkillEditPlan {
            edits: vec![edit],
            counters: crate::evolve::skills::PromotionCounter::default(),
            promoted_count: 1,
        };
        let metrics = crate::shared::evolution::Metrics {
            reward_hacking_suspected: true,
            ..crate::shared::evolution::Metrics::default()
        };
        let outcome = apply_skill_edits(&plan, &metrics);
        assert_eq!(outcome.applied, 0);
        assert!(outcome.manifests.is_empty());
        // nothing was written
        assert!(
            !crate::shared::paths::evolved_dir()
                .join("evo-unit-test-rejected")
                .exists()
        );
    }
}
