use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::common::*;

// ── Phase 1: Session Analysis ───────────────────────

fn analyze_session(observations: &[ObsRecord]) -> SessionAnalysis {
    let scored: Vec<_> = observations.iter().filter(|o| o.score.is_some()).collect();
    let total = scored.len() as u64;
    let errors: Vec<_> = scored.iter().filter(|o| o.result.as_deref() == Some("error")).collect();
    let success_rate = if total > 0 { round3((total - errors.len() as u64) as f64 / total as f64) } else { 1.0 };
    let avg_score = if total > 0 { round3(scored.iter().map(|o| o.score.unwrap_or(0.0)).sum::<f64>() / total as f64) } else { 0.0 };

    // Score distribution
    let mut buckets: HashMap<String, u64> = [
        ("0.0-0.2", 0), ("0.2-0.4", 0), ("0.4-0.6", 0), ("0.6-0.8", 0), ("0.8-1.0", 0),
    ].into_iter().map(|(k, v)| (k.into(), v)).collect();
    for o in &scored {
        let s = o.score.unwrap_or(0.0);
        let key = if s < 0.2 { "0.0-0.2" } else if s < 0.4 { "0.2-0.4" } else if s < 0.6 { "0.4-0.6" } else if s < 0.8 { "0.6-0.8" } else { "0.8-1.0" };
        *buckets.get_mut(key).unwrap() += 1;
    }

    // Per-tool stats
    let mut tool_map: HashMap<String, ToolStats> = HashMap::new();
    for o in &scored {
        let cat = &o.tool_category;
        let ts = tool_map.entry(cat.clone()).or_insert_with(|| ToolStats { tool_category: cat.clone(), ..Default::default() });
        ts.total += 1;
        if o.result.as_deref() == Some("error") {
            ts.errors += 1;
            let fc = o.failure_category.as_deref().unwrap_or("other");
            *ts.failure_categories.entry(fc.into()).or_default() += 1;
        } else {
            ts.successes += 1;
        }
        ts.avg_score = ((ts.avg_score * (ts.total - 1) as f64) + o.score.unwrap_or(0.0)) / ts.total as f64;
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
        if o.result.as_deref() == Some("error") { es.errors += 1; }
    }
    for es in ext_map.values_mut() {
        es.success_rate = if es.total > 0 { round3((es.total - es.errors) as f64 / es.total as f64) } else { 1.0 };
    }

    // Dimension averages
    let dims_scored: Vec<_> = scored.iter().filter_map(|o| o.dimensions.as_ref()).collect();
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
            let curr_hash = if !curr_snippet.is_empty() { hash_string(&normalize_error(curr_snippet)) } else { String::new() };
            let prev_hash_val = if !prev_snippet.is_empty() { hash_string(&normalize_error(prev_snippet)) } else { String::new() };

            let same_error = curr.result.as_deref() == Some("error")
                && prev.result.as_deref() == Some("error")
                && curr.failure_category == prev.failure_category
                && curr.failure_category.is_some()
                && curr.action.is_some() && prev.action.is_some()
                && extract_file(curr.action.as_deref().unwrap_or("")) == extract_file(prev.action.as_deref().unwrap_or(""))
                && (curr_hash == prev_hash_val || curr_hash.is_empty() || prev_hash_val.is_empty());

            if same_error {
                streak += 1;
                streak_file = extract_file(curr.action.as_deref().unwrap_or("")).unwrap_or("").to_string();
                streak_category = curr.failure_category.clone().unwrap_or_default();
                prev_hash = curr_hash;
            } else {
                if streak >= REPEATED_ERROR_MIN {
                    let hash_note = if !prev_hash.is_empty() { format!(" [hash:{prev_hash}]") } else { String::new() };
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
        if streak >= REPEATED_ERROR_MIN {
            let hash_note = if !prev_hash.is_empty() { format!(" [hash:{prev_hash}]") } else { String::new() };
            patterns.push(DetectedPattern {
                pattern_type: "repeated_same_error".into(),
                description: format!("{streak_category} repeated {streak}x on {streak_file}{hash_note}"),
                count: streak,
                involved_files: if streak_file.is_empty() { vec![] } else { vec![streak_file] },
                suggested_remediation: format!("Stop retrying the same approach for {streak_category}. Re-read the full error."),
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
                let file = extract_file(o.action.as_deref().unwrap_or("")).unwrap_or(o.action.as_deref().unwrap_or(""));
                let basename = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
                for next in scored.iter().take((i + FTB_LOOKAHEAD + 1).min(scored.len())).skip(i + 1) {
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
        let ftb_entries: Vec<_> = ftb_files.iter().filter(|(_, c)| **c >= FTB_MIN_CYCLES).collect();
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
            let file = extract_file(o.action.as_deref().unwrap_or("")).unwrap_or("").to_string();
            if !file.is_empty() && file == prev_file {
                run_length += 1;
            } else {
                if run_length >= DEBUG_LOOP_MIN && !prev_file.is_empty() {
                    let entry = file_runs.entry(prev_file.clone()).or_default();
                    *entry = (*entry).max(run_length);
                }
                prev_file = file;
                run_length = 1;
            }
        }
        if run_length >= DEBUG_LOOP_MIN && !prev_file.is_empty() {
            let entry = file_runs.entry(prev_file.clone()).or_default();
            *entry = (*entry).max(run_length);
        }

        for (file, count) in &file_runs {
            let basename = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
            patterns.push(DetectedPattern {
                pattern_type: "long_debug_loop".into(),
                description: format!("Stuck on {basename} for {count} consecutive operations"),
                count: *count,
                involved_files: vec![file.clone()],
                suggested_remediation: "Stuck in debug loop. Stop, re-read the surrounding code context (100+ lines).".into(),
            });
        }
    }

    // Pattern 4: thrashing
    {
        let mut file_stats: HashMap<String, (u64, u64)> = HashMap::new(); // (edits, errors)
        for o in &scored {
            let file = extract_file(o.action.as_deref().unwrap_or("")).unwrap_or("").to_string();
            if file.is_empty() { continue; }
            let entry = file_stats.entry(file).or_default();
            if o.tool_category == "edit" || o.tool_category == "write" { entry.0 += 1; }
            if o.result.as_deref() == Some("error") { entry.1 += 1; }
        }
        for (file, (edits, errors)) in &file_stats {
            if *edits >= THRASH_MIN_EDITS && *errors >= THRASH_MIN_ERRORS {
                let basename = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file);
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
    if metrics.total_sessions == 0 || metrics.best_score == 0.0 {
        return (false, true, 0);
    }

    let improvement = current_score - metrics.best_score;
    if improvement >= IMPROVEMENT_THRESHOLD {
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
    if metrics.stagnation_count >= STAGNATION_LIMIT {
        let degradation = metrics.best_score - current_score;
        if degradation > 0.05 || metrics.best_score < 0.90 {
            let backup = evolved_backup_dir();
            if backup.is_dir() {
                let evolved = evolved_dir();
                let before_count = list_dirs(&evolved).len() as u64;
                rm_dir(&evolved);
                copy_dir(&backup, &evolved);
                metrics.stagnation_count = 0;
                hint("reflect", &format!("Stagnation detected ({STAGNATION_LIMIT} sessions). Rolled back evolved skills."));
                return (true, false, before_count);
            }
        }
    }

    (false, false, 0)
}

// ── Phase 4: Skill Seeding ──────────────────────────

fn seed_smart_skills(analysis: &SessionAnalysis, existing: &[String]) -> u64 {
    let mut seeded = 0u64;
    let cap = MAX_EVOLVED_SKILLS.saturating_sub(existing.len());

    // 4a. From failure patterns
    for pattern in &analysis.failure_patterns {
        if seeded as usize >= cap { break; }
        let name = format!("evo-{}", pattern.pattern_type);
        if existing.contains(&name) { continue; }
        write_skill(&name, &build_pattern_skill(pattern));
        seeded += 1;
    }

    // 4b. From weak tools
    for stats in analysis.per_tool_stats.values() {
        if seeded as usize >= cap { break; }
        let rate = if stats.total > 0 { stats.successes as f64 / stats.total as f64 } else { 1.0 };
        if rate >= WEAK_TOOL_RATE || stats.total < WEAK_TOOL_MIN_OBS { continue; }
        let name = format!("evo-{}-discipline", stats.tool_category);
        if existing.contains(&name) { continue; }
        write_skill(&name, &build_tool_skill(stats));
        seeded += 1;
    }

    // 4c. From weak extensions
    for (ext, stats) in &analysis.per_ext_stats {
        if seeded as usize >= cap { break; }
        if stats.success_rate >= WEAK_EXT_RATE || stats.total < WEAK_EXT_MIN_OBS || ext == "unknown" { continue; }
        let clean = ext.trim_start_matches('.');
        let name = format!("evo-{clean}-care");
        if existing.contains(&name) { continue; }
        write_skill(&name, &build_ext_skill(ext, stats));
        seeded += 1;
    }

    // 4d. From high-frequency errors
    for (category, count) in &analysis.per_error_stats {
        if seeded as usize >= cap { break; }
        if *count < HIGH_FREQ_ERROR_MIN { continue; }
        let name = format!("evo-fix-{}", category.replace('_', "-"));
        if existing.contains(&name) { continue; }
        write_skill(&name, &build_failure_skill(category, *count));
        seeded += 1;
    }

    seeded
}

fn write_skill(name: &str, content: &str) {
    let dir = evolved_dir().join(name);
    ensure_dir(&dir);
    let _ = fs::write(dir.join("SKILL.md"), content);
}

fn build_pattern_skill(p: &DetectedPattern) -> String {
    format!(
        "---\nname: {}\ndescription: \"Auto-evolved from {}x {} pattern.\"\n---\n\n# {}\n\n**Detected**: {}\n**Files**: {}\n\n## Remediation\n{}\n\n## Red Flags\n- Retrying the same approach that already failed\n- Not reading the full error context\n- Patching symptoms instead of root cause\n",
        p.pattern_type, p.count, p.pattern_type, p.pattern_type, p.description,
        if p.involved_files.is_empty() { "various".into() } else { p.involved_files.join(", ") },
        p.suggested_remediation,
    )
}

fn build_tool_skill(stats: &ToolStats) -> String {
    let rate = if stats.total > 0 { (stats.successes as f64 / stats.total as f64 * 100.0) as u32 } else { 0 };
    let mut top: Vec<_> = stats.failure_categories.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    top.truncate(3);
    let failures = top.iter().map(|(c, n)| format!("- {c}: {n} occurrences")).collect::<Vec<_>>().join("\n");

    format!(
        "---\nname: {cat}-discipline\ndescription: \"Auto-evolved. {cat} tool success rate was {rate}%.\"\n---\n\n# {cat} discipline\n\nSuccess rate: {rate}% ({s}/{t})\n\n## Top Failure Types\n{failures}\n\n## Process\n1. Before using {cat}: verify preconditions\n2. Check the expected output format\n3. Validate paths and arguments exist\n4. On failure: read the FULL error, don't retry blindly\n\n## Red Flags\n- {cat} success rate still below 60%\n- Same error type repeating\n",
        cat = stats.tool_category, s = stats.successes, t = stats.total,
        failures = if failures.is_empty() { "- various errors".into() } else { failures },
    )
}

fn build_ext_skill(ext: &str, stats: &ExtStats) -> String {
    let rate = (stats.success_rate * 100.0) as u32;
    format!(
        "---\nname: {clean}-care\ndescription: \"Auto-evolved. {ext} files had {rate}% success rate.\"\n---\n\n# {ext} file care\n\nSuccess rate: {rate}% across {t} operations\n\n## Process\n1. Before editing {ext} files: run type-check / lint / build\n2. After editing: immediately verify\n3. If error: read the full diagnostic before re-editing\n\n## Red Flags\n- Editing {ext} files without verifying afterward\n- Ignoring compiler/linter warnings\n",
        clean = ext.trim_start_matches('.'), t = stats.total,
    )
}

fn build_failure_skill(category: &str, count: u64) -> String {
    let remediation = match category {
        "type_error" => "Check variable types. Read the full type signature. Use explicit type annotations.",
        "syntax_error" => "Check brackets, commas, semicolons. Ensure template literals are properly closed.",
        "test_fail" => "Read the assertion message carefully. Check expected vs actual. Run the failing test in isolation.",
        "lint_fail" => "Run the linter and fix all warnings. Configure the editor to show lint errors inline.",
        "build_fail" => "Run `tsc --noEmit` (or equivalent) to see all errors. Fix type errors before runtime testing.",
        "permission_denied" => "Check file permissions. Don't write to system directories.",
        "timeout" => "Command took too long. Consider a more targeted approach.",
        "not_found" => "File or command not found. Verify the path exists. Use glob to locate.",
        _ => "Read the full error message. Identify root cause. Fix the cause, not the symptom.",
    };
    let display = category.replace('_', " ");
    format!(
        "---\nname: fix-{dash}\ndescription: \"Auto-evolved from {count}x {category} failures.\"\n---\n\n# Fix {display}\n\nDetected {count} occurrences.\n\n## Remediation\n{remediation}\n\n## Process\n1. Stop — do not retry blindly\n2. Read the full error message and stack trace\n3. Form a hypothesis about root cause\n4. Fix the root cause, not the symptom\n5. Verify with a test or build\n\n## Red Flags\n- Retrying the same approach unchanged\n- Patching symptoms instead of root cause\n",
        dash = category.replace('_', "-"),
    )
}

// ── Phase 5: Trend ──────────────────────────────────

fn compute_trend(history: &[SessionScoreEntry]) -> &'static str {
    let recent: Vec<_> = history.iter().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect();
    if recent.len() < 2 { return "stable"; }
    let n = recent.len() as f64;
    let (mut sx, mut sy, mut sxy, mut sxx) = (0.0, 0.0, 0.0, 0.0);
    for (i, e) in recent.iter().enumerate() {
        let x = i as f64;
        sx += x; sy += e.avg_score; sxy += x * e.avg_score; sxx += x * x;
    }
    let denom = n * sxx - sx * sx;
    if denom.abs() < f64::EPSILON { return "stable"; }
    let slope = (n * sxy - sx * sy) / denom;
    if slope > 0.01 { "improving" } else if slope < -0.01 { "declining" } else { "stable" }
}

// ── Phase 6: Skill Attribution ──────────────────────

fn update_skill_attribution(metrics: &mut Metrics, analysis: &SessionAnalysis, evolved_skills: &[String]) {
    for skill in evolved_skills {
        let attr = metrics.skill_attribution.entry(skill.clone()).or_insert(SkillAttribution {
            skill_name: skill.clone(),
            sessions_active: 0,
            avg_score_with: 0.0,
            avg_score_without: 0.0,
            first_seen: now_iso(),
        });
        attr.sessions_active += 1;
        attr.avg_score_with = round3(
            ((attr.avg_score_with * (attr.sessions_active - 1) as f64) + analysis.avg_score) / attr.sessions_active as f64,
        );
    }

    let total_sessions = metrics.total_sessions + 1;
    // Sum of all composite avg_scores across all sessions (history + current).
    // Use score_history (avg_score field, not avg_success_rate) for historical sessions.
    let all_scores_sum = metrics.score_history.iter().map(|e| e.avg_score).sum::<f64>() + analysis.avg_score;
    for attr in metrics.skill_attribution.values_mut() {
        let without = total_sessions.saturating_sub(attr.sessions_active);
        if without > 0 {
            attr.avg_score_without = round3(
                (all_scores_sum - (attr.avg_score_with * attr.sessions_active as f64)) / without as f64,
            );
        }
    }

    metrics.skill_attribution.retain(|name, _| evolved_skills.contains(name));
}

// ── Phase 7: Cross-project export ───────────────────

fn export_to_global(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) {
    if !cross_project_file().is_file() { return; }
    ensure_dir(&global_harness_dir());

    let project_name = cwd().file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let weak_tools: Vec<String> = analysis.per_tool_stats.iter()
        .filter(|(_, s)| s.total >= 5 && (s.successes as f64 / s.total as f64) < 0.6)
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
    if !evolved.is_dir() { return; }

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
    if remaining.len() > MAX_EVOLVED_SKILLS {
        let excess = &remaining[..remaining.len() - MAX_EVOLVED_SKILLS];
        for name in excess {
            rm_dir(&evolved.join(name));
        }
    }
}

// ── Summary ─────────────────────────────────────────

fn build_summary(analysis: &SessionAnalysis) -> String {
    let mut parts = vec![format!(
        "{} obs, {:.1}% success, avg={}",
        analysis.total_observations, analysis.success_rate * 100.0, analysis.avg_score,
    )];

    let mut top_errors: Vec<_> = analysis.per_error_stats.iter().collect();
    top_errors.sort_by(|a, b| b.1.cmp(a.1));
    top_errors.truncate(3);
    if !top_errors.is_empty() {
        let errs: Vec<String> = top_errors.iter().map(|(c, n)| format!("{c}:{n}")).collect();
        parts.push(format!("errors=[{}]", errs.join(",")));
    }

    if !analysis.failure_patterns.is_empty() {
        let pats: Vec<&str> = analysis.failure_patterns.iter().map(|p| p.pattern_type.as_str()).collect();
        parts.push(format!("patterns=[{}]", pats.join(",")));
    }

    parts.join(" | ")
}

fn round3(v: f64) -> f64 { (v * 1000.0).round() / 1000.0 }

// ── Main Hook ───────────────────────────────────────

pub fn run(_input: &HookInput) -> i32 {
    if !harness_exists() { return 0; }
    if !obs_dir().is_dir() { return 0; }

    // 1. Collect today's observations
    let today_str = today();
    let obs_files: Vec<String> = list_files(&obs_dir(), ".jsonl")
        .into_iter()
        .filter(|f| f.contains(&today_str))
        .collect();
    if obs_files.is_empty() { return 0; }

    let mut observations: Vec<ObsRecord> = vec![];
    for f in &obs_files {
        let recs: Vec<ObsRecord> = read_jsonl_typed(&obs_dir().join(f));
        observations.extend(recs);
    }
    if observations.len() < 3 { return 0; }

    // 2. Analyze
    let mut analysis = analyze_session(&observations);
    analysis.failure_patterns = detect_patterns(&observations);

    // 3. Stagnation
    let mut metrics: Metrics = read_json(&metrics_file(), default_metrics());
    let (should_rollback, improved, rolled_back_count) = check_stagnation(&mut metrics, analysis.avg_score);

    // 4. Seed evolved skills
    ensure_dir(&evolved_dir());
    let existing = list_dirs(&evolved_dir());
    let seeded = if !should_rollback { seed_smart_skills(&analysis, &existing) } else { 0 };

    // 5. Gate
    gate_skills();

    // 6. Skill attribution
    update_skill_attribution(&mut metrics, &analysis, &list_dirs(&evolved_dir()));

    // 7. Cross-project export
    export_to_global(&analysis, &analysis.failure_patterns);

    // 8. Evolution record
    let record = EvolutionRecord {
        timestamp: now_iso(),
        observations: analysis.total_observations,
        success_rate: analysis.success_rate,
        avg_score: analysis.avg_score,
        error_patterns: analysis.per_error_stats.clone(),
        failure_patterns: analysis.failure_patterns.clone(),
        skills_seeded: seeded,
        skills_rolled_back: rolled_back_count,
        total_evolved: list_dirs(&evolved_dir()).len() as u64,
        analysis_summary: build_summary(&analysis),
    };
    append_jsonl(&evolution_file(), &record);

    // 9. Session handoff context
    let last_errors: Vec<String> = observations.iter()
        .filter(|o| o.result.as_deref() == Some("error"))
        .rev().take(3).collect::<Vec<_>>().into_iter().rev()
        .map(|o| {
            let cat = o.failure_category.as_deref().unwrap_or("unknown");
            let snippet = o.error_snippet.as_deref().unwrap_or(o.action.as_deref().unwrap_or(""));
            format!("{cat}: {}", &snippet[..snippet.len().min(100)])
        })
        .collect();
    if !last_errors.is_empty() {
        metrics.last_error_context = Some(last_errors.join(" | "));
    }

    // 10. Update metrics
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
        ((metrics.avg_success_rate * (metrics.total_sessions - 1) as f64) + analysis.success_rate) / metrics.total_sessions as f64,
    );
    metrics.total_evolved_skills = record.total_evolved;
    metrics.last_session = Some(now_iso());

    if improved {
        metrics.best_score = analysis.avg_score;
        metrics.best_session = now_iso();
        metrics.stagnation_count = 0;
    }
    metrics.trend = compute_trend(&metrics.score_history).into();

    if let Ok(json) = serde_json::to_string_pretty(&metrics) {
        let _ = fs::write(metrics_file(), json);
    }

    // 11. Report
    hint("reflect", &format!("Session: {:.1}% success, avg_score={} ({} obs)",
        analysis.success_rate * 100.0, analysis.avg_score, analysis.total_observations));

    let weak_tools: Vec<String> = analysis.per_tool_stats.iter()
        .filter(|(_, s)| s.total >= 5 && (s.successes as f64 / s.total as f64) < 0.6)
        .map(|(cat, s)| format!("{cat} {}%", (s.successes as f64 / s.total as f64 * 100.0) as u32))
        .collect();
    if !weak_tools.is_empty() { hint("reflect", &format!("Weak tools: {}", weak_tools.join(", "))); }

    let weak_exts: Vec<String> = analysis.per_ext_stats.iter()
        .filter(|(_, s)| s.total >= 3 && s.success_rate < 0.5)
        .map(|(ext, s)| format!("{ext} {}%", (s.success_rate * 100.0) as u32))
        .collect();
    if !weak_exts.is_empty() { hint("reflect", &format!("Weak file types: {}", weak_exts.join(", "))); }

    if !analysis.failure_patterns.is_empty() {
        let pats: Vec<String> = analysis.failure_patterns.iter()
            .map(|p| format!("{}({})", p.pattern_type, p.count))
            .collect();
        hint("reflect", &format!("Patterns: {}", pats.join(", ")));
    }

    if seeded > 0 { hint("reflect", &format!("Evolved {seeded} new skill(s)")); }
    if should_rollback { hint("reflect", &format!("Rolled back {rolled_back_count} stagnant skills")); }
    hint("reflect", &format!("Trend: {} ({} sessions)", metrics.trend, metrics.score_history.len()));

    // Skill attribution report
    let effective: Vec<_> = metrics.skill_attribution.values()
        .filter(|a| a.sessions_active >= 2 && a.avg_score_with > a.avg_score_without + 0.02)
        .collect();
    let ineffective: Vec<_> = metrics.skill_attribution.values()
        .filter(|a| a.sessions_active >= 2 && a.avg_score_with < a.avg_score_without - 0.02)
        .collect();
    if !effective.is_empty() {
        let parts: Vec<String> = effective.iter()
            .map(|s| format!("{}(+{}%)", s.skill_name, ((s.avg_score_with - s.avg_score_without) * 100.0) as i32))
            .collect();
        hint("reflect", &format!("Effective skills: {}", parts.join(", ")));
    }
    if !ineffective.is_empty() {
        let names: Vec<&str> = ineffective.iter().map(|s| s.skill_name.as_str()).collect();
        hint("reflect", &format!("Ineffective skills: {} — consider /evolve rollback", names.join(", ")));
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obs(tool: &str, cat: &str, result: &str, score: f64, action: Option<&str>) -> ObsRecord {
        ObsRecord {
            timestamp: "2026-04-09T12:00:00Z".into(),
            tool: tool.into(),
            tool_category: cat.into(),
            action: action.map(String::from),
            result: Some(result.into()),
            score: Some(score),
            dimensions: Some(ScoreDimensions { tool_success: if result == "success" { 1.0 } else { 0.0 }, output_quality: score, execution_cost: 1.0 }),
            failure_category: if result == "error" { Some("type_error".into()) } else { None },
            error_snippet: if result == "error" { Some("TypeError: x is not a function".into()) } else { None },
            file_ext: Some(".ts".into()),
            sequence_id: Some(1),
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
        assert!(patterns.iter().any(|p| p.pattern_type == "repeated_same_error"));
    }

    #[test]
    fn no_repeated_error_below_threshold() {
        let obs = vec![
            make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")),
            make_obs("Bash", "bash", "error", 0.0, Some("/src/main.ts")),
        ];
        let patterns = detect_patterns(&obs);
        assert!(!patterns.iter().any(|p| p.pattern_type == "repeated_same_error"));
    }

    #[test]
    fn detect_long_debug_loop() {
        let mut obs = vec![];
        for _ in 0..6 {
            obs.push(make_obs("Edit", "edit", "success", 0.8, Some("/src/buggy.ts")));
        }
        let patterns = detect_patterns(&obs);
        assert!(patterns.iter().any(|p| p.pattern_type == "long_debug_loop"));
    }

    #[test]
    fn no_debug_loop_below_threshold() {
        let mut obs = vec![];
        for _ in 0..4 {
            obs.push(make_obs("Edit", "edit", "success", 0.8, Some("/src/buggy.ts")));
        }
        let patterns = detect_patterns(&obs);
        assert!(!patterns.iter().any(|p| p.pattern_type == "long_debug_loop"));
    }

    #[test]
    fn detect_thrashing() {
        let mut obs = vec![];
        for _ in 0..4 {
            obs.push(make_obs("Edit", "edit", "success", 0.8, Some("/src/main.ts")));
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
        let history: Vec<SessionScoreEntry> = (0..5).map(|i| SessionScoreEntry {
            timestamp: format!("2026-04-0{}", i + 1),
            success_rate: 0.5 + i as f64 * 0.1,
            avg_score: 0.5 + i as f64 * 0.1,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }).collect();
        assert_eq!(compute_trend(&history), "improving");
    }

    #[test]
    fn trend_declining() {
        let history: Vec<SessionScoreEntry> = (0..5).map(|i| SessionScoreEntry {
            timestamp: format!("2026-04-0{}", i + 1),
            success_rate: 0.9 - i as f64 * 0.1,
            avg_score: 0.9 - i as f64 * 0.1,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }).collect();
        assert_eq!(compute_trend(&history), "declining");
    }

    #[test]
    fn trend_stable_flat() {
        let history: Vec<SessionScoreEntry> = (0..5).map(|i| SessionScoreEntry {
            timestamp: format!("2026-04-0{}", i + 1),
            success_rate: 0.75,
            avg_score: 0.75,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }).collect();
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
        metrics.best_score = 0.7;
        metrics.stagnation_count = 2;
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.8);
        assert!(!rollback);
        assert!(improved);
    }

    #[test]
    fn stagnation_increments_on_no_improvement() {
        let mut metrics = default_metrics();
        metrics.total_sessions = 5;
        metrics.best_score = 0.8;
        metrics.stagnation_count = 0;
        let (rollback, improved, _) = check_stagnation(&mut metrics, 0.78);
        assert!(!rollback);
        assert!(!improved);
        assert_eq!(metrics.stagnation_count, 1);
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
        assert!(body_splitn.starts_with("# Body"), "splitn body: {:?}", body_splitn);
        // splitn(3) body preserves the embedded "---" inside the body
        assert!(body_splitn.contains("more content"), "splitn must preserve full body: {:?}", body_splitn);
        // unlimited split's nth(2) stops at the *body's* "---", truncating it
        assert!(!body_unlimited.contains("more content"), "unlimited split truncates body: {:?}", body_unlimited);
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

        let attr = metrics.skill_attribution.get("evo-test").expect("attribution entry missing");
        assert!((attr.avg_score_with - 0.70).abs() < 0.01, "avg_score_with should be 0.70, got {}", attr.avg_score_with);
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
        let history: Vec<SessionScoreEntry> = (0..5).map(|i| SessionScoreEntry {
            timestamp: format!("2026-04-0{}", i + 1),
            success_rate: 0.80,
            avg_score: 0.80,
            observations: 10,
            dimension_averages: ScoreDimensions::default(),
        }).collect();
        let trend = compute_trend(&history);
        // Must not be "NaN" / must be a valid string
        assert!(trend == "stable" || trend == "improving" || trend == "declining",
            "trend must be a valid value, got: {:?}", trend);
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
        let stats = ExtStats { total: 10, errors: 5, success_rate: 0.5 };
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
}
