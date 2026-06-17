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
pub(crate) struct SlowUpdateEntry {
    epoch_class: String,
    score: f64,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SkillSlowMeta {
    slow_updates: Vec<SlowUpdateEntry>,
}

const MAX_SLOW_UPDATES: usize = 20;

/// Append a slow update entry to each evolved skill's meta.json.
/// The `slow_updates` array is capped at `MAX_SLOW_UPDATES` entries.
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
        let mut meta: SkillSlowMeta = read_json(&meta_path, SkillSlowMeta::default());
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
pub(crate) fn apply_skill_edits(plan: &SkillEditPlan) -> u64 {
    let mut applied = 0u64;
    for edit in &plan.edits {
        if edit.validate().is_err() {
            continue;
        }
        if matches!(edit.apply(), crate::evolve::edits::EditOutcome::Applied) {
            applied += 1;
        }
    }
    applied
}

pub fn seed_smart_skills(analysis: &SessionAnalysis, existing: &[String]) -> u64 {
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
        return 0;
    }

    let full_seeding = avg_score < CONFIG.pattern.graduated_scope_moderate;
    if !full_seeding {
        hint(
            "reflect",
            &format!("Graduated Scope: moderate seeding (avg_score={avg_score:.3})"),
        );
    }

    let mut counters = load_promotion_counters();
    let plan = plan_skill_edits(analysis, existing, &mut counters);
    let seeded = apply_skill_edits(&plan);

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
    seeded
}

pub fn sanitize_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.starts_with('.')
        && name.len() < 64
}

pub fn write_skill_with_meta(name: &str, content: &str, origin: &str, confidence: f64) {
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
        prompt_tuning_history: vec![],
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
                prompt_tuning_history: vec![],
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

// ── Prompt Auto-Tuning (#49) ─────────────────────────

/// One tuning mutation applied to an evolved skill's SKILL.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTuningEntry {
    pub timestamp: String,
    pub score_before: f64,
    pub section: String,
    /// Updated on the next session that observes this skill.
    pub score_after: Option<f64>,
}

// Prompt auto-tuning constants and functions.
// These are called by the evolve loop after metrics analysis.
// Allow dead_code until the evolve loop integration lands.
#[allow(dead_code)]
const MAX_TUNING_HISTORY: usize = 10;
#[allow(dead_code)]
const TUNING_DECLINE_LIMIT: usize = 3;

/// Append a tuning section to an evolved skill's SKILL.md.
/// **Never modifies or deletes existing content** — only appends after a delimiter.
#[allow(dead_code)]
/// Public entry point for typed edits (R4 HarnessEdit::ModifySkill).
/// Delegates to the private implementation.
pub fn append_tuning_section_pub(name: &str, section: &str) {
    append_tuning_section(name, section)
}

fn append_tuning_section(name: &str, section: &str) {
    let dir = evolved_dir().join(name);
    let skill_file = dir.join("SKILL.md");
    let existing = fs::read_to_string(&skill_file).unwrap_or_default();

    // Delimiter marks auto-tuned content
    let delimiter = "\n\n---\n<!-- auto-tuned -->\n";
    if existing.contains("<!-- auto-tuned -->") {
        // Already has a tuning section — replace only the last one
        let idx = existing.rfind("<!-- auto-tuned -->").unwrap_or(0);
        let base = existing[..idx].trim_end_matches('\n');
        let updated = format!("{base}{delimiter}{section}\n");
        let _ = fs::write(&skill_file, sanitize_skill_content(&updated));
    } else {
        let updated = format!("{existing}{delimiter}{section}\n");
        let _ = fs::write(&skill_file, sanitize_skill_content(&updated));
    }
}

/// Strip all auto-tuned sections, restoring the original SKILL.md content.
#[allow(dead_code)]
fn strip_tuning_sections(name: &str) -> Option<String> {
    let dir = evolved_dir().join(name);
    let skill_file = dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_file).ok()?;
    if let Some(idx) = content.find("\n\n---\n<!-- auto-tuned -->") {
        let original = content[..idx].to_string();
        let _ = fs::write(&skill_file, sanitize_skill_content(&original));
        Some(original)
    } else {
        None
    }
}

/// Generate a tuning section based on the performance gap and failure patterns.
#[allow(dead_code)]
fn build_tuning_section(skill_name: &str, score_with: f64, score_without: f64) -> String {
    let gap = score_without - score_with;
    let guidance = if gap > 0.3 {
        "This skill is significantly underperforming. Consider:\n\
         1. Narrowing the trigger conditions\n\
         2. Adding more specific remediation steps\n\
         3. Including explicit anti-patterns to avoid"
    } else if gap > 0.1 {
        "This skill has room for improvement:\n\
         1. Review recent failure patterns\n\
         2. Add targeted guidance for common pitfalls"
    } else {
        "Minor tuning applied. Monitor performance over next session."
    };
    format!(
        "## Auto-Tuning (session)\n\
         \n\
         Performance gap: {gap:.3} (with={score_with:.3}, without={score_without:.3})\n\
         \n\
         {guidance}\n\
         \n\
         Skill: {skill_name}"
    )
}

/// Check all evolved skills and auto-tune those that are underperforming.
/// Returns the number of skills tuned.
#[allow(dead_code)]
pub fn auto_tune_skills(metrics: &serde_json::Value) -> usize {
    let evolved = evolved_dir();
    if !evolved.is_dir() {
        return 0;
    }

    let mut tuned = 0;
    for name in list_dirs(&evolved) {
        let meta_path = evolved.join(&name).join("meta.json");
        if !meta_path.is_file() {
            continue;
        }

        // Read current meta
        let mut meta: SkillMeta = read_json(&meta_path, SkillMeta::default());

        // Get A/B scores from metrics
        let (score_with, score_without) = get_skill_scores(metrics, &name);

        // Only tune if we have enough data and skill is underperforming
        if score_with <= 0.0 || score_without <= 0.0 {
            continue;
        }
        if score_with >= score_without {
            // Skill is performing well — update score_after on last tuning entry
            if let Some(last) = meta.prompt_tuning_history.last_mut() {
                last.score_after = Some(score_with);
            }
            write_meta(&meta_path, &meta);
            continue;
        }

        // Check if we should rollback (3 consecutive declines)
        let decline_streak = count_consecutive_declines(&meta.prompt_tuning_history);
        if decline_streak >= TUNING_DECLINE_LIMIT {
            hint(
                "reflect",
                &format!(
                    "Prompt tuning rollback: {name} declined {decline_streak} sessions in a row"
                ),
            );
            strip_tuning_sections(&name);
            meta.prompt_tuning_history.clear();
            write_meta(&meta_path, &meta);
            continue;
        }

        // Generate and apply tuning
        let section = build_tuning_section(&name, score_with, score_without);
        append_tuning_section(&name, &section);

        meta.prompt_tuning_history.push(PromptTuningEntry {
            timestamp: now_iso(),
            score_before: score_with,
            section: section.clone(),
            score_after: None,
        });

        // Cap history
        if meta.prompt_tuning_history.len() > MAX_TUNING_HISTORY {
            let start = meta.prompt_tuning_history.len() - MAX_TUNING_HISTORY;
            meta.prompt_tuning_history = meta.prompt_tuning_history[start..].to_vec();
        }

        write_meta(&meta_path, &meta);
        tuned += 1;
    }
    tuned
}

/// Extract A/B scores from metrics.json for a given skill.
#[allow(dead_code)]
fn get_skill_scores(metrics: &serde_json::Value, skill_name: &str) -> (f64, f64) {
    let attr = metrics
        .get("skill_attribution")
        .and_then(|a| a.get(skill_name));
    let score_with = attr
        .and_then(|a| a.get("avg_score_with"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let score_without = metrics
        .get("skill_attribution")
        .and_then(|a| a.get(skill_name))
        .and_then(|a| a.get("avg_score_without"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    (score_with, score_without)
}

/// Count how many consecutive tuning entries show decline (score_after < score_before).
#[allow(dead_code)]
fn count_consecutive_declines(history: &[PromptTuningEntry]) -> usize {
    history
        .iter()
        .rev()
        .take_while(|e| e.score_after.is_some_and(|after| after < e.score_before))
        .count()
}

/// Write meta.json for a skill.
#[allow(dead_code)]
fn write_meta(path: &std::path::Path, meta: &SkillMeta) {
    if let Ok(json) = serde_json::to_string_pretty(meta) {
        let _ = fs::write(path, json);
    }
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
    #[serde(default)]
    pub prompt_tuning_history: Vec<PromptTuningEntry>,
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
                prompt_tuning_history: vec![],
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
            prompt_tuning_history: vec![],
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
            prompt_tuning_history: vec![],
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

    // ── Prompt Auto-Tuning tests (#49) ──────────────────

    #[test]
    fn prompt_tuning_entry_serializes() {
        let entry = PromptTuningEntry {
            timestamp: "2026-06-09T00:00:00Z".into(),
            score_before: 0.5,
            section: "## Auto-Tuning".into(),
            score_after: Some(0.6),
        };
        let json = serde_json::to_string_pretty(&entry).expect("serialize");
        let rt: PromptTuningEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rt.timestamp, "2026-06-09T00:00:00Z");
        assert!((rt.score_before - 0.5).abs() < f64::EPSILON);
        assert_eq!(rt.score_after, Some(0.6));
    }

    #[test]
    fn skill_meta_with_tuning_history_roundtrips() {
        let meta = SkillMeta {
            name: "evo-test".into(),
            origin: "pattern".into(),
            confidence: 0.5,
            project: "test".into(),
            created: "2026-01-01T00:00:00Z".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            active: true,
            prompt_tuning_history: vec![PromptTuningEntry {
                timestamp: "2026-06-09T00:00:00Z".into(),
                score_before: 0.4,
                section: "test".into(),
                score_after: None,
            }],
        };
        let json = serde_json::to_string_pretty(&meta).expect("serialize");
        let rt: SkillMeta = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rt.prompt_tuning_history.len(), 1);
        assert!((rt.prompt_tuning_history[0].score_before - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn skill_meta_deserializes_without_tuning_history() {
        // Backward compat: old meta.json without prompt_tuning_history field
        let json = r#"{"name":"evo-test","origin":"pattern","confidence":0.5,"project":"test","created":"2026-01-01","updated":"2026-01-01","active":true}"#;
        let meta: SkillMeta = serde_json::from_str(json).expect("deserialize old format");
        assert_eq!(meta.name, "evo-test");
        assert!(meta.prompt_tuning_history.is_empty());
    }

    #[test]
    fn consecutive_declines_counted_correctly() {
        let history = vec![
            PromptTuningEntry {
                timestamp: "2026-06-07".into(),
                score_before: 0.5,
                section: "s1".into(),
                score_after: Some(0.6), // improvement
            },
            PromptTuningEntry {
                timestamp: "2026-06-08".into(),
                score_before: 0.6,
                section: "s2".into(),
                score_after: Some(0.4), // decline
            },
            PromptTuningEntry {
                timestamp: "2026-06-09".into(),
                score_before: 0.4,
                section: "s3".into(),
                score_after: Some(0.3), // decline
            },
            PromptTuningEntry {
                timestamp: "2026-06-10".into(),
                score_before: 0.3,
                section: "s4".into(),
                score_after: Some(0.2), // decline
            },
        ];
        assert_eq!(count_consecutive_declines(&history), 3);
    }

    #[test]
    fn no_declines_when_improving() {
        let history = vec![
            PromptTuningEntry {
                timestamp: "2026-06-08".into(),
                score_before: 0.5,
                section: "s1".into(),
                score_after: Some(0.6),
            },
            PromptTuningEntry {
                timestamp: "2026-06-09".into(),
                score_before: 0.6,
                section: "s2".into(),
                score_after: Some(0.7),
            },
        ];
        assert_eq!(count_consecutive_declines(&history), 0);
    }

    #[test]
    fn no_declines_when_score_after_is_none() {
        let history = vec![PromptTuningEntry {
            timestamp: "2026-06-09".into(),
            score_before: 0.5,
            section: "s1".into(),
            score_after: None, // no data yet
        }];
        assert_eq!(count_consecutive_declines(&history), 0);
    }

    #[test]
    fn build_tuning_section_contains_gap() {
        let section = build_tuning_section("evo-test", 0.3, 0.7);
        assert!(section.contains("0.400"), "should show gap: {section}");
        assert!(section.contains("with=0.3"));
        assert!(section.contains("without=0.7"));
    }

    #[test]
    fn get_skill_scores_extracts_from_metrics() {
        let metrics = serde_json::json!({
            "skill_attribution": {
                "evo-test": {
                    "avg_score_with": 0.4,
                    "avg_score_without": 0.8
                }
            }
        });
        let (with, without) = get_skill_scores(&metrics, "evo-test");
        assert!((with - 0.4).abs() < f64::EPSILON);
        assert!((without - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn get_skill_scores_defaults_to_zero() {
        let metrics = serde_json::json!({});
        let (with, without) = get_skill_scores(&metrics, "nonexistent");
        assert_eq!(with, 0.0);
        assert_eq!(without, 0.0);
    }
}
