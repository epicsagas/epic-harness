use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const MAX_SKILL_MD_BYTES: u64 = 512 * 1024; // 512 KB
const MAX_SCORE_HISTORY: usize = 200;
const MAX_EVOLUTION_ENTRIES: usize = 50;
const MAX_OBS_LINES_PER_FILE: usize = 10_000;
const MAX_ORBIT_PIPELINES: usize = 100;

static HARNESS_DIR: OnceLock<Result<PathBuf, String>> = OnceLock::new();

fn harness_project_dir() -> Result<PathBuf, String> {
    HARNESS_DIR
        .get_or_init(|| {
            // Allow explicit override via HARNESS_DIR env var (useful for installed
            // binaries and macOS app bundles where exe-relative path detection fails).
            if let Ok(explicit) = std::env::var("HARNESS_DIR") {
                let p = PathBuf::from(explicit.trim());
                if p.exists() {
                    return Ok(p);
                }
            }

            // Run epic-harness from the repo root (parent of src-tauri), not the
            // Tauri process CWD, so the slug resolves to the real project dir.
            let repo_root = std::env::current_exe()
                .ok()
                .and_then(|p| {
                    // exe is at target/…/epic-harness-dashboard; walk up to find
                    // the directory that contains src-tauri/
                    let mut dir = p.parent()?.to_path_buf();
                    loop {
                        if dir.join("src-tauri").exists() {
                            return Some(dir);
                        }
                        dir = dir.parent()?.to_path_buf();
                    }
                })
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

            let output = std::process::Command::new("epic-harness")
                .arg("path")
                .current_dir(&repo_root)
                .output()
                .map_err(|e| format!("epic-harness not found: {e}"))?;
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                return Err("epic-harness path returned empty".into());
            }
            Ok(PathBuf::from(path))
        })
        .clone()
}

fn tilde_collapse(path: &std::path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let s = path.to_string_lossy();
    if !home.is_empty() && s.starts_with(&home) {
        format!("~{}", &s[home.len()..])
    } else {
        s.into_owned()
    }
}

// ── Metrics ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub struct HarnessMetrics {
    pub score_history: Vec<f64>,
    pub trend: String,
    pub stagnation_count: u32,
    pub session_count: u32,
    pub avg_score: f64,
    pub skill_attribution: serde_json::Value,
    pub score_weights: serde_json::Value,
    // Fields expected by the TS HarnessMetrics interface
    pub total_sessions: u32,
    pub avg_success_rate: f64,
    pub total_evolved_skills: u32,
    pub last_session: Option<String>,
}

#[tauri::command]
pub async fn get_harness_metrics() -> Result<HarnessMetrics, String> {
    let dir = harness_project_dir()?;
    let path = dir.join("metrics.json");
    if !path.exists() {
        return Ok(HarnessMetrics {
            trend: "stable".into(),
            ..Default::default()
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read metrics.json: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse metrics.json: {e}"))?;

    // score_history entries are objects: {timestamp, avg_score, ...}
    // extract avg_score from each entry (fall back to plain f64 for older format)
    let all_scores: Vec<f64> = v["score_history"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    x["avg_score"].as_f64().or_else(|| x.as_f64())
                })
                .collect()
        })
        .unwrap_or_default();
    let total_count = v["total_sessions"].as_u64().unwrap_or(all_scores.len() as u64) as u32;
    let scores: Vec<f64> = all_scores.iter().rev().take(MAX_SCORE_HISTORY).copied().collect::<Vec<_>>().into_iter().rev().collect();
    let avg = if !scores.is_empty() {
        scores.iter().sum::<f64>() / scores.len() as f64
    } else {
        0.0
    };

    // avg_success_rate: prefer explicit field, else derive from score_history success_rate entries
    let avg_success_rate = v["avg_success_rate"].as_f64().unwrap_or_else(|| {
        let rates: Vec<f64> = v["score_history"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["success_rate"].as_f64()).collect())
            .unwrap_or_default();
        if rates.is_empty() { avg } else { rates.iter().sum::<f64>() / rates.len() as f64 }
    });

    // total_evolved_skills: count evolved/ directory entries via the metrics field or default 0
    let total_evolved_skills = v["total_evolved_skills"].as_u64().unwrap_or(0) as u32;

    // last_session: ISO timestamp of the most recent session
    let last_session = v["last_session"].as_str().map(str::to_string);

    Ok(HarnessMetrics {
        score_history: scores,
        trend: v["trend"].as_str().unwrap_or("stable").to_string(),
        stagnation_count: v["stagnation_count"].as_u64().unwrap_or(0) as u32,
        session_count: total_count,
        avg_score: (avg * 1000.0).round() / 1000.0,
        skill_attribution: v["skill_attribution"].clone(),
        score_weights: v["score_weights"].clone(),
        total_sessions: total_count,
        avg_success_rate: (avg_success_rate * 1000.0).round() / 1000.0,
        total_evolved_skills,
        last_session,
    })
}

// ── Orbit Pipelines ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct OrbitPipeline {
    pub id: String,
    pub mode: Option<String>,
    pub phase: String,
    pub status: String,
    pub goal_slug: Option<String>,
    pub branch: Option<String>,
    pub check_fail_count: u32,
    pub started_at: String,
    pub updated_at: String,
    pub deadline: Option<String>,
    pub phase_history: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn get_orbit_pipelines() -> Result<Vec<OrbitPipeline>, String> {
    let dir = harness_project_dir()?;
    let orbit_dir = dir.join("orbit");
    if !orbit_dir.exists() {
        return Ok(vec![]);
    }

    // Collect file names first so we can sort and limit before reading file contents,
    // avoiding unbounded I/O when pipelines accumulate over many sessions.
    let mut pipeline_files: Vec<(String, std::path::PathBuf)> = fs::read_dir(&orbit_dir)
        .map_err(|e| format!("read orbit dir: {e}"))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str())?.to_string();
            if name.starts_with("PIPELINE-") && name.ends_with(".json") {
                Some((name, path))
            } else {
                None
            }
        })
        .collect();

    // Sort descending by filename (encodes timestamp) then cap to avoid excessive I/O
    pipeline_files.sort_by(|a, b| b.0.cmp(&a.0));
    pipeline_files.truncate(MAX_ORBIT_PIPELINES);

    let mut pipelines = Vec::with_capacity(pipeline_files.len());
    for (_, path) in pipeline_files {
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        pipelines.push(OrbitPipeline {
            id: v["id"].as_str().unwrap_or("").to_string(),
            mode: v["mode"].as_str().map(str::to_string),
            phase: v["phase"].as_str().unwrap_or("unknown").to_string(),
            status: v["status"].as_str().unwrap_or("unknown").to_string(),
            goal_slug: v["goal_slug"].as_str().map(str::to_string),
            branch: v["branch"].as_str().map(str::to_string),
            check_fail_count: v["check_fail_count"].as_u64().unwrap_or(0) as u32,
            started_at: v["started_at"].as_str().unwrap_or("").to_string(),
            updated_at: v["updated_at"].as_str().unwrap_or("").to_string(),
            deadline: v["deadline"].as_str().map(str::to_string),
            phase_history: v["phase_history"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        });
    }

    // Already sorted descending by filename; stable sort by started_at for accuracy
    pipelines.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(pipelines)
}

// ── Evolved Skills ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct EvolvedSkill {
    pub name: String,
    pub skill_md: String,
    pub created_at: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct EvolutionData {
    pub evolved_skills: Vec<EvolvedSkill>,
    pub evolution_history: Vec<serde_json::Value>,
    pub total_sessions_analyzed: u32,
    pub patterns_detected: u32,
}

#[tauri::command]
pub async fn get_evolved_skills() -> Result<EvolutionData, String> {
    let dir = harness_project_dir()?;

    // Read evolved skills
    let mut skills = Vec::new();
    let evolved_dir = dir.join("evolved");
    if evolved_dir.exists() {
        let entries = fs::read_dir(&evolved_dir).map_err(|e| format!("read evolved dir: {e}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let skill_md_path = path.join("SKILL.md");
                let skill_md = skill_md_path.metadata()
                    .ok()
                    .filter(|m| m.len() <= MAX_SKILL_MD_BYTES)
                    .and_then(|_| fs::read_to_string(&skill_md_path).ok())
                    .unwrap_or_default();
                let created = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.created().ok())
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs().to_string())
                    });
                skills.push(EvolvedSkill {
                    name: skill_name,
                    skill_md,
                    created_at: created,
                });
            }
        }
    }

    // Read evolution history — stream line-by-line, count all but keep only recent entries
    let mut history: std::collections::VecDeque<serde_json::Value> =
        std::collections::VecDeque::with_capacity(MAX_EVOLUTION_ENTRIES + 1);
    let mut total_sessions = 0u32;
    let mut patterns = 0u32;
    let evo_path = dir.join("evolution.jsonl");
    if evo_path.exists() {
        use std::io::BufRead;
        if let Ok(file) = fs::File::open(&evo_path) {
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                if line.trim().is_empty() { continue; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    total_sessions += 1;
                    if let Some(arr) = v["patterns"].as_array() {
                        patterns += arr.len() as u32;
                    }
                    history.push_back(v);
                    if history.len() > MAX_EVOLUTION_ENTRIES {
                        history.pop_front();
                    }
                }
            }
        }
    }
    // VecDeque is already in order; convert newest-first
    let history_vec: Vec<_> = history.into_iter().rev().collect();

    Ok(EvolutionData {
        evolved_skills: skills,
        evolution_history: history_vec,
        total_sessions_analyzed: total_sessions,
        patterns_detected: patterns,
    })
}

// ── Obs Summary ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub struct ObsSummary {
    pub recent_sessions: Vec<SessionSummary>,
    pub tool_stats: Vec<ToolStat>,
    pub total_tool_calls: u32,
    pub avg_score: f64,
    pub active_agents: Vec<ActiveAgent>,
}

#[derive(Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub date: String,
    pub tool_calls: u32,
    pub avg_score: f64,
    pub failures: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ToolStat {
    pub tool: String,
    pub calls: u32,
    pub success_rate: f64,
    pub avg_score: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ActiveAgent {
    pub name: String,
    pub last_tool: String,
    pub last_action: String,
    pub score: f64,
    pub timestamp: String,
}

#[tauri::command]
pub async fn get_obs_summary() -> Result<ObsSummary, String> {
    let dir = harness_project_dir()?;
    let obs_dir = dir.join("obs");
    if !obs_dir.exists() {
        return Ok(ObsSummary::default());
    }

    let mut newest_session_entries: Vec<serde_json::Value> = Vec::new();
    let mut session_files: Vec<(String, PathBuf)> = Vec::new();

    let entries = fs::read_dir(&obs_dir).map_err(|e| format!("read obs dir: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("session_") && name.ends_with(".jsonl") {
            session_files.push((name.to_string(), path));
        }
    }

    // Sort newest first, take last 10 sessions
    session_files.sort_by(|a, b| b.0.cmp(&a.0));
    let recent_files: Vec<_> = session_files.into_iter().take(10).collect();

    let mut session_summaries = Vec::new();
    let mut tool_map: std::collections::HashMap<String, (u32, f64, u32)> =
        std::collections::HashMap::new();

    for (idx, (fname, path)) in recent_files.iter().enumerate() {
        use std::io::BufRead;
        let mut calls = 0u32;
        let mut score_sum = 0.0f64;
        let mut failures = 0u32;
        let mut session_entries: Vec<serde_json::Value> = Vec::new();

        if let Ok(file) = fs::File::open(path) {
            for line in std::io::BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .take(MAX_OBS_LINES_PER_FILE)
            {
                if line.trim().is_empty() { continue; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    calls += 1;
                    let score = v["score"].as_f64().unwrap_or(0.0);
                    score_sum += score;
                    if v["result"].as_str() != Some("success") {
                        failures += 1;
                    }
                    if let Some(tool) = v["tool"].as_str() {
                        let e = tool_map.entry(tool.to_string()).or_insert((0, 0.0, 0));
                        e.0 += 1;
                        e.1 += score;
                        if v["result"].as_str() != Some("success") {
                            e.2 += 1;
                        }
                    }
                    session_entries.push(v);
                }
            }
        }

        // Capture the most recent session (idx == 0 = newest) for active_agents
        if idx == 0 {
            newest_session_entries = session_entries;
        }
        let fname = fname;

        let date = fname
            .strip_prefix("session_")
            .and_then(|s| s.get(..8))
            .unwrap_or("")
            .to_string();
        let session_id = fname
            .strip_prefix("session_")
            .and_then(|s| s.strip_suffix(".jsonl"))
            .unwrap_or(fname)
            .to_string();

        session_summaries.push(SessionSummary {
            session_id,
            date,
            tool_calls: calls,
            avg_score: if calls > 0 {
                (score_sum / calls as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            },
            failures,
        });
    }

    let total_calls: u32 = session_summaries.iter().map(|s| s.tool_calls).sum();
    let total_score: f64 = session_summaries.iter().map(|s| s.avg_score * s.tool_calls as f64).sum();
    let overall_avg = if total_calls > 0 {
        (total_score / total_calls as f64 * 1000.0).round() / 1000.0
    } else {
        0.0
    };

    let mut tool_stats: Vec<ToolStat> = tool_map
        .into_iter()
        .map(|(tool, (calls, score_sum, failures))| ToolStat {
            tool,
            calls,
            success_rate: if calls > 0 {
                ((calls - failures) as f64 / calls as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            },
            avg_score: if calls > 0 {
                (score_sum / calls as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            },
        })
        .collect();
    tool_stats.sort_by(|a, b| b.calls.cmp(&a.calls));

    // Active agents: last 5 entries from the most recent session (newest_session_entries)
    let active_agents: Vec<ActiveAgent> = newest_session_entries
        .iter()
        .rev()
        .take(5)
        .map(|v| ActiveAgent {
            name: v["tool"].as_str().unwrap_or("unknown").to_string(),
            last_tool: v["tool"].as_str().unwrap_or("").to_string(),
            last_action: v["action"].as_str().unwrap_or("").to_string(),
            score: v["score"].as_f64().unwrap_or(0.0),
            timestamp: v["timestamp"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    Ok(ObsSummary {
        recent_sessions: session_summaries,
        tool_stats,
        total_tool_calls: total_calls,
        avg_score: overall_avg,
        active_agents,
    })
}

// ── Integration Status ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub installed: bool,
    pub config_path: Option<String>,
    pub version: Option<String>,
}

#[tauri::command]
pub async fn get_integration_status() -> Result<Vec<IntegrationStatus>, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/nonexistent".to_string());
    let checks = vec![
        (
            "Claude Code",
            vec![
                format!("{home}/.claude/settings.json"),
                format!("{home}/.claude.json"),
            ],
        ),
        (
            "Codex",
            vec![format!("{home}/.codex/config.toml")],
        ),
        (
            "Gemini CLI",
            vec![format!("{home}/.gemini/settings.json")],
        ),
        (
            "Cursor",
            vec![format!("{home}/.cursor/mcp.json")],
        ),
        (
            "Cline",
            vec![format!("{home}/.vscode/extensions")],
        ),
        (
            "Aider",
            vec![format!("{home}/.aider.conf.yml")],
        ),
    ];

    let results = checks
        .into_iter()
        .map(|(name, paths)| {
            let found = paths.iter().find(|p| std::path::Path::new(p).exists());
            IntegrationStatus {
                name: name.to_string(),
                installed: found.is_some(),
                config_path: found.map(|p| tilde_collapse(std::path::Path::new(p))),
                version: None,
            }
        })
        .collect();

    Ok(results)
}
