use serde::{Deserialize, Serialize};

use crate::state::AppState;
use tauri::State;

const MAX_SCORE_HISTORY: usize = 200;
const MAX_EVOLUTION_ENTRIES: usize = 50;
const MAX_ORBIT_PIPELINES: usize = 100;

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
pub async fn get_harness_metrics(state: State<'_, AppState>) -> Result<HarnessMetrics, String> {
    let db = state.harness_db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;

        let m = epic_harness::store::metrics::load_metrics_conn(&conn)
            .map_err(|e| format!("load metrics: {e}"))?;

        let all_scores: Vec<f64> = m
            .score_history
            .iter()
            .map(|e| e.avg_score)
            .collect();
        let scores: Vec<f64> = all_scores
            .iter()
            .rev()
            .take(MAX_SCORE_HISTORY)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let avg = if !scores.is_empty() {
            scores.iter().sum::<f64>() / scores.len() as f64
        } else {
            0.0
        };

        let skill_attribution: serde_json::Value =
            serde_json::to_value(&m.skill_attribution).unwrap_or(serde_json::Value::Null);

        // Use configured weights, falling back to defaults [0.5, 0.3, 0.2].
        let weights = &epic_harness::config::CONFIG.scoring.weights;
        let score_weights = serde_json::json!({
            "success": weights[0],
            "quality": weights[1],
            "cost": weights[2],
        });

        Ok(HarnessMetrics {
            score_history: scores,
            trend: m.trend,
            stagnation_count: m.stagnation_count as u32,
            session_count: m.total_sessions as u32,
            avg_score: (avg * 1000.0).round() / 1000.0,
            skill_attribution,
            score_weights,
            total_sessions: m.total_sessions as u32,
            avg_success_rate: (m.avg_success_rate * 1000.0).round() / 1000.0,
            total_evolved_skills: m.total_evolved_skills as u32,
            last_session: m.last_session,
        })
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
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
    pub audit_fail_count: u32,
    pub started_at: String,
    pub updated_at: String,
    pub deadline: Option<String>,
    pub phase_history: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn get_orbit_pipelines(
    state: State<'_, AppState>,
) -> Result<Vec<OrbitPipeline>, String> {
    let db = state.harness_db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;

        let pipelines = epic_harness::store::orbit_store::list_all_pipelines_conn_limited(
            &conn,
            MAX_ORBIT_PIPELINES,
        )
        .map_err(|e| format!("list pipelines: {e}"))?;

        let result: Vec<OrbitPipeline> = pipelines
            .into_iter()
            .map(|v| OrbitPipeline {
                id: v["id"].as_str().unwrap_or("").to_string(),
                mode: v["mode"].as_str().map(str::to_string),
                phase: v["phase"].as_str().unwrap_or("unknown").to_string(),
                status: v["status"].as_str().unwrap_or("unknown").to_string(),
                goal_slug: v["goal_slug"].as_str().map(str::to_string),
                branch: v["branch"].as_str().map(str::to_string),
                audit_fail_count: v["audit_fail_count"].as_u64().unwrap_or(0) as u32,
                started_at: v["started_at"].as_str().unwrap_or("").to_string(),
                updated_at: v["updated_at"].as_str().unwrap_or("").to_string(),
                deadline: v["deadline"].as_str().map(str::to_string),
                phase_history: v["phase_history"].as_array().cloned().unwrap_or_default(),
            })
            .collect();

        Ok(result)
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
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
pub async fn get_evolved_skills(state: State<'_, AppState>) -> Result<EvolutionData, String> {
    let db = state.harness_db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;

        // Evolved skills
        let skills = epic_harness::store::evolved::list_skills_full_conn(&conn)
            .map_err(|e| format!("list evolved skills: {e}"))?;
        let evolved_skills: Vec<EvolvedSkill> = skills
            .into_iter()
            .map(|s| EvolvedSkill {
                name: s.name,
                skill_md: s.skill_md,
                created_at: Some(s.created),
            })
            .collect();

        // Evolution history
        let records = epic_harness::store::evolution::query_recent_records_conn(
            &conn,
            MAX_EVOLUTION_ENTRIES,
        )
        .map_err(|e| format!("query evolution records: {e}"))?;

        let total_sessions = records.len() as u32;
        let patterns_detected: u32 = records
            .iter()
            .map(|r| r.error_patterns.len() as u32 + r.failure_patterns.len() as u32)
            .sum();

        let history: Vec<serde_json::Value> = records
            .into_iter()
            .rev()
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect();

        Ok(EvolutionData {
            evolved_skills,
            evolution_history: history,
            total_sessions_analyzed: total_sessions,
            patterns_detected,
        })
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
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
pub async fn get_obs_summary(state: State<'_, AppState>) -> Result<ObsSummary, String> {
    let db = state.harness_db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;

        // Use a wide date range to cover all stored data.
        // SQLite treats these as string comparisons; the padded format ensures
        // correct lexicographic ordering against ISO-8601 timestamps.
        let stats = epic_harness::store::observations::query_obs_stats_conn(
            &conn,
            "2000-01-01",
            "2099-12-31",
        )
        .map_err(|e| format!("query obs stats: {e}"))?;

        // Session summaries from aggregate stats
        let session_summaries: Vec<SessionSummary> = stats
            .session_stats
            .iter()
            .map(|s| {
                let date = s.session_id.get(..8).unwrap_or("").to_string();
                let calls = s.calls as u32;
                let failures = s.failures as u32;
                SessionSummary {
                    session_id: s.session_id.clone(),
                    date,
                    tool_calls: calls,
                    avg_score: (s.avg_score * 1000.0).round() / 1000.0,
                    failures,
                }
            })
            .collect();

        let total_calls = stats.total as u32;

        let tool_stats: Vec<ToolStat> = stats
            .tool_stats
            .iter()
            .map(|t| ToolStat {
                tool: t.tool.clone(),
                calls: t.calls as u32,
                success_rate: if t.calls > 0 {
                    ((t.successes as f64 / t.calls as f64) * 1000.0).round() / 1000.0
                } else {
                    0.0
                },
                avg_score: (t.avg_score * 1000.0).round() / 1000.0,
            })
            .collect();

        // Active agents: last 5 observations
        let recent = epic_harness::store::observations::query_latest_observations_conn(&conn, 5)
            .map_err(|e| format!("query latest observations: {e}"))?;

        let active_agents: Vec<ActiveAgent> = recent
            .into_iter()
            .map(|r| ActiveAgent {
                name: r.tool.clone(),
                last_tool: r.tool,
                last_action: r.action.unwrap_or_default(),
                score: r.score.unwrap_or(0.0),
                timestamp: r.timestamp,
            })
            .collect();

        Ok(ObsSummary {
            recent_sessions: session_summaries,
            tool_stats,
            total_tool_calls: total_calls,
            avg_score: (stats.avg_score * 1000.0).round() / 1000.0,
            active_agents,
        })
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}

// ── Integration Status ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub installed: bool,
    pub config_path: Option<String>,
    pub version: Option<String>,
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
        ("Codex", vec![format!("{home}/.codex/config.toml")]),
        (
            "Antigravity",
            vec![format!("{home}/.gemini/config/mcp_config.json")],
        ),
        ("Cursor", vec![format!("{home}/.cursor/mcp.json")]),
        ("Cline", vec![format!("{home}/.vscode/extensions")]),
        ("Aider", vec![format!("{home}/.aider.conf.yml")]),
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
