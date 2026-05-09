use std::collections::HashMap;

use crate::config::CONFIG;
use crate::shared::{classify::extract_file, evolution::*, helpers::*, obs::ObsRecord, scoring::*};

// -- Re-export round3 for use by other evolve submodules --
pub(crate) fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

pub fn analyze_session(observations: &[ObsRecord]) -> SessionAnalysis {
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

pub fn detect_patterns(observations: &[ObsRecord]) -> Vec<DetectedPattern> {
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
                let basename = std::path::Path::new(file)
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
            let basename = std::path::Path::new(file)
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
                let basename = std::path::Path::new(file)
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

pub fn build_summary(analysis: &SessionAnalysis) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::obs::ObsRecord;
    use crate::shared::scoring::ScoreDimensions;
    use std::collections::HashMap;

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

    #[test]
    fn round3_precision() {
        assert_eq!(round3(0.12345), 0.123);
        assert_eq!(round3(0.9999), 1.0);
        assert_eq!(round3(0.0), 0.0);
    }
}
