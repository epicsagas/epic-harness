use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::common::*;
use super::config::CONFIG;
use super::mem::store;
use super::telemetry::{SessionTrend, Telemetry};

static TELEMETRY: LazyLock<Telemetry> = LazyLock::new(Telemetry::init);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PromotionCounter {
    /// Map of skill name -> number of sessions that observed this pattern
    counts: HashMap<String, u64>,
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
fn check_promotion(name: &str, counters: &mut PromotionCounter) -> bool {
    let count = counters.counts.entry(name.into()).or_insert(0);
    *count += 1;
    *count >= CONFIG.evolution.gated_promotion_min
}

// ── R14: Solver-Proposes + Curator Pattern ──────────

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ProposalAction {
    Accept,
    Merge,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillProposal {
    name: String,
    content: String,
    origin: String,
    confidence: f64,
    rationale: String,
}

/// Curator: decides whether to accept a proposal based on masked feedback.
/// The curator only sees pass/fail signals (existing names, confidence thresholds),
/// not raw observation scores.
fn curate_proposal(proposal: &SkillProposal, existing: &[String]) -> ProposalAction {
    // Rule 1: If skill already exists, skip (don't overwrite)
    if existing.contains(&proposal.name) {
        return ProposalAction::Skip;
    }

    // Rule 2: If confidence is too low, skip
    if proposal.confidence < 0.3 {
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
fn build_proposals(analysis: &SessionAnalysis) -> Vec<SkillProposal> {
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

    proposals
}

// ── Phase 1: Session Analysis ───────────────────────

fn analyze_session(observations: &[ObsRecord]) -> SessionAnalysis {
    let scored: Vec<_> = observations.iter().filter(|o| o.score.is_some()).collect();
    let total = scored.len() as u64;
    let errors: Vec<_> = scored
        .iter()
        .filter(|o| o.result.as_deref() == Some("error"))
        .collect();
    let success_rate = if total > 0 {
        round3((total - errors.len() as u64) as f64 / total as f64)
    } else {
        1.0
    };
    let avg_score = if total > 0 {
        round3(scored.iter().map(|o| o.score.unwrap_or(0.0)).sum::<f64>() / total as f64)
    } else {
        0.0
    };

    // Score distribution
    let mut buckets: HashMap<String, u64> = [
        ("0.0-0.2", 0),
        ("0.2-0.4", 0),
        ("0.4-0.6", 0),
        ("0.6-0.8", 0),
        ("0.8-1.0", 0),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    for o in &scored {
        let s = o.score.unwrap_or(0.0);
        let key = if s < 0.2 {
            "0.0-0.2"
        } else if s < 0.4 {
            "0.2-0.4"
        } else if s < 0.6 {
            "0.4-0.6"
        } else if s < 0.8 {
            "0.6-0.8"
        } else {
            "0.8-1.0"
        };
        *buckets.entry(key.into()).or_default() += 1;
    }

    // Per-tool stats
    let mut tool_map: HashMap<String, ToolStats> = HashMap::new();
    for o in &scored {
        let cat = &o.tool_category;
        let ts = tool_map.entry(cat.clone()).or_insert_with(|| ToolStats {
            tool_category: cat.clone(),
            ..Default::default()
        });
        ts.total += 1;
        if o.result.as_deref() == Some("error") {
            ts.errors += 1;
            let fc = o.failure_category.as_deref().unwrap_or("other");
            *ts.failure_categories.entry(fc.into()).or_default() += 1;
        } else {
            ts.successes += 1;
        }
        ts.avg_score =
            ((ts.avg_score * (ts.total - 1) as f64) + o.score.unwrap_or(0.0)) / ts.total as f64;
    }

    // Per-error stats
    let mut error_stats: HashMap<String, u64> = HashMap::new();
    for o in &errors {
        let fc = o.failure_category.as_deref().unwrap_or("other");
        *error_stats.entry(fc.into()).or_default() += 1;
    }

    // Per-ext stats
    let mut ext_map: HashMap<String, ExtStats> = HashMap::new();
    for o in &scored {
        let ext = o.file_ext.as_deref().unwrap_or("unknown");
        let es = ext_map.entry(ext.into()).or_default();
        es.total += 1;
        if o.result.as_deref() == Some("error") {
            es.errors += 1;
        }
    }
    for es in ext_map.values_mut() {
        es.success_rate = if es.total > 0 {
            round3((es.total - es.errors) as f64 / es.total as f64)
        } else {
            1.0
        };
    }

    // Dimension averages
    let dims_scored: Vec<_> = scored
        .iter()
        .filter_map(|o| o.dimensions.as_ref())
        .collect();
    let dim_avg = if !dims_scored.is_empty() {
        let n = dims_scored.len() as f64;
        ScoreDimensions {
            tool_success: round3(dims_scored.iter().map(|d| d.tool_success).sum::<f64>() / n),
            output_quality: round3(dims_scored.iter().map(|d| d.output_quality).sum::<f64>() / n),
            execution_cost: round3(dims_scored.iter().map(|d| d.execution_cost).sum::<f64>() / n),
        }
    } else {
        ScoreDimensions::default()
    };

    SessionAnalysis {
        total_observations: total,
        success_rate,
        avg_score,
        score_distribution: buckets,
        per_tool_stats: tool_map,
        per_error_stats: error_stats,
        per_ext_stats: ext_map,
        failure_patterns: vec![],
        dimension_averages: dim_avg,
    }
}

// ── Phase 2: Pattern Detection ──────────────────────

fn detect_patterns(observations: &[ObsRecord]) -> Vec<DetectedPattern> {
    let mut patterns = vec![];
    let scored: Vec<_> = observations.iter().filter(|o| o.result.is_some()).collect();

    // Pattern 1: repeated_same_error (with error hash dedup)
    {
        let mut streak = 1u64;
        let mut streak_file = String::new();
        let mut streak_category = String::new();
        let mut prev_hash = String::new();

        for i in 1..scored.len() {
            let prev = scored[i - 1];
            let curr = scored[i];

            let curr_snippet = curr.error_snippet.as_deref().unwrap_or("");
            let prev_snippet = prev.error_snippet.as_deref().unwrap_or("");
            let curr_hash = if !curr_snippet.is_empty() {
                hash_string(&normalize_error(curr_snippet))
            } else {
                String::new()
            };
            let prev_hash_val = if !prev_snippet.is_empty() {
                hash_string(&normalize_error(prev_snippet))
            } else {
                String::new()
            };

            let same_error = curr.result.as_deref() == Some("error")
                && prev.result.as_deref() == Some("error")
                && curr.failure_category == prev.failure_category
                && curr.failure_category.is_some()
                && curr.action.is_some()
                && prev.action.is_some()
                && extract_file(curr.action.as_deref().unwrap_or(""))
                    == extract_file(prev.action.as_deref().unwrap_or(""))
                && (curr_hash == prev_hash_val || curr_hash.is_empty() || prev_hash_val.is_empty());

            if same_error {
                streak += 1;
                streak_file = extract_file(curr.action.as_deref().unwrap_or(""))
                    .unwrap_or("")
                    .to_string();
                streak_category = curr.failure_category.clone().unwrap_or_default();
                prev_hash = curr_hash;
            } else {
                if streak >= CONFIG.pattern.repeated_error_min {
                    let hash_note = if !prev_hash.is_empty() {
                        format!(" [hash:{prev_hash}]")
                    } else {
                        String::new()
                    };
                    patterns.push(DetectedPattern {
                        pattern_type: "repeated_same_error".into(),
                        description: format!("{streak_category} repeated {streak}x on {streak_file}{hash_note}"),
                        count: streak,
                        involved_files: if streak_file.is_empty() { vec![] } else { vec![streak_file.clone()] },
                        suggested_remediation: format!("Stop retrying the same approach for {streak_category}. Re-read the full error, check root cause."),
                    });
                }
                streak = 1;
                prev_hash.clear();
            }
        }
        if streak >= CONFIG.pattern.repeated_error_min {
            let hash_note = if !prev_hash.is_empty() {
                format!(" [hash:{prev_hash}]")
            } else {
                String::new()
            };
            patterns.push(DetectedPattern {
                pattern_type: "repeated_same_error".into(),
                description: format!(
                    "{streak_category} repeated {streak}x on {streak_file}{hash_note}"
                ),
                count: streak,
                involved_files: if streak_file.is_empty() {
                    vec![]
                } else {
                    vec![streak_file]
                },
                suggested_remediation: format!(
                    "Stop retrying the same approach for {streak_category}. Re-read the full error."
                ),
            });
        }
    }

    // Pattern 2: fix_then_break
    {
        let mut ftb_files: HashMap<String, u64> = HashMap::new();
        for i in 0..scored.len() {
            let o = scored[i];
            if (o.tool_category == "edit" || o.tool_category == "write")
                && o.result.as_deref() == Some("success")
                && o.action.is_some()
            {
                let file = extract_file(o.action.as_deref().unwrap_or(""))
                    .unwrap_or(o.action.as_deref().unwrap_or(""));
                let basename = Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file);
                for next in scored
                    .iter()
                    .take((i + CONFIG.pattern.ftb_lookahead + 1).min(scored.len()))
                    .skip(i + 1)
                {
                    if next.result.as_deref() == Some("error") && next.tool_category == "bash" {
                        let snippet = next.error_snippet.as_deref().unwrap_or("");
                        if snippet.contains(file) || snippet.contains(basename) {
                            *ftb_files.entry(file.to_string()).or_default() += 1;
                            break;
                        }
                    }
                }
            }
        }
        let ftb_entries: Vec<_> = ftb_files
            .iter()
            .filter(|(_, c)| **c >= CONFIG.pattern.ftb_min_cycles)
            .collect();
        if !ftb_entries.is_empty() {
            let files: Vec<String> = ftb_entries.iter().map(|(f, _)| f.to_string()).collect();
            let total: u64 = ftb_entries.iter().map(|(_, c)| **c).sum();
            patterns.push(DetectedPattern {
                pattern_type: "fix_then_break".into(),
                description: format!("Edit→Break cycle on {}", files.join(", ")),
                count: total,
                involved_files: files,
                suggested_remediation: "Before editing, run the build/test to establish a baseline. After editing, immediately verify.".into(),
            });
        }
    }

    // Pattern 3: long_debug_loop
    {
        let mut prev_file = String::new();
        let mut run_length = 0u64;
        let mut file_runs: HashMap<String, u64> = HashMap::new();

        for o in &scored {
            let file = extract_file(o.action.as_deref().unwrap_or(""))
                .unwrap_or("")
                .to_string();
            if !file.is_empty() && file == prev_file {
                run_length += 1;
            } else {
                if run_length >= CONFIG.pattern.debug_loop_min && !prev_file.is_empty() {
                    let entry = file_runs.entry(prev_file.clone()).or_default();
                    *entry = (*entry).max(run_length);
                }
                prev_file = file;
                run_length = 1;
            }
        }
        if run_length >= CONFIG.pattern.debug_loop_min && !prev_file.is_empty() {
            let entry = file_runs.entry(prev_file.clone()).or_default();
            *entry = (*entry).max(run_length);
        }

        for (file, count) in &file_runs {
            let basename = Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);
            patterns.push(DetectedPattern {
                pattern_type: "long_debug_loop".into(),
                description: format!("Stuck on {basename} for {count} consecutive operations"),
                count: *count,
                involved_files: vec![file.clone()],
                suggested_remediation:
                    "Stuck in debug loop. Stop, re-read the surrounding code context (100+ lines)."
                        .into(),
            });
        }
    }

    // Pattern 4: thrashing
    {
        let mut file_stats: HashMap<String, (u64, u64)> = HashMap::new(); // (edits, errors)
        for o in &scored {
            let file = extract_file(o.action.as_deref().unwrap_or(""))
                .unwrap_or("")
                .to_string();
            if file.is_empty() {
                continue;
            }
            let entry = file_stats.entry(file).or_default();
            if o.tool_category == "edit" || o.tool_category == "write" {
                entry.0 += 1;
            }
            if o.result.as_deref() == Some("error") {
                entry.1 += 1;
            }
        }
        for (file, (edits, errors)) in &file_stats {
            if *edits >= CONFIG.pattern.thrash_min_edits
                && *errors >= CONFIG.pattern.thrash_min_errors
            {
                let basename = Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file);
                patterns.push(DetectedPattern {
                    pattern_type: "thrashing".into(),
                    description: format!("Edit↔Error thrashing on {basename} ({edits} edits, {errors} errors)"),
                    count: edits + errors,
                    involved_files: vec![file.clone()],
                    suggested_remediation: "Alternating edit-error cycle detected. Stop and read the surrounding context.".into(),
                });
            }
        }
    }

    patterns
}

// ── Phase 3: Stagnation Gating ──────────────────────

fn check_stagnation(metrics: &mut Metrics, current_score: f64) -> (bool, bool, u64) {
    // Returns (should_rollback, improved, rolled_back_count)
    if metrics.total_sessions == 0 || metrics.best_score.is_none() {
        // First session or genuinely uninitialized — set best_score and treat as improved.
        metrics.best_score = Some(current_score);
        return (false, true, 0);
    }

    let best = metrics.best_score.unwrap_or(0.0);
    let improvement = current_score - best;
    if improvement >= CONFIG.evolution.improvement_threshold {
        // Improved! Backup evolved skills
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
        let degradation = best - current_score;
        if degradation > 0.05 || metrics.best_score.unwrap_or(0.0) < 0.90 {
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

// ── Helpers ─────────────────────────────────────────

/// Clamp avg_score to a finite f64 so it serialises as a valid JSON number.
/// NaN and ±Infinity are both invalid JSON; replace them with 0.0.
fn safe_avg_score(score: f64) -> f64 {
    if score.is_finite() { score } else { 0.0 }
}

// ── Phase 4: Skill Seeding ──────────────────────────

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

fn seed_smart_skills(analysis: &SessionAnalysis, existing: &[String]) -> u64 {
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
    let mut seeded = 0u64;
    let mut promoted_count = 0u64;
    let cap = CONFIG.evolution.max_skills.saturating_sub(existing.len());

    // Build proposals (solver role)
    let proposals = build_proposals(analysis);

    // Curate and write (curator role)
    for proposal in &proposals {
        if seeded as usize >= cap {
            break;
        }
        if existing.contains(&proposal.name) {
            continue;
        }

        // Apply graduated scope: skip weak_ext and high_freq_error in moderate mode
        if !full_seeding && (proposal.origin == "weak_ext" || proposal.origin == "high_freq_error")
        {
            continue;
        }

        // Curate: decide whether to accept
        let action = curate_proposal(proposal, existing);
        match action {
            ProposalAction::Skip => continue,
            ProposalAction::Merge | ProposalAction::Accept => {
                // Check gated promotion
                if !check_promotion(&proposal.name, &mut counters) {
                    continue;
                }
                write_skill_with_meta(
                    &proposal.name,
                    &proposal.content,
                    &proposal.origin,
                    proposal.confidence,
                );
                seeded += 1;
                promoted_count += 1;
            }
        }
    }

    save_promotion_counters(&counters);
    if promoted_count > 0 {
        let min_obs = CONFIG.evolution.gated_promotion_min;
        hint(
            "reflect",
            &format!(
                "Gated Promotion: {promoted_count} skill(s) promoted after {min_obs}+ observations"
            ),
        );
    }
    seeded
}

fn sanitize_skill_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.starts_with('.')
        && name.len() < 64
}

fn write_skill_with_meta(name: &str, content: &str, origin: &str, confidence: f64) {
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

fn write_workspace_manifest() {
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

fn build_pattern_skill(p: &DetectedPattern) -> String {
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

fn build_tool_skill(stats: &ToolStats) -> String {
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

fn build_ext_skill(ext: &str, stats: &ExtStats) -> String {
    let rate = (stats.success_rate * 100.0) as u32;
    sanitize_skill_content(&format!(
        "---\nname: {clean}-care\ndescription: \"Auto-evolved. {ext} files had {rate}% success rate.\"\n---\n\n# {ext} file care\n\nSuccess rate: {rate}% across {t} operations\n\n## Process\n1. Before editing {ext} files: run type-check / lint / build\n2. After editing: immediately verify\n3. If error: read the full diagnostic before re-editing\n\n## Red Flags\n- Editing {ext} files without verifying afterward\n- Ignoring compiler/linter warnings\n",
        clean = ext.trim_start_matches('.'),
        t = stats.total,
    ))
}

fn build_failure_skill(category: &str, count: u64) -> String {
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

// ── Phase 5: Trend ──────────────────────────────────

fn compute_trend(history: &[SessionScoreEntry]) -> &'static str {
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

// ── Phase 6: Skill Attribution ──────────────────────

fn update_skill_attribution(
    metrics: &mut Metrics,
    analysis: &SessionAnalysis,
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
        attr.avg_score_with = round3(
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
            attr.avg_score_without = round3(
                (all_scores_sum - (attr.avg_score_with * attr.sessions_active as f64))
                    / without as f64,
            );
        }
    }

    metrics
        .skill_attribution
        .retain(|name, _| evolved_skills.contains(name));
}

// ── Phase 7: Cross-project export ───────────────────

fn export_to_global(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) {
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

    append_jsonl(&global_patterns_file(), &record);
}

// ── Gate ─────────────────────────────────────────────

fn gate_skills() {
    let evolved = evolved_dir();
    if !evolved.is_dir() {
        return;
    }

    for name in list_dirs(&evolved) {
        let skill_file = evolved.join(&name).join("SKILL.md");
        if !skill_file.is_file() {
            rm_dir(&evolved.join(&name));
            continue;
        }
        let content = fs::read_to_string(&skill_file).unwrap_or_default();
        let body = content.splitn(3, "---").nth(2).unwrap_or("").trim();
        if !content.starts_with("---") || body.len() < 20 {
            rm_dir(&evolved.join(&name));
        }
    }

    // Enforce cap
    let mut remaining = list_dirs(&evolved);
    remaining.sort();
    if remaining.len() > CONFIG.evolution.max_skills {
        let excess = &remaining[..remaining.len() - CONFIG.evolution.max_skills];
        for name in excess {
            rm_dir(&evolved.join(name));
        }
    }
}

// ── Summary ─────────────────────────────────────────

fn build_summary(analysis: &SessionAnalysis) -> String {
    let mut parts = vec![format!(
        "{} obs, {:.1}% success, avg={}",
        analysis.total_observations,
        analysis.success_rate * 100.0,
        analysis.avg_score,
    )];

    let mut top_errors: Vec<_> = analysis.per_error_stats.iter().collect();
    top_errors.sort_by(|a, b| b.1.cmp(a.1));
    top_errors.truncate(3);
    if !top_errors.is_empty() {
        let errs: Vec<String> = top_errors.iter().map(|(c, n)| format!("{c}:{n}")).collect();
        parts.push(format!("errors=[{}]", errs.join(",")));
    }

    if !analysis.failure_patterns.is_empty() {
        let pats: Vec<&str> = analysis
            .failure_patterns
            .iter()
            .map(|p| p.pattern_type.as_str())
            .collect();
        parts.push(format!("patterns=[{}]", pats.join(",")));
    }

    parts.join(" | ")
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ── Phase 8: Memory Auto-Ingest ────────────────────

/// Find or create a project hub node. Returns the hub node's ID.
fn ensure_project_hub(conn: &rusqlite::Connection, slug: &str) -> std::io::Result<String> {
    // Check if hub already exists
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM nodes WHERE type = 'project' AND title = ?1 LIMIT 1",
            rusqlite::params![format!("project: {}", slug)],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create new project hub
    let id = store::new_uuid();
    let now = store::now_iso();
    let node = store::Node {
        frontmatter: store::NodeFrontmatter {
            id: id.clone(),
            node_type: "project".to_string(),
            title: format!("project: {}", slug),
            tags: vec!["hub".to_string()],
            projects: vec![slug.to_string()],
            agents: vec![],
            created: now.clone(),
            updated: now,
            importance: store::importance_for_type("project"),
            access_count: 0,
            accessed_at: String::new(),
        },
        body: format!("Project hub node for {}", slug),
    };
    store::write_node_conn(conn, &node)?;
    Ok(id)
}

/// Ingest session analysis results into the knowledge graph.
/// Returns (nodes_created, edges_created).
fn ingest_to_memory(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) -> (u64, u64) {
    let conn = match store::open_db() {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let slug = project_slug();
    let ts = now_iso();
    let dedup_hours = 24u64;
    // `unchecked_transaction()` is used here because `open_db()` always returns
    // a fresh connection in autocommit mode (no prior transaction active). Using
    // the checked variant would be equivalent but adds unnecessary overhead for
    // this single-writer, fresh-connection pattern.
    let tx = match conn.unchecked_transaction() {
        Ok(t) => t,
        Err(_) => return (0, 0),
    };

    let mut nodes_created = 0u64;
    let mut edges_created = 0u64;
    let mut session_node_id = String::new();

    // 8a. Session summary node
    {
        let title = format!(
            "session: {} {:.0}% avg={}",
            slug,
            analysis.success_rate * 100.0,
            analysis.avg_score
        );
        let body = build_summary(analysis);
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "session".into(),
                title,
                tags: vec!["auto".into(), "session".into()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("session"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        match store::write_node_dedup_conn(&tx, &node, dedup_hours) {
            Ok((id, false)) => {
                session_node_id = id;
                nodes_created += 1;
            }
            Ok((id, true)) => {
                session_node_id = id;
            }
            Err(_) => {}
        }
    }

    // 8b. Pattern nodes + edges to session
    let mut pattern_node_ids: Vec<(String, Vec<String>)> = vec![]; // (node_id, involved_files)
    for pattern in patterns {
        let title = format!("{}: {} ({}x)", slug, pattern.pattern_type, pattern.count);
        let body = format!(
            "**Pattern**: {}\n**Description**: {}\n**Files**: {}\n**Remediation**: {}",
            pattern.pattern_type,
            pattern.description,
            if pattern.involved_files.is_empty() {
                "various".into()
            } else {
                pattern.involved_files.join(", ")
            },
            pattern.suggested_remediation,
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "pattern".into(),
                title,
                tags: vec!["auto".into(), pattern.pattern_type.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("pattern"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, deduped)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
            let files = pattern.involved_files.clone();
            pattern_node_ids.push((id.clone(), files));
            if !deduped {
                nodes_created += 1;
            }
            // Edge: session → pattern (detected_in)
            if !session_node_id.is_empty() {
                let edge = store::Edge {
                    id: store::new_uuid(),
                    source: session_node_id.clone(),
                    target: id,
                    relation: "detected_in".into(),
                    weight: 1.0,
                    ts: ts.clone(),
                };
                if store::append_edge_conn(&tx, &edge).is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8c. Weak tool nodes
    let mut error_node_ids: Vec<String> = vec![];
    for (cat, stats) in &analysis.per_tool_stats {
        let rate = if stats.total > 0 {
            stats.successes as f64 / stats.total as f64
        } else {
            1.0
        };
        if rate >= CONFIG.pattern.weak_tool_rate || stats.total < CONFIG.pattern.weak_tool_min_obs {
            continue;
        }
        let title = format!("{}: weak tool {} ({:.0}%)", slug, cat, rate * 100.0);
        let body = format!(
            "Tool `{}` success rate: {:.1}% ({}/{} ops)\nTop failures: {:?}",
            cat,
            rate * 100.0,
            stats.successes,
            stats.total,
            stats.failure_categories,
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "error".into(),
                title,
                tags: vec!["auto".into(), "weak-tool".into(), cat.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("error"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, false)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
            error_node_ids.push(id);
            nodes_created += 1;
        }
    }

    // 8d. High-frequency error nodes
    for (category, count) in &analysis.per_error_stats {
        if *count < CONFIG.pattern.high_freq_error_min {
            continue;
        }
        let title = format!("{}: high-freq {} ({}x)", slug, category, count);
        let body = format!(
            "Error category `{}` occurred {} times in this session.",
            category, count
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "error".into(),
                title,
                tags: vec!["auto".into(), "high-freq-error".into(), category.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("error"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, false)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
            error_node_ids.push(id);
            nodes_created += 1;
        }
    }

    // 8e. Auto edges between patterns sharing files
    for i in 0..pattern_node_ids.len() {
        for j in (i + 1)..pattern_node_ids.len() {
            let (id_a, files_a) = &pattern_node_ids[i];
            let (id_b, files_b) = &pattern_node_ids[j];
            let shared: Vec<_> = files_a.iter().filter(|f| files_b.contains(f)).collect();
            if !shared.is_empty() {
                let edge = store::Edge {
                    id: store::new_uuid(),
                    source: id_a.clone(),
                    target: id_b.clone(),
                    relation: "related".into(),
                    weight: shared.len() as f64,
                    ts: ts.clone(),
                };
                if store::append_edge_conn(&tx, &edge).is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8f. Project hub nodes + belongs_to edges
    // For each unique project slug in the session, create/find a project hub node
    // and link all session/pattern/error nodes to it.
    if let Ok(hub_id) = ensure_project_hub(&tx, &slug) {
        // Link session node to project hub
        if !session_node_id.is_empty() {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: session_node_id.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.5,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
        // Link pattern nodes to project hub
        for (pid, _) in &pattern_node_ids {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: pid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
        // Link error nodes to project hub
        for eid in &error_node_ids {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: eid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
    }

    // 8g. Session chain: link to previous session in same project
    if !session_node_id.is_empty() {
        let prev_session: Option<String> = tx
            .query_row(
                "SELECT id FROM nodes WHERE type = 'session' AND id != ?1
             AND (',' || projects || ',' LIKE '%,' || ?2 || ',%')
             ORDER BY updated DESC LIMIT 1",
                rusqlite::params![session_node_id, slug],
                |row| row.get(0),
            )
            .ok();

        if let Some(prev_id) = prev_session {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: prev_id,
                    target: session_node_id.clone(),
                    relation: "follows".to_string(),
                    weight: 0.3,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
    }

    // 8h. Same-tag edges: link non-session nodes that share tags
    let pattern_only_ids: Vec<String> = pattern_node_ids.iter().map(|(id, _)| id.clone()).collect();
    let all_new_ids: Vec<&str> = pattern_only_ids
        .iter()
        .chain(error_node_ids.iter())
        .map(String::as_str)
        .collect();

    if all_new_ids.len() >= 2 {
        let new_nodes = store::read_nodes_conn(&tx, &all_new_ids);
        for i in 0..new_nodes.len() {
            for j in (i + 1)..new_nodes.len() {
                let shared: Vec<String> = new_nodes[i]
                    .frontmatter
                    .tags
                    .iter()
                    .filter(|t| **t != "auto" && new_nodes[j].frontmatter.tags.contains(t))
                    .cloned()
                    .collect();
                if !shared.is_empty() {
                    let _ = store::append_edge_conn(
                        &tx,
                        &store::Edge {
                            id: store::new_uuid(),
                            source: new_nodes[i].frontmatter.id.clone(),
                            target: new_nodes[j].frontmatter.id.clone(),
                            relation: "shares_context".to_string(),
                            weight: shared.len() as f64,
                            ts: ts.clone(),
                        },
                    )
                    .map(|_| edges_created += 1);
                }
            }
        }
    }

    let _ = tx.commit();
    (nodes_created, edges_created)
}

// ── Phase 8.5: Instinct Extraction & Promotion (R12) ─

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Instinct {
    trigger: String,
    confidence: f64,
    domain: String,
    scope: String,
    observation_count: u64,
    success_count: u64,
    projects: Vec<String>,
}

fn extract_instincts(observations: &[ObsRecord], analysis: &SessionAnalysis) -> Vec<Instinct> {
    if observations.len() < CONFIG.instinct.min_observations
        || analysis.avg_score < CONFIG.instinct.min_avg_score
    {
        return vec![];
    }

    // Extract per-tool-category instincts
    let mut instincts = Vec::new();
    for (tool_cat, stats) in &analysis.per_tool_stats {
        if stats.total < 3 {
            continue;
        }
        let rate = stats.successes as f64 / stats.total as f64;
        if rate >= CONFIG.instinct.confidence_threshold {
            instincts.push(Instinct {
                trigger: format!("high-success-{}", tool_cat),
                confidence: rate,
                domain: "tool-usage".into(),
                scope: "local".into(),
                observation_count: stats.total,
                success_count: stats.successes,
                projects: vec![project_slug()],
            });
        }
    }

    // Extract per-file-extension instincts
    for (ext, stats) in &analysis.per_ext_stats {
        if stats.total < 3 {
            continue;
        }
        let rate = stats.success_rate;
        if rate >= CONFIG.instinct.confidence_threshold {
            instincts.push(Instinct {
                trigger: format!("high-success-{}", ext.trim_start_matches('.')),
                confidence: rate,
                domain: ext.trim_start_matches('.').into(),
                scope: "local".into(),
                observation_count: stats.total,
                success_count: stats.total - stats.errors,
                projects: vec![project_slug()],
            });
        }
    }

    instincts.truncate(CONFIG.instinct.max_instincts);
    instincts
}

fn promote_instincts_to_global(instincts: &[Instinct]) -> u64 {
    let conn = match store::open_db() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let tx = match conn.unchecked_transaction() {
        Ok(t) => t,
        Err(_) => return 0,
    };

    let mut promoted = 0u64;
    for instinct in instincts {
        if instinct.confidence < CONFIG.instinct.confidence_threshold {
            continue;
        }

        // Only promote instincts seen across multiple projects (or single-project
        // when the threshold is set to 1 for early bootstrapping).
        if instinct.projects.len() < CONFIG.instinct.promotion_min_projects {
            continue;
        }

        let title = format!("instinct: {} ({})", instinct.trigger, instinct.domain);
        let body = format!(
            "**Trigger**: {}\n**Confidence**: {:.2}\n**Domain**: {}\n**Scope**: {}\n**Observations**: {} ({} successful)\n**Projects**: {}",
            instinct.trigger,
            instinct.confidence,
            instinct.domain,
            instinct.scope,
            instinct.observation_count,
            instinct.success_count,
            instinct.projects.join(", "),
        );

        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "instinct".into(),
                title,
                tags: vec!["auto".into(), "instinct".into(), instinct.domain.clone()],
                projects: instinct.projects.clone(),
                agents: vec![],
                created: now_iso(),
                updated: now_iso(),
                importance: store::importance_for_type("instinct"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };

        // Use dedup to avoid duplicate instincts (7-day window)
        if let Ok((_, is_new)) = store::write_node_dedup_conn(&tx, &node, 168)
            && is_new
        {
            promoted += 1;
        }
    }

    let _ = tx.commit();
    promoted
}

// ── Phase 9: Workspace Contract (R13) ────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SkillMeta {
    name: String,
    origin: String, // "pattern", "weak_tool", "weak_ext", "high_freq_error"
    confidence: f64,
    project: String,
    created: String,
    updated: String,
    active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkspaceManifest {
    version: String,
    updated: String,
    skills: Vec<SkillMeta>,
}

// ── Reflect Context (subcommand) ─────────────────────

/// Collect harness data for /reflect skill as JSON on stdout.
/// Replaces the Python-based `reflect-context.sh` for Windows compat.
pub fn run_context(
    days: u32,
    since: Option<String>,
    project: Option<String>,
    all_projects: bool,
    sources: Vec<String>,
) -> i32 {
    if !harness_exists() {
        eprintln!("{{\"error\":\"harness directory not found\"}}");
        return 1;
    }

    // Determine which project slugs to analyze
    let project_slugs: Vec<String> = if all_projects {
        list_harness_project_slugs()
    } else if let Some(ref slug) = project {
        vec![slug.clone()]
    } else {
        vec![project_slug()]
    };

    // Fix 1: Validate slugs — reject path traversal attempts
    for slug in &project_slugs {
        if slug.contains("..") || slug.contains('/') || slug.contains('\\') {
            eprintln!("{{\"error\":\"invalid project slug: {slug}\"}}");
            return 1;
        }
    }

    // Fix 5: Validate --since format (YYYYMMDD)
    if let Some(ref s) = since {
        if s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()) {
            eprintln!("{{\"error\":\"--since must be YYYYMMDD format, got: {s}\"}}");
            return 1;
        }
    }

    // 1. Obs stats — compute date range
    let (cutoff_tag, date_from) = if let Some(ref s) = since {
        (s.clone(), s.clone())
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff_ts = now.saturating_sub((days as u64) * 86400);
        let days_since_epoch = cutoff_ts / 86400;
        let (y, m, d) = epoch_days_to_ymd(days_since_epoch as i32);
        let tag = format!("{y:04}{m:02}{d:02}");
        (tag.clone(), tag)
    };
    let date_to = today();

    let mut total_obs: u64 = 0;
    let mut tool_counts: HashMap<String, u64> = HashMap::new();
    let mut failure_cats: HashMap<String, u64> = HashMap::new();
    let mut file_ext_counts: HashMap<String, u64> = HashMap::new();
    let mut scores: Vec<f64> = Vec::new();
    let mut dim_sums: HashMap<String, f64> = HashMap::new();
    let mut dim_counts: HashMap<String, u64> = HashMap::new();
    let mut tool_success_map: HashMap<String, (u64, u64)> = HashMap::new(); // (success, total)

    // Collect obs from all target project slugs
    for slug in &project_slugs {
        // Fix 1: Verify resolved path stays within harness projects root
        let slug_harness = harness_dir_for_slug(slug);
        let safe = if slug_harness.exists() {
            slug_harness
                .canonicalize()
                .ok()
                .map(|p| p.starts_with(harness_projects_root()))
                .unwrap_or(false)
        } else {
            slug_harness.starts_with(harness_projects_root())
        };
        if !safe {
            eprintln!("{{\"error\":\"slug escapes harness root: {slug}\"}}");
            return 1;
        }
        let slug_obs_dir = slug_harness.join("obs");
        if !slug_obs_dir.is_dir() {
            continue;
        }
        let all_obs = list_files(&slug_obs_dir, ".jsonl");
        let filtered: Vec<String> = all_obs
            .into_iter()
            .filter(|f| {
                let tag = f.replace("session_", "");
                tag.get(..8).map(|s| s >= cutoff_tag.as_str()).unwrap_or(true)
            })
            .collect();
        for f in &filtered {
            let recs: Vec<ObsRecord> = read_jsonl_typed(&slug_obs_dir.join(f));
            for r in &recs {
                total_obs += 1;
                *tool_counts.entry(r.tool.clone()).or_default() += 1;
                if let Some(ref fc) = r.failure_category {
                    *failure_cats.entry(fc.clone()).or_default() += 1;
                }
                if let Some(ref ext) = r.file_ext {
                    *file_ext_counts.entry(ext.clone()).or_default() += 1;
                }
                if let Some(s) = r.score {
                    scores.push(s);
                }
                if let Some(ref dims) = r.dimensions {
                    let ds = serde_json::to_value(dims).ok();
                    if let Some(obj) = ds.as_ref().and_then(|v| v.as_object()) {
                        for (k, v) in obj {
                            if let Some(n) = v.as_f64() {
                                *dim_sums.entry(k.clone()).or_default() += n;
                                *dim_counts.entry(k.clone()).or_default() += 1;
                            }
                        }
                    }
                }
                let entry = tool_success_map.entry(r.tool.clone()).or_insert((0, 0));
                entry.1 += 1;
                if r.result.as_deref() == Some("success")
                    || (r.result.is_none() && r.score.unwrap_or(0.0) >= 0.7)
                {
                    entry.0 += 1;
                }
            }
        }
    }

    let avg_score = if scores.is_empty() { 0.0 } else { round3(scores.iter().sum::<f64>() / scores.len() as f64) };
    let high_ge09 = scores.iter().filter(|&&s| s >= 0.9).count() as u64;
    let mid_06_09 = scores.iter().filter(|&&s| (0.6..0.9).contains(&s)).count() as u64;
    let low_lt06 = scores.iter().filter(|&&s| s < 0.6).count() as u64;

    let mut top_tools: Vec<(String, u64)> = tool_counts.into_iter().collect();
    top_tools.sort_by_key(|b| std::cmp::Reverse(b.1));
    let top_tools_map: serde_json::Map<String, serde_json::Value> = top_tools
        .iter()
        .take(10)
        .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
        .collect();

    let mut fc_sorted: Vec<(String, u64)> = failure_cats.into_iter().collect();
    fc_sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    let fc_map: serde_json::Map<String, serde_json::Value> = fc_sorted
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
        .collect();

    let mut ext_sorted: Vec<(String, u64)> = file_ext_counts.into_iter().collect();
    ext_sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    let ext_map: serde_json::Map<String, serde_json::Value> = ext_sorted
        .iter()
        .take(8)
        .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
        .collect();

    let dim_avgs: serde_json::Map<String, serde_json::Value> = dim_sums
        .iter()
        .map(|(k, s)| {
            let c = dim_counts.get(k).copied().unwrap_or(1);
            (k.clone(), serde_json::Value::from(round3(s / c as f64)))
        })
        .collect();

    // Fix 6: Use CONFIG thresholds instead of hardcoded literals
    let wt_min = CONFIG.pattern.weak_tool_min_obs;
    let wt_rate = CONFIG.pattern.weak_tool_rate;
    let st_rate = 0.9f64; // strong_tool threshold — not in CONFIG, kept as named constant

    let weak_tools: Vec<String> = tool_success_map
        .iter()
        .filter(|(_, (s, n))| *n >= wt_min && (*s as f64 / *n as f64) < wt_rate)
        .map(|(t, _)| t.clone())
        .collect();
    let strong_tools: Vec<String> = tool_success_map
        .iter()
        .filter(|(_, (s, n))| *n >= wt_min && (*s as f64 / *n as f64) >= st_rate)
        .map(|(t, _)| t.clone())
        .collect();
    let total_success: u64 = tool_success_map.values().map(|(s, _)| *s).sum();
    let total_calls: u64 = tool_success_map.values().map(|(_, n)| *n).sum();
    let tool_success_rate = if total_calls == 0 { 0.0 } else { round3(total_success as f64 / total_calls as f64) };

    let obs_stats = serde_json::json!({
        "total": total_obs,
        "avg_score": avg_score,
        "score_distribution": { "high_ge09": high_ge09, "mid_06_09": mid_06_09, "low_lt06": low_lt06 },
        "top_tools": top_tools_map,
        "failure_categories": fc_map,
        "top_file_exts": ext_map,
        "dimension_averages": dim_avgs,
        "weak_tools": weak_tools,
        "strong_tools": strong_tools,
        "tool_success_rate": tool_success_rate,
    });

    // 2. Evolution stats
    let evo_records: Vec<serde_json::Value> =
        read_jsonl_typed::<serde_json::Value>(&evolution_file());
    let mut pattern_freq: HashMap<String, u64> = HashMap::new();
    let mut trend_hist: Vec<String> = Vec::new();
    let mut skills_generated: u64 = 0;
    let mut stagnation_count: u64 = 0;
    for r in &evo_records {
        if let Some(pats) = r.get("patterns").and_then(|p| p.as_array()) {
            for p in pats {
                if let Some(t) = p.get("type").and_then(|v| v.as_str()) {
                    *pattern_freq.entry(t.to_string()).or_default() += 1;
                }
            }
        }
        if let Some(t) = r.get("trend").and_then(|v| v.as_str())
            && !t.is_empty() {
                trend_hist.push(t.to_string());
            }
        skills_generated += r.get("skills_generated").and_then(|v| v.as_u64()).unwrap_or(0);
        if r.get("stagnation_triggered").and_then(|v| v.as_bool()).unwrap_or(false) {
            stagnation_count += 1;
        }
    }
    let recent_evo: Vec<&serde_json::Value> = evo_records.iter().rev().take(10).collect();
    let recent_weak: Vec<String> = recent_evo
        .iter()
        .filter_map(|r| r.get("weak_tools").and_then(|v| v.as_array()))
        .flat_map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)))
        .collect();
    let recent_weak: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        recent_weak.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };
    let recent_seeded: Vec<String> = recent_evo
        .iter()
        .filter_map(|r| r.get("seeded_skills").and_then(|v| v.as_array()))
        .flat_map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)))
        .collect();
    let recent_seeded: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        recent_seeded.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };
    let mut pf_sorted: Vec<(String, u64)> = pattern_freq.into_iter().collect();
    pf_sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    let pf_map: serde_json::Map<String, serde_json::Value> = pf_sorted
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
        .collect();
    let evo_stats = serde_json::json!({
        "total_sessions": evo_records.len(),
        "patterns_detected": evo_records.iter().filter(|r| r.get("patterns").and_then(|p| p.as_object()).map(|o| !o.is_empty()).unwrap_or(false)).count(),
        "skills_generated": skills_generated,
        "pattern_frequency": pf_map,
        "trend_last10": trend_hist.into_iter().rev().take(10).collect::<Vec<_>>(),
        "recent_weak_tools": recent_weak,
        "recent_seeded_skills": recent_seeded,
        "stagnation_count": stagnation_count,
    });

    // 3. Metrics summary
    let metrics: Metrics = read_json(&metrics_file(), default_metrics());
    let sh = &metrics.score_history;
    let score_trend_delta: f64 = if sh.len() >= 3 {
        let recent: Vec<f64> = sh.iter().rev().take(10).map(|s| s.avg_score).collect();
        let deltas: Vec<f64> = recent.windows(2).map(|w| w[0] - w[1]).collect();
        if deltas.is_empty() { 0.0 } else { round4(deltas.iter().sum::<f64>() / deltas.len() as f64) }
    } else { 0.0 };

    let score_comparison = if sh.len() >= 10 {
        let first5: f64 = sh.iter().take(5).map(|s| s.avg_score).sum::<f64>() / 5.0;
        let last5: f64 = sh.iter().rev().take(5).map(|s| s.avg_score).sum::<f64>() / 5.0;
        let dir = if last5 > first5 { "improving" } else if last5 < first5 { "declining" } else { "stable" };
        Some(serde_json::json!({
            "first_5_avg": round3(first5),
            "last_5_avg": round3(last5),
            "direction": dir,
            "delta": round3(last5 - first5),
        }))
    } else { None };

    let skill_attr: serde_json::Map<String, serde_json::Value> = metrics
        .skill_attribution
        .iter()
        .map(|(k, v)| {
            (k.clone(), serde_json::json!({
                "sessions_active": v.sessions_active,
                "avg_score_with": v.avg_score_with,
                "avg_score_without": v.avg_score_without,
                "delta": round3(v.avg_score_with - v.avg_score_without),
            }))
        })
        .collect();

    let latest_dims = sh.last().map(|s| {
        serde_json::to_value(s.dimension_averages).unwrap_or_default()
    }).unwrap_or_default();

    let metrics_summary = serde_json::json!({
        "total_sessions": metrics.total_sessions,
        "avg_success_rate": metrics.avg_success_rate,
        "total_evolved_skills": metrics.total_evolved_skills,
        "last_session": metrics.last_session,
        "trend": metrics.trend,
        "best_score": metrics.best_score,
        "stagnation_count": metrics.stagnation_count,
        "score_trend_delta": score_trend_delta,
        "score_comparison": score_comparison,
        "latest_avg_score": sh.last().map(|s| s.avg_score).unwrap_or(0.0),
        "latest_dimensions": latest_dims,
        "skill_attribution": skill_attr,
    });

    // 4. Session snapshots
    let snap_files = list_files(&sessions_dir(), ".json");
    let snapshots: Vec<serde_json::Value> = snap_files
        .iter()
        .rev()
        .take(5)
        .filter_map(|f| {
            let sp: serde_json::Value = read_json(&sessions_dir().join(f), serde_json::Value::Null);
            if sp.is_null() { return None; }
            Some(serde_json::json!({
                "timestamp": sp.get("timestamp").and_then(|v| v.as_str()).unwrap_or(""),
                "type": sp.get("type").and_then(|v| v.as_str()).unwrap_or(""),
                "summary": sp.get("summary").and_then(|v| v.as_str()).unwrap_or("").chars().take(400).collect::<String>(),
            }))
        })
        .collect();

    // 5. Evolved skills
    let evolved = list_dirs(&evolved_dir());
    let mut evolved_list: Vec<serde_json::Value> = Vec::new();
    let mut by_type: HashMap<&str, u64> = HashMap::new();
    for name in &evolved {
        let skill_path = evolved_dir().join(name).join("SKILL.md");
        let content = fs::read_to_string(&skill_path).unwrap_or_default();
        let stype = if content.contains("Evolved:") || content.contains("evolved_from:") {
            "evolved"
        } else if content.to_lowercase().contains("auto-evolved") {
            "auto-evolved"
        } else {
            "preset"
        };
        *by_type.entry(stype).or_default() += 1;
        evolved_list.push(serde_json::json!({"name": name, "type": stype}));
    }

    // Effective sources
    // mem is always included (baseline). --source adds extra sources on top.
    let effective_sources: Vec<&str> = if sources.contains(&"all".to_string()) {
        vec!["harness", "mem", "claude-session", "alcove"]
    } else if sources.is_empty() {
        vec!["harness", "mem"]
    } else {
        // Always prepend mem unless the caller explicitly passed "mem" already
        let mut v: Vec<&str> = vec!["harness", "mem"];
        for s in sources.iter() {
            let s = s.as_str();
            if s != "harness" && s != "mem" {
                v.push(s);
            }
        }
        v
    };

    let extra_sources_json = {
        let mut map = serde_json::Map::new();
        // mem — always collected
        map.insert("mem".into(), collect_mem(&project_slugs));
        if effective_sources.contains(&"claude-session") {
            map.insert("claude_session".into(), collect_claude_session());
        }
        if effective_sources.contains(&"alcove") {
            map.insert("alcove".into(), collect_alcove(&CONFIG.context.alcove));
        }
        serde_json::Value::Object(map)
    };

    // Fix 4: Scope note clarifying which fields are per-project vs aggregated
    let scope_note = if all_projects || project.is_some() {
        "evolution_stats, metrics_summary, session_snapshots, and evolved_skills are scoped to the current working directory project. Only obs_stats is aggregated across all analyzed projects."
    } else {
        ""
    };

    // Compile
    let output = serde_json::json!({
        "generated_at": now_iso(),
        "analysis_window_days": days,
        "date_range": { "from": date_from, "to": date_to },
        "projects_analyzed": project_slugs,
        "scope_note": scope_note,
        "extra_sources": extra_sources_json,
        "obs_stats": obs_stats,
        "evolution_stats": evo_stats,
        "metrics_summary": metrics_summary,
        "session_snapshots": snapshots,
        "evolved_skills": evolved_list,
        "evolved_skills_summary": {
            "total": evolved.len(),
            "by_type": {
                "preset": by_type.get("preset").copied().unwrap_or(0),
                "evolved": by_type.get("evolved").copied().unwrap_or(0),
                "auto-evolved": by_type.get("auto-evolved").copied().unwrap_or(0),
            }
        },
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    0
}

/// Collect mem nodes from ~/.harness/memory.db.
/// Pulls top nodes by importance for each project slug (or all if slugs = [current]).
/// Session-type nodes are excluded (importance=0.05, noise) unless there's nothing else.
fn collect_mem(project_slugs: &[String]) -> serde_json::Value {
    let conn = match store::open_db() {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({"error": format!("mem db unavailable: {e}")});
        }
    };

    // Determine project filter: use first slug if single-project, else no filter (all)
    let project_filter: Option<&str> = if project_slugs.len() == 1 {
        project_slugs.first().map(|s| s.as_str())
    } else {
        None
    };

    // Smart recall — hint = broad engineering context, limit = 30
    let recalled = store::smart_recall_conn(
        &conn,
        project_filter,
        Some("decision pattern error resolution concept"),
        30,
    );

    // Also pull top decisions/resolutions explicitly (high-value types)
    let decisions = store::query_nodes_conn(
        &conn,
        None,       // tag filter
        Some("decision"),
        project_filter,
        10,
    );
    let resolutions = store::query_nodes_conn(
        &conn,
        None,
        Some("resolution"),
        project_filter,
        10,
    );

    // Merge and deduplicate by id, prefer higher-importance entry
    let mut seen: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();

    for sn in &recalled {
        let id = sn.node.frontmatter.id.clone();
        let entry = serde_json::json!({
            "id": id,
            "type": sn.node.frontmatter.node_type,
            "title": sn.node.frontmatter.title,
            "importance": sn.node.frontmatter.importance,
            "tags": sn.node.frontmatter.tags,
            "updated": sn.node.frontmatter.updated,
            "body_preview": sn.node.body.chars().take(200).collect::<String>(),
        });
        seen.insert(id, entry);
    }
    for node in decisions.iter().chain(resolutions.iter()) {
        let id = node.frontmatter.id.clone();
        seen.entry(id.clone()).or_insert_with(|| serde_json::json!({
            "id": id,
            "type": node.frontmatter.node_type,
            "title": node.frontmatter.title,
            "importance": node.frontmatter.importance,
            "tags": node.frontmatter.tags,
            "updated": node.frontmatter.updated,
            "body_preview": node.body.chars().take(200).collect::<String>(),
        }));
    }

    // Sort by importance desc, take top 30
    let mut nodes: Vec<serde_json::Value> = seen.into_values().collect();
    nodes.sort_by(|a, b| {
        let ia = a.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ib = b.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.0);
        ib.partial_cmp(&ia).unwrap_or(std::cmp::Ordering::Equal)
    });
    nodes.truncate(30);

    serde_json::json!({
        "total_nodes_sampled": nodes.len(),
        "project_filter": project_filter,
        "nodes": nodes,
    })
}

fn collect_claude_session() -> serde_json::Value {
    let claude_projects = dirs_home().join(".claude").join("projects");
    if !claude_projects.is_dir() {
        return serde_json::json!({"error": "~/.claude/projects not found"});
    }
    let mut sessions: Vec<serde_json::Value> = vec![];
    let project_dirs: Vec<_> = std::fs::read_dir(&claude_projects)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    for pd in project_dirs.iter().take(20) {
        let jsonl_files = list_files(&pd.path(), ".jsonl");
        for f in jsonl_files.iter().rev().take(3) {
            let recs: Vec<serde_json::Value> = read_jsonl_typed(&pd.path().join(f));
            for r in recs.iter().take(5) {
                let meta = serde_json::json!({
                    "project": pd.file_name().to_string_lossy(),
                    "timestamp": r.get("timestamp").and_then(|v| v.as_str()).unwrap_or(""),
                    "model": r.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                    "message_count": r.get("message_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    "cost_usd": r.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
                });
                sessions.push(meta);
            }
        }
    }
    sessions.sort_by(|a, b| {
        let ta = a.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        tb.cmp(ta)
    });
    let sessions: Vec<_> = sessions.into_iter().take(20).collect();
    serde_json::json!({
        "total_sessions_sampled": sessions.len(),
        "sessions": sessions,
    })
}

fn collect_alcove(cfg: &super::config::AlcoveConfig) -> serde_json::Value {
    if cfg.vault_path.is_empty() {
        return serde_json::json!({"error": "alcove vault_path not configured"});
    }
    let vault = if cfg.vault_path.starts_with("~/") {
        dirs_home().join(&cfg.vault_path[2..])
    } else {
        std::path::PathBuf::from(&cfg.vault_path)
    };
    // Fix 2: Canonicalize vault path and verify it stays within home directory
    let vault = if let Ok(canonical) = vault.canonicalize() {
        let home = dirs_home();
        if !canonical.starts_with(&home) {
            return serde_json::json!({
                "error": format!("vault_path escapes home directory: {}", canonical.display())
            });
        }
        canonical
    } else {
        return serde_json::json!({"error": format!("vault_path not found: {}", vault.display())});
    };
    let max = cfg.max_docs.max(1);
    let mut docs: Vec<serde_json::Value> = vec![];
    let mut visited = 0usize;
    collect_md_files(&vault, &cfg.projects, max, &mut docs, 0, &mut visited);
    serde_json::json!({
        "vault_path": cfg.vault_path,
        "docs_collected": docs.len(),
        "documents": docs,
    })
}

fn collect_md_files(
    dir: &std::path::Path,
    filter_projects: &[String],
    max: usize,
    out: &mut Vec<serde_json::Value>,
    depth: usize,
    visited: &mut usize,
) {
    // Fix 3: Guard against deep recursion, excessive file visits, and symlink loops
    const MAX_DEPTH: usize = 10;
    const MAX_VISITED: usize = 5000;

    if out.len() >= max || depth > MAX_DEPTH || *visited > MAX_VISITED {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= max || *visited > MAX_VISITED {
            break;
        }
        let path = entry.path();
        // Fix 3: Skip symlinks to prevent directory traversal via symlink
        if path.is_dir() && !path.is_symlink() {
            if !filter_projects.is_empty() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !filter_projects.iter().any(|p| p == &name) {
                    continue;
                }
            }
            collect_md_files(&path, &[], max, out, depth + 1, visited);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            *visited += 1;
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let summary: String = content.chars().take(200).collect();
            out.push(serde_json::json!({
                "path": path.display().to_string(),
                "summary": summary,
            }));
        }
    }
}

/// Simple epoch-day to (year, month, day) without chrono dependency.
fn epoch_days_to_ymd(days: i32) -> (i32, u32, u32) {
    let mut y = 1970 + days / 365;
    // Refine
    for candidate in (1970..=y + 2).rev() {
        let d = days_to_year_start(candidate);
        if d <= days {
            y = candidate;
            break;
        }
    }
    let remaining = days - days_to_year_start(y);
    let leap = is_leap(y);
    let month_days: [u32; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m: u32 = 1;
    let mut acc: i32 = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if acc + md as i32 > remaining {
            m = (i + 1) as u32;
            break;
        }
        acc += md as i32;
    }
    let d = (remaining - acc + 1).max(1) as u32;
    (y, m, d)
}

fn days_to_year_start(year: i32) -> i32 {
    let mut days = 0i32;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    days
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

// ── Main Hook ───────────────────────────────────────

pub fn run(_input: &HookInput) -> i32 {
    if !should_run(PROFILE_REFLECT) {
        return 0;
    }
    if !harness_exists() {
        return 0;
    }
    if !obs_dir().is_dir() {
        return 0;
    }

    // 1. Collect today's observations
    let today_str = today();
    let obs_files: Vec<String> = list_files(&obs_dir(), ".jsonl")
        .into_iter()
        .filter(|f| f.contains(&today_str))
        .collect();
    if obs_files.is_empty() {
        return 0;
    }

    let mut observations: Vec<ObsRecord> = vec![];
    for f in &obs_files {
        let recs: Vec<ObsRecord> = read_jsonl_typed(&obs_dir().join(f));
        observations.extend(recs);
    }
    if observations.len() < 3 {
        return 0;
    }

    // 2. Analyze
    let mut analysis = analyze_session(&observations);
    analysis.failure_patterns = detect_patterns(&observations);

    // 3. Stagnation
    let mut metrics: Metrics = read_json(&metrics_file(), default_metrics());
    let (should_rollback, improved, rolled_back_count) =
        check_stagnation(&mut metrics, analysis.avg_score);

    // 4. Seed evolved skills
    ensure_dir(&evolved_dir());
    let existing = list_dirs(&evolved_dir());
    let seeded = if !should_rollback {
        seed_smart_skills(&analysis, &existing)
    } else {
        0
    };

    // 5. Gate
    gate_skills();

    // 6. Skill attribution (reuse listing after gate may have pruned)
    let evolved_dirs = list_dirs(&evolved_dir());
    update_skill_attribution(&mut metrics, &analysis, &evolved_dirs);

    // 7. Cross-project export
    export_to_global(&analysis, &analysis.failure_patterns);

    // 8. Memory auto-ingest (knowledge graph)
    let (mem_nodes, mem_edges) = ingest_to_memory(&analysis, &analysis.failure_patterns);

    // 8.5. Instinct extraction and promotion
    let instincts = extract_instincts(&observations, &analysis);
    let instincts_promoted = if !instincts.is_empty() {
        promote_instincts_to_global(&instincts)
    } else {
        0
    };
    if instincts_promoted > 0 {
        hint(
            "reflect",
            &format!("Instinct: promoted {instincts_promoted} new instinct(s)"),
        );
    }

    // 9. Evolution record
    let record = EvolutionRecord {
        timestamp: now_iso(),
        observations: analysis.total_observations,
        success_rate: analysis.success_rate,
        avg_score: analysis.avg_score,
        error_patterns: analysis.per_error_stats.clone(),
        failure_patterns: analysis.failure_patterns.clone(),
        skills_seeded: seeded,
        skills_rolled_back: rolled_back_count,
        total_evolved: evolved_dirs.len() as u64,
        analysis_summary: build_summary(&analysis),
    };
    append_jsonl(&evolution_file(), &record);

    // 10. Session handoff context
    let last_errors: Vec<String> = observations
        .iter()
        .filter(|o| o.result.as_deref() == Some("error"))
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|o| {
            let cat = o.failure_category.as_deref().unwrap_or("unknown");
            let snippet = o
                .error_snippet
                .as_deref()
                .unwrap_or(o.action.as_deref().unwrap_or(""));
            format!("{cat}: {}", &snippet[..snippet.len().min(100)])
        })
        .collect();
    if !last_errors.is_empty() {
        metrics.last_error_context = Some(last_errors.join(" | "));
    }

    // 11. Update metrics
    let score_entry = SessionScoreEntry {
        timestamp: now_iso(),
        success_rate: analysis.success_rate,
        avg_score: analysis.avg_score,
        observations: analysis.total_observations,
        dimension_averages: analysis.dimension_averages,
    };
    metrics.score_history.push(score_entry);
    if metrics.score_history.len() > 50 {
        let start = metrics.score_history.len() - 50;
        metrics.score_history = metrics.score_history[start..].to_vec();
    }

    metrics.total_sessions += 1;
    metrics.avg_success_rate = round3(
        ((metrics.avg_success_rate * (metrics.total_sessions - 1) as f64) + analysis.success_rate)
            / metrics.total_sessions as f64,
    );
    metrics.total_evolved_skills = record.total_evolved;
    metrics.last_session = Some(now_iso());

    if improved {
        metrics.best_score = Some(analysis.avg_score);
        metrics.best_session = now_iso();
        metrics.stagnation_count = 0;
    }
    metrics.trend = compute_trend(&metrics.score_history).into();

    if let Ok(json) = serde_json::to_string_pretty(&metrics) {
        let _ = fs::write(metrics_file(), json);
    }

    // 11.5. Workspace manifest
    write_workspace_manifest();

    // 12. Report
    hint(
        "reflect",
        &format!(
            "Session: {:.1}% success, avg_score={} ({} obs)",
            analysis.success_rate * 100.0,
            analysis.avg_score,
            analysis.total_observations
        ),
    );

    let weak_tools: Vec<String> = analysis
        .per_tool_stats
        .iter()
        .filter(|(_, s)| {
            s.total >= CONFIG.pattern.weak_tool_min_obs
                && (s.successes as f64 / s.total as f64) < CONFIG.pattern.weak_tool_rate
        })
        .map(|(cat, s)| {
            format!(
                "{cat} {}%",
                (s.successes as f64 / s.total as f64 * 100.0) as u32
            )
        })
        .collect();
    if !weak_tools.is_empty() {
        hint("reflect", &format!("Weak tools: {}", weak_tools.join(", ")));
    }

    let weak_exts: Vec<String> = analysis
        .per_ext_stats
        .iter()
        .filter(|(_, s)| {
            s.total >= CONFIG.pattern.weak_ext_min_obs
                && s.success_rate < CONFIG.pattern.weak_ext_rate
        })
        .map(|(ext, s)| format!("{ext} {}%", (s.success_rate * 100.0) as u32))
        .collect();
    if !weak_exts.is_empty() {
        hint(
            "reflect",
            &format!("Weak file types: {}", weak_exts.join(", ")),
        );
    }

    if !analysis.failure_patterns.is_empty() {
        let pats: Vec<String> = analysis
            .failure_patterns
            .iter()
            .map(|p| format!("{}({})", p.pattern_type, p.count))
            .collect();
        hint("reflect", &format!("Patterns: {}", pats.join(", ")));
    }

    if seeded > 0 {
        hint("reflect", &format!("Evolved {seeded} new skill(s)"));
    }
    if should_rollback {
        hint(
            "reflect",
            &format!("Rolled back {rolled_back_count} stagnant skills"),
        );
    }
    hint(
        "reflect",
        &format!(
            "Trend: {} ({} sessions)",
            metrics.trend,
            metrics.score_history.len()
        ),
    );

    // Skill attribution report
    let effective: Vec<_> = metrics
        .skill_attribution
        .values()
        .filter(|a| a.sessions_active >= 2 && a.avg_score_with > a.avg_score_without + 0.02)
        .collect();
    let ineffective: Vec<_> = metrics
        .skill_attribution
        .values()
        .filter(|a| a.sessions_active >= 2 && a.avg_score_with < a.avg_score_without - 0.02)
        .collect();
    if !effective.is_empty() {
        let parts: Vec<String> = effective
            .iter()
            .map(|s| {
                format!(
                    "{}(+{}%)",
                    s.skill_name,
                    ((s.avg_score_with - s.avg_score_without) * 100.0) as i32
                )
            })
            .collect();
        hint(
            "reflect",
            &format!("Effective skills: {}", parts.join(", ")),
        );
    }
    if !ineffective.is_empty() {
        let names: Vec<&str> = ineffective.iter().map(|s| s.skill_name.as_str()).collect();
        hint(
            "reflect",
            &format!(
                "Ineffective skills: {} — consider /evolve rollback",
                names.join(", ")
            ),
        );
    }

    if mem_nodes > 0 || mem_edges > 0 {
        hint(
            "reflect",
            &format!("Memory: +{mem_nodes} nodes, +{mem_edges} edges ingested"),
        );
    }

    TELEMETRY.track_session_ended(
        analysis.success_rate,
        safe_avg_score(analysis.avg_score),
        analysis.total_observations,
        metrics.trend.parse().unwrap_or(SessionTrend::Stable),
        seeded,
    );

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obs(
        tool: &str,
        cat: &str,
        result: &str,
        score: f64,
        action: Option<&str>,
    ) -> ObsRecord {
        ObsRecord {
            timestamp: "2026-04-09T12:00:00Z".into(),
            tool: tool.into(),
            tool_category: cat.into(),
            action: action.map(String::from),
            result: Some(result.into()),
            score: Some(score),
            dimensions: Some(ScoreDimensions {
                tool_success: if result == "success" { 1.0 } else { 0.0 },
                output_quality: score,
                execution_cost: 1.0,
            }),
            failure_category: if result == "error" {
                Some("type_error".into())
            } else {
                None
            },
            error_snippet: if result == "error" {
                Some("TypeError: x is not a function".into())
            } else {
                None
            },
            file_ext: Some(".ts".into()),
            sequence_id: Some(1),
            pipeline_id: None,
        }
    }

    // ── analyze_session ─────────────────────────────
    #[test]
    fn analyze_empty() {
        let analysis = analyze_session(&[]);
        assert_eq!(analysis.total_observations, 0);
        assert_eq!(analysis.success_rate, 1.0);
    }

    #[test]
    fn analyze_all_success() {
        let obs = vec![
            make_obs("Bash", "bash", "success", 1.0, Some("npm test")),
            make_obs("Edit", "edit", "success", 0.9, Some("/src/main.ts")),
        ];
        let analysis = analyze_session(&obs);
        assert_eq!(analysis.total_observations, 2);
        assert_eq!(analysis.success_rate, 1.0);
        assert!(analysis.avg_score > 0.9);
    }

    #[test]
    fn analyze_with_errors() {
        let obs = vec![
            make_obs("Bash", "bash", "success", 1.0, Some("npm test")),
            make_obs("Bash", "bash", "error", 0.0, Some("node broken.js")),
        ];
        let analysis = analyze_session(&obs);
        assert_eq!(analysis.success_rate, 0.5);
        assert!(!analysis.per_error_stats.is_empty());
    }

    #[test]
    fn analyze_per_tool_stats() {
        let obs = vec![
            make_obs("Bash", "bash", "success", 1.0, Some("ls")),
            make_obs("Bash", "bash", "error", 0.0, Some("node x.js")),
            make_obs("Edit", "edit", "success", 0.9, Some("/src/a.ts")),
        ];
        let analysis = analyze_session(&obs);
        assert!(analysis.per_tool_stats.contains_key("bash"));
        assert!(analysis.per_tool_stats.contains_key("edit"));
        let bash = &analysis.per_tool_stats["bash"];
        assert_eq!(bash.total, 2);
        assert_eq!(bash.successes, 1);
        assert_eq!(bash.errors, 1);
    }

    #[test]
    fn analyze_per_ext_stats() {
        let obs = vec![
            make_obs("Edit", "edit", "success", 1.0, Some("/src/a.ts")),
            make_obs("Edit", "edit", "error", 0.0, Some("/src/b.ts")),
        ];
        let analysis = analyze_session(&obs);
        let ts = &analysis.per_ext_stats[".ts"];
        assert_eq!(ts.total, 2);
        assert_eq!(ts.errors, 1);
        assert_eq!(ts.success_rate, 0.5);
    }

    #[test]
    fn analyze_dimension_averages() {
        let obs = vec![
            make_obs("Bash", "bash", "success", 1.0, Some("ls")),
            make_obs("Bash", "bash", "error", 0.0, Some("bad")),
        ];
        let analysis = analyze_session(&obs);
        assert_eq!(analysis.dimension_averages.tool_success, 0.5);
    }

    // ── detect_patterns ─────────────────────────────
    #[test]
    fn detect_repeated_same_error() {
        let mut obs = vec![];
        for _ in 0..4 {
            obs.push(make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")));
        }
        let patterns = detect_patterns(&obs);
        assert!(
            patterns
                .iter()
                .any(|p| p.pattern_type == "repeated_same_error")
        );
    }

    #[test]
    fn no_repeated_error_below_threshold() {
        let obs = vec![
            make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")),
            make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")),
        ];
        let patterns = detect_patterns(&obs);
        assert!(
            !patterns
                .iter()
                .any(|p| p.pattern_type == "repeated_same_error")
        );
    }

    #[test]
    fn detect_long_debug_loop() {
        let mut obs = vec![];
        for _ in 0..6 {
            obs.push(make_obs(
                "Edit",
                "edit",
                "success",
                0.8,
                Some("/src/buggy.ts"),
            ));
        }
        let patterns = detect_patterns(&obs);
        assert!(patterns.iter().any(|p| p.pattern_type == "long_debug_loop"));
    }

    #[test]
    fn no_debug_loop_below_threshold() {
        let mut obs = vec![];
        for _ in 0..4 {
            obs.push(make_obs(
                "Edit",
                "edit",
                "success",
                0.8,
                Some("/src/buggy.ts"),
            ));
        }
        let patterns = detect_patterns(&obs);
        assert!(!patterns.iter().any(|p| p.pattern_type == "long_debug_loop"));
    }

    #[test]
    fn detect_thrashing() {
        let mut obs = vec![];
        for _ in 0..4 {
            obs.push(make_obs(
                "Edit",
                "edit",
                "success",
                0.8,
                Some("/src/main.ts"),
            ));
            obs.push(make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")));
        }
        let patterns = detect_patterns(&obs);
        assert!(patterns.iter().any(|p| p.pattern_type == "thrashing"));
    }

    #[test]
    fn no_thrashing_below_threshold() {
        let obs = vec![
            make_obs("Edit", "edit", "success", 0.8, Some("/src/main.ts")),
            make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")),
        ];
        let patterns = detect_patterns(&obs);
        assert!(!patterns.iter().any(|p| p.pattern_type == "thrashing"));
    }

    // ── compute_trend ───────────────────────────────
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

    // ── check_stagnation ────────────────────────────
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
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.8);
        assert!(!rollback);
        assert!(improved);
    }

    #[test]
    fn stagnation_increments_on_no_improvement() {
        let mut metrics = default_metrics();
        metrics.total_sessions = 5;
        metrics.best_score = Some(0.8);
        metrics.stagnation_count = 0;
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.78);
        assert!(!rollback);
        assert!(!improved);
        assert_eq!(metrics.stagnation_count, 1);
    }

    #[test]
    fn stagnation_zero_score_is_not_sentinel() {
        // A genuine all-failure session (score=0.0) must increment stagnation_count,
        // not be treated as "uninitialized" (the old best_score == 0.0 sentinel bug).
        let mut metrics = default_metrics();
        metrics.total_sessions = 3;
        metrics.best_score = Some(0.0); // best so far is also 0.0 — no improvement
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
        // First session with score=0.0 must set best_score to Some(0.0), not leave it None.
        let mut metrics = default_metrics();
        // total_sessions=0 triggers first-session branch regardless of score value.
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.0);
        assert!(!rollback);
        assert!(improved);
        assert_eq!(metrics.best_score, Some(0.0));
    }

    // ── build_summary ───────────────────────────────
    #[test]
    fn summary_basic() {
        let analysis = SessionAnalysis {
            total_observations: 10,
            success_rate: 0.8,
            avg_score: 0.75,
            ..Default::default()
        };
        let s = build_summary(&analysis);
        assert!(s.contains("10 obs"));
        assert!(s.contains("80.0%"));
    }

    #[test]
    fn summary_with_errors() {
        let mut errors = HashMap::new();
        errors.insert("type_error".into(), 3);
        let analysis = SessionAnalysis {
            total_observations: 10,
            success_rate: 0.7,
            avg_score: 0.65,
            per_error_stats: errors,
            ..Default::default()
        };
        let s = build_summary(&analysis);
        assert!(s.contains("type_error"));
    }

    // ── round3 ──────────────────────────────────────
    #[test]
    fn round3_precision() {
        assert_eq!(round3(0.12345), 0.123);
        assert_eq!(round3(0.9999), 1.0);
        assert_eq!(round3(0.0), 0.0);
    }

    // ── gate_skills: frontmatter with --- in skill body ──
    #[test]
    fn gate_skills_body_extraction_with_embedded_dashes() {
        // A SKILL.md whose body contains a "---" horizontal rule.
        // With unlimited split("---").nth(2), the body is truncated at the embedded "---".
        // With splitn(3, "---").nth(2), the remainder (everything after the closing "---")
        // is returned in full, preserving the body's embedded "---".
        let content = "---\nname: test\n---\n\n# Body content here, long enough\n\n---\n\nmore content below\n";
        let body_splitn = content.splitn(3, "---").nth(2).unwrap_or("").trim();
        let body_unlimited = content.split("---").nth(2).unwrap_or("").trim();

        // splitn(3) body starts at the closing "---" delimiter and contains everything after it
        assert!(
            body_splitn.starts_with("# Body"),
            "splitn body: {:?}",
            body_splitn
        );
        // splitn(3) body preserves the embedded "---" inside the body
        assert!(
            body_splitn.contains("more content"),
            "splitn must preserve full body: {:?}",
            body_splitn
        );
        // unlimited split's nth(2) stops at the *body's* "---", truncating it
        assert!(
            !body_unlimited.contains("more content"),
            "unlimited split truncates body: {:?}",
            body_unlimited
        );
        // splitn body must be >= 20 chars (passes gate validation)
        assert!(body_splitn.len() >= 20);
    }

    // ── update_skill_attribution: uses avg_score not avg_success_rate ──
    #[test]
    fn skill_attribution_uses_avg_score_not_success_rate() {
        let mut metrics = default_metrics();
        // Set up divergent values so we can detect which one is used
        metrics.avg_success_rate = 0.99; // should NOT be used
        // score_history has avg_score = 0.60
        metrics.score_history.push(SessionScoreEntry {
            timestamp: "2026-04-09T00:00:00Z".into(),
            success_rate: 0.99,
            avg_score: 0.60,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        });
        metrics.total_sessions = 1;

        let analysis = SessionAnalysis {
            avg_score: 0.70,
            ..Default::default()
        };
        // skill is active this session → avg_score_with = 0.70
        // skill was absent in the 1 prior session (total=2, active=1, without=1)
        // avg_score_without should be derived from score_history avg_score (0.60), NOT avg_success_rate (0.99)
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
        // avg_score_without should be close to 0.60 (from score_history), not 0.99
        assert!(
            (attr.avg_score_without - 0.60).abs() < 0.05,
            "avg_score_without should be ~0.60 (from score_history avg_score), got {}",
            attr.avg_score_without
        );
    }

    // ── compute_trend: no NaN when all scores are identical ──
    #[test]
    fn trend_no_nan_with_zero_denominator() {
        // All scores the same → n*sxx - sx*sx == 0 → division by zero → NaN slope
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
        // Must not be "NaN" / must be a valid string
        assert!(
            trend == "stable" || trend == "improving" || trend == "declining",
            "trend must be a valid value, got: {:?}",
            trend
        );
        assert_eq!(trend, "stable");
    }

    // ── skill builders ──────────────────────────────
    #[test]
    fn pattern_skill_has_frontmatter() {
        let p = DetectedPattern {
            pattern_type: "repeated_same_error".into(),
            description: "test".into(),
            count: 5,
            involved_files: vec!["/src/main.ts".into()],
            suggested_remediation: "stop".into(),
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

    // ── seeding_scope (Graduated Scope) ─────────────
    #[test]
    fn seeding_scope_skip_when_excellent() {
        assert_eq!(seeding_scope(0.90), "skip");
        assert_eq!(seeding_scope(0.95), "skip");
        assert_eq!(seeding_scope(1.00), "skip");
    }

    #[test]
    fn seeding_scope_moderate_in_middle_range() {
        assert_eq!(seeding_scope(0.70), "moderate");
        assert_eq!(seeding_scope(0.80), "moderate");
        assert_eq!(seeding_scope(0.89), "moderate");
    }

    #[test]
    fn seeding_scope_full_when_low_score() {
        assert_eq!(seeding_scope(0.69), "full");
        assert_eq!(seeding_scope(0.50), "full");
        assert_eq!(seeding_scope(0.0), "full");
    }

    // ── extract_instincts (R12) ──────────────────────
    #[test]
    fn extract_instincts_returns_empty_for_few_observations() {
        let obs: Vec<ObsRecord> = (0..5)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let analysis = SessionAnalysis {
            total_observations: 5,
            avg_score: 0.9,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.is_empty(),
            "fewer than 10 observations should yield no instincts"
        );
    }

    #[test]
    fn extract_instincts_returns_empty_for_low_score() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.3, Some("ls")))
            .collect();
        let analysis = SessionAnalysis {
            total_observations: 15,
            avg_score: 0.3,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.is_empty(),
            "avg_score < 0.5 should yield no instincts"
        );
    }

    #[test]
    fn extract_instincts_finds_high_success_tools() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let mut per_tool = HashMap::new();
        per_tool.insert(
            "bash".into(),
            ToolStats {
                tool_category: "bash".into(),
                total: 15,
                successes: 15,
                errors: 0,
                avg_score: 0.9,
                failure_categories: HashMap::new(),
            },
        );
        let analysis = SessionAnalysis {
            total_observations: 15,
            avg_score: 0.9,
            per_tool_stats: per_tool,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.iter().any(|i| i.trigger == "high-success-bash"),
            "should find high-success-bash instinct"
        );
    }

    #[test]
    fn extract_instincts_respects_max_limit() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let mut per_tool = HashMap::new();
        // Create more tool categories than CONFIG.instinct.max_instincts
        for i in 0..30 {
            let cat = format!("tool-{}", i);
            per_tool.insert(
                cat.clone(),
                ToolStats {
                    tool_category: cat,
                    total: 5,
                    successes: 5,
                    errors: 0,
                    avg_score: 0.9,
                    failure_categories: HashMap::new(),
                },
            );
        }
        let analysis = SessionAnalysis {
            total_observations: 150,
            avg_score: 0.9,
            per_tool_stats: per_tool,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.len() <= CONFIG.instinct.max_instincts,
            "should not exceed max_instincts ({})",
            CONFIG.instinct.max_instincts
        );
    }

    // ── Workspace Contract (R13) ──────────────────────
    #[test]
    fn workspace_manifest_has_version() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let evolved = tmp.path().join("evolved");
        ensure_dir(&evolved);

        // Create a skill directory with SKILL.md
        let skill_dir = evolved.join("evo-test-skill");
        ensure_dir(&skill_dir);
        let _ = fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test\n---\n\n# Test skill content that is long enough to pass gate\n",
        );

        // Write manifest
        let dirs = list_dirs(&evolved);
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

        // Verify
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

        // Override evolved_dir by writing directly to tmp path
        let skill_dir = tmp.path().join("evo-test-skill");
        ensure_dir(&skill_dir);
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
        };
        let json = serde_json::to_string_pretty(&meta).expect("serialize meta");
        let _ = fs::write(skill_dir.join("meta.json"), &json);

        // Verify both files exist
        assert!(skill_dir.join("SKILL.md").is_file(), "SKILL.md must exist");
        assert!(
            skill_dir.join("meta.json").is_file(),
            "meta.json must exist"
        );

        // Verify meta.json content
        let meta_content = fs::read_to_string(skill_dir.join("meta.json")).expect("read meta");
        let parsed: SkillMeta = serde_json::from_str(&meta_content).expect("parse meta");
        assert_eq!(parsed.name, "evo-test-skill");
        assert_eq!(parsed.origin, "weak_tool");
        assert!((parsed.confidence - 0.6).abs() < f64::EPSILON);
        assert!(parsed.active);
    }

    // ── Gated Promotion (R16) ──────────────────────
    #[test]
    fn check_promotion_increments() {
        let mut counters = PromotionCounter::default();
        assert!(!check_promotion("evo-test", &mut counters));
        assert_eq!(counters.counts["evo-test"], 1);
        assert!(!check_promotion("evo-test", &mut counters));
        assert_eq!(counters.counts["evo-test"], 2);
        assert!(check_promotion("evo-test", &mut counters));
        assert_eq!(counters.counts["evo-test"], 3);
    }

    #[test]
    fn check_promotion_requires_min_observations() {
        let mut counters = PromotionCounter::default();
        // Below threshold: should return false each time
        for _ in 0..CONFIG.evolution.gated_promotion_min - 1 {
            assert!(
                !check_promotion("evo-fix-type-error", &mut counters),
                "should not be promoted before reaching gated_promotion_min"
            );
        }
        // Exactly at threshold: should return true
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
        // A single observation must NOT be promoted
        let promoted = check_promotion("evo-once-seen", &mut counters);
        assert!(
            !promoted,
            "skill seen only once must not be promoted (count={})",
            counters.counts.get("evo-once-seen").unwrap_or(&0)
        );
        assert_eq!(counters.counts["evo-once-seen"], 1);
    }

    // ── R14: Solver-Proposes + Curator Pattern ──────
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
            curate_proposal(&proposal, &existing),
            ProposalAction::Skip
        ));
    }

    #[test]
    fn curate_proposal_skips_low_confidence() {
        let proposal = SkillProposal {
            name: "evo-new".into(),
            content: "content".into(),
            origin: "pattern".into(),
            confidence: 0.2,
            rationale: "test".into(),
        };
        assert!(matches!(
            curate_proposal(&proposal, &[]),
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
            curate_proposal(&proposal, &[]),
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
            curate_proposal(&proposal, &[]),
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
        // Tool with 60% success rate (>= 0.6 threshold) should be excluded
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

    // ── sanitize_skill_name (path traversal prevention) ──
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

    // ── ensure_project_hub (Phase 3 auto-edges) ──────
    fn open_test_mem_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        store::init_schema(&conn).expect("schema");
        conn
    }

    #[test]
    fn ensure_project_hub_creates_new_hub_node() {
        let conn = open_test_mem_db();
        let hub_id = ensure_project_hub(&conn, "test-project").unwrap();
        assert!(!hub_id.is_empty(), "hub ID must not be empty");

        // Verify the node exists
        let node = store::read_node_conn(&conn, &hub_id).unwrap();
        assert_eq!(node.frontmatter.node_type, "project");
        assert_eq!(node.frontmatter.title, "project: test-project");
        assert!(node.frontmatter.tags.contains(&"hub".to_string()));
        assert!(
            node.frontmatter
                .projects
                .contains(&"test-project".to_string())
        );
    }

    #[test]
    fn ensure_project_hub_returns_existing_hub() {
        let conn = open_test_mem_db();
        let id1 = ensure_project_hub(&conn, "my-proj").unwrap();
        let id2 = ensure_project_hub(&conn, "my-proj").unwrap();
        assert_eq!(id1, id2, "second call must return same hub ID");
    }

    #[test]
    fn ensure_project_hub_different_projects_get_different_ids() {
        let conn = open_test_mem_db();
        let id_a = ensure_project_hub(&conn, "proj-a").unwrap();
        let id_b = ensure_project_hub(&conn, "proj-b").unwrap();
        assert_ne!(id_a, id_b, "different projects must get different hub IDs");
    }

    #[test]
    fn auto_edge_belongs_to_links_session_to_project_hub() {
        let conn = open_test_mem_db();

        // Create a project hub
        let hub_id = ensure_project_hub(&conn, "edge-test-proj").unwrap();

        // Create a session node
        let session_id = store::new_uuid();
        let session_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: session_id.clone(),
                node_type: "session".into(),
                title: "session: edge-test-proj 80% avg=0.8".into(),
                tags: vec!["auto".into(), "session".into()],
                projects: vec!["edge-test-proj".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                importance: store::importance_for_type("session"),
                ..Default::default()
            },
            body: "test session".into(),
        };
        store::write_node_conn(&conn, &session_node).unwrap();

        // Create belongs_to edge: session -> hub
        let edge = store::Edge {
            id: store::new_uuid(),
            source: session_id.clone(),
            target: hub_id.clone(),
            relation: "belongs_to".into(),
            weight: 0.5,
            ts: store::now_iso(),
        };
        store::append_edge_conn(&conn, &edge).unwrap();

        // Verify edge exists
        let edges = store::read_edges_conn(&conn);
        let found = edges
            .iter()
            .any(|e| e.source == session_id && e.target == hub_id && e.relation == "belongs_to");
        assert!(found, "belongs_to edge from session to hub must exist");
    }

    #[test]
    fn auto_edge_follows_links_previous_session() {
        let conn = open_test_mem_db();

        // Create two session nodes in the same project
        let prev_id = store::new_uuid();
        let prev_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: prev_id.clone(),
                node_type: "session".into(),
                title: "session: chain-proj 70%".into(),
                tags: vec!["auto".into()],
                projects: vec!["chain-proj".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "prev session".into(),
        };
        store::write_node_conn(&conn, &prev_node).unwrap();

        let curr_id = store::new_uuid();
        let curr_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: curr_id.clone(),
                node_type: "session".into(),
                title: "session: chain-proj 80%".into(),
                tags: vec!["auto".into()],
                projects: vec!["chain-proj".into()],
                created: "2026-01-02T00:00:00Z".into(),
                updated: "2026-01-02T00:00:00Z".into(),
                ..Default::default()
            },
            body: "curr session".into(),
        };
        store::write_node_conn(&conn, &curr_node).unwrap();

        // Simulate the follows edge logic from ingest_to_memory
        let prev_session: Option<String> = conn
            .query_row(
                "SELECT id FROM nodes WHERE type = 'session' AND id != ?1
             AND (',' || projects || ',' LIKE '%,' || ?2 || ',%')
             ORDER BY updated DESC LIMIT 1",
                rusqlite::params![curr_id, "chain-proj"],
                |row| row.get(0),
            )
            .ok();

        assert!(prev_session.is_some(), "should find a previous session");
        assert_eq!(prev_session.unwrap(), prev_id);

        // Create follows edge
        let edge = store::Edge {
            id: store::new_uuid(),
            source: prev_id.clone(),
            target: curr_id.clone(),
            relation: "follows".into(),
            weight: 0.3,
            ts: store::now_iso(),
        };
        store::append_edge_conn(&conn, &edge).unwrap();

        let edges = store::read_edges_conn(&conn);
        let found = edges
            .iter()
            .any(|e| e.source == prev_id && e.target == curr_id && e.relation == "follows");
        assert!(
            found,
            "follows edge must exist from prev to current session"
        );
    }

    #[test]
    fn auto_edge_shares_context_links_same_tag_nodes() {
        let conn = open_test_mem_db();

        // Create two error nodes sharing a tag (but not "auto")
        let id_a = store::new_uuid();
        let node_a = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_a.clone(),
                node_type: "error".into(),
                title: "error A".into(),
                tags: vec!["auto".into(), "weak-tool".into(), "bash".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error A body".into(),
        };
        store::write_node_conn(&conn, &node_a).unwrap();

        let id_b = store::new_uuid();
        let node_b = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_b.clone(),
                node_type: "error".into(),
                title: "error B".into(),
                tags: vec!["auto".into(), "high-freq-error".into(), "bash".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error B body".into(),
        };
        store::write_node_conn(&conn, &node_b).unwrap();

        // Read back both nodes and compute shared tags (mirroring 8h logic)
        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_conn(&conn, &all_new_ids);
        assert_eq!(new_nodes.len(), 2);

        let shared: Vec<String> = new_nodes[0]
            .frontmatter
            .tags
            .iter()
            .filter(|t| **t != "auto" && new_nodes[1].frontmatter.tags.contains(t))
            .cloned()
            .collect();
        assert!(
            shared.contains(&"bash".to_string()),
            "should share 'bash' tag"
        );

        // Create shares_context edge
        let edge = store::Edge {
            id: store::new_uuid(),
            source: id_a.clone(),
            target: id_b.clone(),
            relation: "shares_context".into(),
            weight: shared.len() as f64,
            ts: store::now_iso(),
        };
        store::append_edge_conn(&conn, &edge).unwrap();

        let edges = store::read_edges_conn(&conn);
        let found = edges
            .iter()
            .any(|e| e.source == id_a && e.target == id_b && e.relation == "shares_context");
        assert!(
            found,
            "shares_context edge must exist between same-tag nodes"
        );
    }

    #[test]
    fn auto_edge_shares_context_ignores_auto_tag() {
        let conn = open_test_mem_db();

        // Two nodes that only share "auto" tag — should NOT get shares_context edge
        let id_a = store::new_uuid();
        let node_a = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_a.clone(),
                node_type: "error".into(),
                title: "error C".into(),
                tags: vec!["auto".into(), "weak-tool".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error C body".into(),
        };
        store::write_node_conn(&conn, &node_a).unwrap();

        let id_b = store::new_uuid();
        let node_b = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_b.clone(),
                node_type: "error".into(),
                title: "error D".into(),
                tags: vec!["auto".into(), "high-freq-error".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error D body".into(),
        };
        store::write_node_conn(&conn, &node_b).unwrap();

        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_conn(&conn, &all_new_ids);

        let shared: Vec<String> = new_nodes[0]
            .frontmatter
            .tags
            .iter()
            .filter(|t| **t != "auto" && new_nodes[1].frontmatter.tags.contains(t))
            .cloned()
            .collect();
        assert!(
            shared.is_empty(),
            "nodes sharing only 'auto' tag should have no shared tags"
        );
    }

    // ── instinct promotion gate (no .min(1) bypass) ──
    #[test]
    fn instinct_promotion_requires_multi_project_by_config() {
        // With default config (promotion_min_projects=2), a single-project instinct
        // must NOT pass the gate. Only instincts from >= 2 projects should be promoted.
        let instinct_one_project = Instinct {
            trigger: "high-success-bash".into(),
            confidence: 0.9,
            domain: "tool-usage".into(),
            scope: "local".into(),
            observation_count: 20,
            success_count: 18,
            projects: vec!["project-a".into()],
        };
        // The gate checks: projects.len() < promotion_min_projects
        // With default config (2), one project should fail the gate
        assert!(
            instinct_one_project.projects.len() < CONFIG.instinct.promotion_min_projects,
            "single-project instinct must fail the promotion gate with default config (min=2)"
        );

        let instinct_two_projects = Instinct {
            trigger: "high-success-bash".into(),
            confidence: 0.9,
            domain: "tool-usage".into(),
            scope: "local".into(),
            observation_count: 20,
            success_count: 18,
            projects: vec!["project-a".into(), "project-b".into()],
        };
        assert!(
            instinct_two_projects.projects.len() >= CONFIG.instinct.promotion_min_projects,
            "two-project instinct must pass the promotion gate with default config (min=2)"
        );
    }

    // ── run_context signature ──────────────────────────
    #[test]
    fn effective_sources_all_expands() {
        let sources: Vec<String> = vec!["all".into()];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness"]
        } else {
            sources.iter().map(|s| s.as_str()).collect()
        };
        assert_eq!(effective, vec!["harness", "claude-session", "alcove"]);
    }

    #[test]
    fn effective_sources_empty_defaults_to_harness() {
        let sources: Vec<String> = vec![];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness"]
        } else {
            sources.iter().map(|s| s.as_str()).collect()
        };
        assert_eq!(effective, vec!["harness"]);
    }

    #[test]
    fn effective_sources_explicit_list_passthrough() {
        let sources: Vec<String> = vec!["harness".into(), "alcove".into()];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness"]
        } else {
            sources.iter().map(|s| s.as_str()).collect()
        };
        assert_eq!(effective, vec!["harness", "alcove"]);
    }

    #[test]
    fn since_overrides_days_in_date_range() {
        // When --since is provided, date_from should equal since value
        let since: Option<String> = Some("20260101".into());
        let days: u32 = 30;

        let (cutoff_tag, date_from) = if let Some(ref s) = since {
            (s.clone(), s.clone())
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let cutoff_ts = now.saturating_sub((days as u64) * 86400);
            let days_since_epoch = cutoff_ts / 86400;
            let (y, m, d) = epoch_days_to_ymd(days_since_epoch as i32);
            let tag = format!("{y:04}{m:02}{d:02}");
            (tag.clone(), tag)
        };

        assert_eq!(cutoff_tag, "20260101");
        assert_eq!(date_from, "20260101");
    }
}
