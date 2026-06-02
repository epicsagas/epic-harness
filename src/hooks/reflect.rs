use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

use super::common::*;
use crate::config::CONFIG;
use crate::episteme_client::{self, InsightPayload};
use crate::evolve;
use crate::mem::store;
use crate::shared::{evolution::*, helpers::*, obs::ObsRecord, paths::*};
use crate::telemetry::{SessionTrend, Telemetry};

static TELEMETRY: LazyLock<Telemetry> = LazyLock::new(Telemetry::init);

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
    if let Some(ref s) = since
        && (s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()))
    {
        eprintln!("{{\"error\":\"--since must be YYYYMMDD format, got: {s}\"}}");
        return 1;
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

    // Collect obs from all target project slugs — try SQLite first, fall back to JSONL
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

        // Try SQLite first (primary source after migration)
        let sqlite_obs = crate::store::open_harness_db()
            .ok()
            .and_then(|conn| {
                crate::store::observations::query_obs_for_date_range_conn(
                    &conn, &date_from, &date_to, None,
                )
                .ok()
            })
            .unwrap_or_default();

        let recs: Vec<ObsRecord> = if !sqlite_obs.is_empty() {
            sqlite_obs
        } else {
            // Fallback to JSONL files
            let slug_obs_dir = slug_harness.join("obs");
            if !slug_obs_dir.is_dir() {
                continue;
            }
            let all_obs = list_files(&slug_obs_dir, ".jsonl");
            let filtered: Vec<String> = all_obs
                .into_iter()
                .filter(|f| {
                    let tag = f.replace("session_", "");
                    tag.get(..8)
                        .map(|s| s >= cutoff_tag.as_str())
                        .unwrap_or(true)
                })
                .collect();
            let mut combined: Vec<ObsRecord> = Vec::new();
            for f in &filtered {
                combined.extend(read_jsonl_typed::<ObsRecord>(&slug_obs_dir.join(f)));
            }
            combined
        };

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

    fn round3(v: f64) -> f64 {
        (v * 1000.0).round() / 1000.0
    }

    fn round4(v: f64) -> f64 {
        (v * 10000.0).round() / 10000.0
    }

    let avg_score = if scores.is_empty() {
        0.0
    } else {
        round3(scores.iter().sum::<f64>() / scores.len() as f64)
    };
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
    let tool_success_rate = if total_calls == 0 {
        0.0
    } else {
        round3(total_success as f64 / total_calls as f64)
    };

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

    // 2. Evolution stats (SQLite first, fallback to JSONL)
    let evo_records: Vec<serde_json::Value> = if let Ok(conn) = crate::store::open_harness_db() {
        match crate::store::evolution::query_all_records_conn(&conn) {
            Ok(recs) => recs
                .iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect(),
            Err(e) => {
                eprintln!("[reflect] SQLite evolution read failed, falling back to JSONL: {e}");
                read_jsonl_typed::<serde_json::Value>(&evolution_file())
            }
        }
    } else {
        eprintln!("[reflect] harness.db unavailable for evolution records, falling back to JSONL");
        read_jsonl_typed::<serde_json::Value>(&evolution_file())
    };
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
            && !t.is_empty()
        {
            trend_hist.push(t.to_string());
        }
        skills_generated += r
            .get("skills_generated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if r.get("stagnation_triggered")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
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
        recent_weak
            .into_iter()
            .filter(|s| seen.insert(s.clone()))
            .collect()
    };
    let recent_seeded: Vec<String> = recent_evo
        .iter()
        .filter_map(|r| r.get("seeded_skills").and_then(|v| v.as_array()))
        .flat_map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)))
        .collect();
    let recent_seeded: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        recent_seeded
            .into_iter()
            .filter(|s| seen.insert(s.clone()))
            .collect()
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

    // 3. Metrics summary (SQLite first, fallback to JSON)
    let metrics: Metrics = if let Ok(conn) = crate::store::open_harness_db() {
        match crate::store::metrics::load_metrics_conn(&conn) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[reflect] SQLite metrics read failed, falling back to JSON: {e}");
                read_json(&metrics_file(), default_metrics())
            }
        }
    } else {
        eprintln!("[reflect] harness.db unavailable for metrics, falling back to JSON");
        read_json(&metrics_file(), default_metrics())
    };
    let sh = &metrics.score_history;
    let score_trend_delta: f64 = if sh.len() >= 3 {
        let recent: Vec<f64> = sh.iter().rev().take(10).map(|s| s.avg_score).collect();
        let deltas: Vec<f64> = recent.windows(2).map(|w| w[0] - w[1]).collect();
        if deltas.is_empty() {
            0.0
        } else {
            round4(deltas.iter().sum::<f64>() / deltas.len() as f64)
        }
    } else {
        0.0
    };

    let score_comparison = if sh.len() >= 10 {
        let first5: f64 = sh.iter().take(5).map(|s| s.avg_score).sum::<f64>() / 5.0;
        let last5: f64 = sh.iter().rev().take(5).map(|s| s.avg_score).sum::<f64>() / 5.0;
        let dir = if last5 > first5 {
            "improving"
        } else if last5 < first5 {
            "declining"
        } else {
            "stable"
        };
        Some(serde_json::json!({
            "first_5_avg": round3(first5),
            "last_5_avg": round3(last5),
            "direction": dir,
            "delta": round3(last5 - first5),
        }))
    } else {
        None
    };

    let skill_attr: serde_json::Map<String, serde_json::Value> = metrics
        .skill_attribution
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                serde_json::json!({
                    "sessions_active": v.sessions_active,
                    "avg_score_with": v.avg_score_with,
                    "avg_score_without": v.avg_score_without,
                    "delta": round3(v.avg_score_with - v.avg_score_without),
                }),
            )
        })
        .collect();

    let latest_dims = sh
        .last()
        .map(|s| serde_json::to_value(s.dimension_averages).unwrap_or_default())
        .unwrap_or_default();

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

    // 4. Session snapshots (SQLite first, fallback to JSON)
    let snapshots: Vec<serde_json::Value> = if let Ok(conn) = crate::store::open_harness_db() {
        match crate::store::sessions::list_recent_snapshots_conn(&conn, 5) {
            Ok(snaps) => snaps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "timestamp": s.timestamp,
                        "type": s.snap_type,
                        "summary": s.summary.chars().take(400).collect::<String>(),
                    })
                })
                .collect(),
            Err(e) => {
                eprintln!("[reflect] SQLite sessions read failed, falling back to JSON: {e}");
                vec![]
            }
        }
    } else {
        let snap_files = list_files(&sessions_dir(), ".json");
        snap_files.iter().rev().take(5).filter_map(|f| {
            let sp: serde_json::Value = read_json(&sessions_dir().join(f), serde_json::Value::Null);
            if sp.is_null() { return None; }
            Some(serde_json::json!({
                "timestamp": sp.get("timestamp").and_then(|v| v.as_str()).unwrap_or(""),
                "type": sp.get("type").and_then(|v| v.as_str()).unwrap_or(""),
                "summary": sp.get("summary").and_then(|v| v.as_str()).unwrap_or("").chars().take(400).collect::<String>(),
            }))
        }).collect()
    };

    // 5. Evolved skills
    let evolved_list_dirs = list_dirs(&evolved_dir());
    let mut evolved_list: Vec<serde_json::Value> = Vec::new();
    let mut by_type: HashMap<&str, u64> = HashMap::new();
    for name in &evolved_list_dirs {
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
            "total": evolved_list_dirs.len(),
            "by_type": {
                "preset": by_type.get("preset").copied().unwrap_or(0),
                "evolved": by_type.get("evolved").copied().unwrap_or(0),
                "auto-evolved": by_type.get("auto-evolved").copied().unwrap_or(0),
            }
        },
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
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
    let recalled = match store::smart_recall_conn(
        &conn,
        project_filter,
        Some("decision pattern error resolution concept"),
        30,
    ) {
        Ok(s) => s,
        Err(e) => return serde_json::json!({"error": format!("recall failed: {e}")}),
    };

    // Also pull top decisions/resolutions explicitly (high-value types)
    let decisions = store::query_nodes_conn(
        &conn,
        None, // tag filter
        Some("decision"),
        project_filter,
        10,
    )
    .unwrap_or_default();
    let resolutions = store::query_nodes_conn(&conn, None, Some("resolution"), project_filter, 10)
        .unwrap_or_default();

    // Merge and deduplicate by id, prefer higher-importance entry
    let mut seen: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

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
        seen.entry(id.clone()).or_insert_with(|| {
            serde_json::json!({
                "id": id,
                "type": node.frontmatter.node_type,
                "title": node.frontmatter.title,
                "importance": node.frontmatter.importance,
                "tags": node.frontmatter.tags,
                "updated": node.frontmatter.updated,
                "body_preview": node.body.chars().take(200).collect::<String>(),
            })
        });
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

fn collect_alcove(cfg: &crate::config::AlcoveConfig) -> serde_json::Value {
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

// ── Main Hook ───────────────────────────────────────

pub fn run(_input: &HookInput) -> i32 {
    if !should_run(PROFILE_REFLECT) {
        return 0;
    }
    if !harness_exists() {
        return 0;
    }

    // 1. Collect today's observations from SQLite (fallback to JSONL)
    let today_str = today();
    let observations = if let Ok(conn) = crate::store::open_harness_db() {
        match crate::store::observations::query_obs_for_date_range_conn(
            &conn, &today_str, &today_str, None,
        ) {
            Ok(recs) => recs,
            Err(e) => {
                eprintln!("[reflect] SQLite observations read failed, falling back to JSONL: {e}");
                // Fallthrough to JSONL path below
                return 0;
            }
        }
    } else {
        // Fallback: read from JSONL files
        if !obs_dir().is_dir() {
            return 0;
        }
        let obs_files: Vec<String> = list_files(&obs_dir(), ".jsonl")
            .into_iter()
            .filter(|f| f.contains(&today_str))
            .collect();
        if obs_files.is_empty() {
            return 0;
        }
        let mut recs: Vec<ObsRecord> = vec![];
        for f in &obs_files {
            recs.extend(read_jsonl_typed(&obs_dir().join(f)));
        }
        recs
    };
    if observations.len() < 3 {
        return 0;
    }

    // 2. Analyze
    let mut analysis = evolve::analyze_session(&observations);
    analysis.failure_patterns = evolve::detect_patterns(&observations);

    // 3. Stagnation (load metrics from SQLite, fallback to JSON)
    let mut metrics: Metrics = if let Ok(conn) = crate::store::open_harness_db() {
        match crate::store::metrics::load_metrics_conn(&conn) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[reflect] SQLite metrics load failed, falling back to JSON: {e}");
                read_json(&metrics_file(), default_metrics())
            }
        }
    } else {
        eprintln!("[reflect] harness.db unavailable for metrics load, falling back to JSON");
        read_json(&metrics_file(), default_metrics())
    };
    let (should_rollback, improved, rolled_back_count) =
        evolve::check_stagnation(&mut metrics, analysis.avg_score);

    // 4. Seed evolved skills
    ensure_dir(&evolved_dir());
    let existing = list_dirs(&evolved_dir());
    let seeded = if !should_rollback {
        evolve::seed_smart_skills(&analysis, &existing)
    } else {
        0
    };

    // 5. Gate
    evolve::gate_skills();

    // 6. Skill attribution (reuse listing after gate may have pruned)
    let evolved_dirs = list_dirs(&evolved_dir());
    evolve::update_skill_attribution(&mut metrics, &analysis, &evolved_dirs);

    // 7. Cross-project export
    evolve::export_to_global(&analysis, &analysis.failure_patterns);

    // 8. Memory auto-ingest (knowledge graph)
    let (mem_nodes, mem_edges) = evolve::ingest_to_memory(&analysis, &analysis.failure_patterns);

    // 8.5a. Episteme ingest — send key insights to the knowledge graph
    // Runs in parallel with instinct extraction via early-return on any failure.
    // Graceful degradation: Episteme errors are non-fatal.
    let episteme_ok = ingest_to_episteme(&analysis);
    if !episteme_ok {
        hint(
            "reflect",
            "Episteme: ingest skipped (binary unavailable or error)",
        );
    }

    // 8.5. Instinct extraction and promotion
    let instincts = evolve::extract_instincts(&observations, &analysis);
    let instincts_promoted = if !instincts.is_empty() {
        evolve::promote_instincts_to_global(&instincts)
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
        analysis_summary: evolve::build_summary(&analysis),
    };
    // Write evolution record to SQLite (primary) + JSONL (fallback)
    if let Ok(conn) = crate::store::open_harness_db() {
        if let Err(e) = crate::store::evolution::insert_record_conn(&conn, &record) {
            eprintln!("[reflect] SQLite evo write failed: {e}");
        }
    }
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
    fn round3(v: f64) -> f64 {
        (v * 1000.0).round() / 1000.0
    }

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
    metrics.trend = evolve::compute_trend(&metrics.score_history).into();

    // Save metrics to SQLite (primary) + JSON file (fallback)
    if let Ok(conn) = crate::store::open_harness_db() {
        let _ = crate::store::metrics::save_metrics_conn(&conn, &metrics);
    }
    if let Ok(json) = serde_json::to_string_pretty(&metrics) {
        let _ = fs::write(metrics_file(), json);
    }

    // 11.5. Workspace manifest
    evolve::write_workspace_manifest();

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
        evolve::safe_avg_score(analysis.avg_score),
        analysis.total_observations,
        metrics.trend.parse().unwrap_or(SessionTrend::Stable),
        seeded,
    );

    0
}

// ── Episteme Integration ────────────────────────────────

/// Ingest session insights into the Episteme knowledge graph via `add_insight`.
///
/// Builds a concise natural-language summary and forwards it with metadata.
/// Returns `true` if the call succeeded (or if there was nothing to ingest),
/// `false` on any Episteme-side error. Errors are non-fatal by design.
fn ingest_to_episteme(analysis: &SessionAnalysis) -> bool {
    // Skip if session had very few observations (not worth persisting)
    if analysis.total_observations < 3 {
        return true;
    }

    let slug = project_slug();

    // Build insight text: concise summary of the session
    let weak_tools: Vec<String> = analysis
        .per_tool_stats
        .iter()
        .filter(|(_, s)| {
            s.total >= CONFIG.pattern.weak_tool_min_obs
                && (s.successes as f64 / s.total as f64) < CONFIG.pattern.weak_tool_rate
        })
        .map(|(t, s)| {
            format!(
                "{} ({:.0}%)",
                t,
                s.successes as f64 / s.total as f64 * 100.0
            )
        })
        .collect();

    let pattern_names: Vec<String> = analysis
        .failure_patterns
        .iter()
        .map(|p| format!("{}({}x)", p.pattern_type, p.count))
        .collect();

    let mut parts: Vec<String> = vec![format!(
        "Session in `{}`: {:.1}% success rate, avg_score={:.3} ({} observations).",
        slug,
        analysis.success_rate * 100.0,
        analysis.avg_score,
        analysis.total_observations
    )];

    if !weak_tools.is_empty() {
        parts.push(format!("Weak tools: {}.", weak_tools.join(", ")));
    }
    if !pattern_names.is_empty() {
        parts.push(format!("Failure patterns: {}.", pattern_names.join(", ")));
    }
    // Top error categories
    let mut top_errors: Vec<(&String, &u64)> = analysis.per_error_stats.iter().collect();
    top_errors.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let top_errors: Vec<String> = top_errors
        .iter()
        .take(3)
        .map(|(cat, count)| format!("{cat}({}x)", count))
        .collect();
    if !top_errors.is_empty() {
        parts.push(format!("Top errors: {}.", top_errors.join(", ")));
    }

    let insight_text = parts.join(" ");

    // Tags: project slug + detected pattern types
    let mut tags: Vec<String> = vec!["auto".to_string(), "session-reflect".to_string()];
    for p in &analysis.failure_patterns {
        tags.push(p.pattern_type.clone());
    }

    // Confidence: composite_score × pattern density factor
    let confidence =
        episteme_client::compute_confidence(analysis.avg_score, analysis.failure_patterns.len());

    let payload = InsightPayload {
        text: insight_text,
        tags,
        linked_entities: vec![], // Episteme auto-detects links from text
        project: slug,
        confidence,
    };

    match episteme_client::add_insight(&payload) {
        Ok(id) => {
            hint("reflect", &format!("Episteme: insight recorded (id={id})"));
            true
        }
        Err(e) => {
            // Non-fatal: log but do not abort the hook
            hint("reflect", &format!("Episteme: {e}"));
            false
        }
    }
}

// ── Inline tests (kept here: run_context epoch helpers) ──
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_days_to_ymd_known_date() {
        // 1970-01-01 = day 0
        assert_eq!(epoch_days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn epoch_days_to_ymd_mid_year() {
        // Approximate check: day 180 should be around June/July 1970
        let (y, m, _d) = epoch_days_to_ymd(180);
        assert_eq!(y, 1970);
        assert!(
            (6..=7).contains(&m),
            "month should be June or July, got {m}"
        );
    }

    // ── run_context signature ──────────────────────────
    #[test]
    fn effective_sources_all_expands() {
        let sources: Vec<String> = vec!["all".into()];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "mem", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness", "mem"]
        } else {
            let mut v: Vec<&str> = vec!["harness", "mem"];
            for s in sources.iter() {
                let s = s.as_str();
                if s != "harness" && s != "mem" {
                    v.push(s);
                }
            }
            v
        };
        assert_eq!(
            effective,
            vec!["harness", "mem", "claude-session", "alcove"]
        );
    }

    #[test]
    fn effective_sources_empty_defaults_to_harness() {
        let sources: Vec<String> = vec![];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "mem", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness", "mem"]
        } else {
            let mut v: Vec<&str> = vec!["harness", "mem"];
            for s in sources.iter() {
                let s = s.as_str();
                if s != "harness" && s != "mem" {
                    v.push(s);
                }
            }
            v
        };
        assert_eq!(effective, vec!["harness", "mem"]);
    }

    #[test]
    fn effective_sources_explicit_list_passthrough() {
        let sources: Vec<String> = vec!["harness".into(), "alcove".into()];
        let effective: Vec<&str> = if sources.contains(&"all".to_string()) {
            vec!["harness", "mem", "claude-session", "alcove"]
        } else if sources.is_empty() {
            vec!["harness", "mem"]
        } else {
            let mut v: Vec<&str> = vec!["harness", "mem"];
            for s in sources.iter() {
                let s = s.as_str();
                if s != "harness" && s != "mem" {
                    v.push(s);
                }
            }
            v
        };
        assert_eq!(effective, vec!["harness", "mem", "alcove"]);
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
