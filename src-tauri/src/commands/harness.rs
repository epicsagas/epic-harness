use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

const MAX_SCORE_HISTORY: usize = 200;
const MAX_EVOLUTION_ENTRIES: i64 = 50;
const MAX_ORBIT_PIPELINES: i64 = 100;

// ── Metrics ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub struct HarnessMetrics {
    pub score_history: Vec<epic_harness::shared::evolution::SessionScoreEntry>,
    pub trend: String,
    pub stagnation_count: u32,
    pub session_count: u32,
    pub avg_score: f64,
    pub skill_attribution: serde_json::Value,
    // Fields expected by the TS HarnessMetrics interface
    pub total_sessions: u32,
    pub avg_success_rate: f64,
    pub total_evolved_skills: u32,
    pub last_session: Option<String>,
    pub best_score: Option<f64>,
    pub best_session: Option<String>,
    pub last_error_context: Option<String>,
}

#[tauri::command]
pub async fn get_harness_metrics(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<HarnessMetrics, String> {
    let pool = state.harness_db.clone();

    let m = match project {
        Some(ref slug) => epic_harness::store::metrics::load_metrics_pool(&pool, slug).await,
        None => epic_harness::store::metrics::load_metrics_all_pool(&pool).await,
    }
    .map_err(|e| format!("load metrics: {e}"))?;

    // Keep the most recent MAX_SCORE_HISTORY entries (sorted chronologically).
    let history: Vec<_> = m
        .score_history
        .into_iter()
        .rev()
        .take(MAX_SCORE_HISTORY)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let avg = if !history.is_empty() {
        history.iter().map(|e| e.avg_score).sum::<f64>() / history.len() as f64
    } else {
        0.0
    };

    let skill_attribution: serde_json::Value =
        serde_json::to_value(&m.skill_attribution).unwrap_or(serde_json::Value::Null);

    Ok(HarnessMetrics {
        score_history: history,
        trend: m.trend,
        stagnation_count: m.stagnation_count as u32,
        session_count: m.total_sessions as u32,
        avg_score: (avg * 1000.0).round() / 1000.0,
        skill_attribution,
        total_sessions: m.total_sessions as u32,
        avg_success_rate: (m.avg_success_rate * 1000.0).round() / 1000.0,
        total_evolved_skills: m.total_evolved_skills as u32,
        last_session: m.last_session,
        best_score: m.best_score,
        best_session: Some(m.best_session).filter(|s| !s.is_empty()),
        last_error_context: m.last_error_context,
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
    pub audit_fail_count: u32,
    pub started_at: String,
    pub updated_at: String,
    pub deadline: Option<String>,
    pub phase_history: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn get_orbit_pipelines(state: State<'_, AppState>) -> Result<Vec<OrbitPipeline>, String> {
    let pool = state.harness_db.clone();

    let pipelines = epic_harness::store::orbit_store::list_all_pipelines_pool_limited(
        &pool,
        MAX_ORBIT_PIPELINES,
    )
    .await
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
}

// ── Evolved Skills ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct EvolvedSkill {
    pub name: String,
    pub origin: String,
    pub confidence: f64,
    pub project: String,
    pub active: bool,
    pub skill_md: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct EvolutionData {
    pub evolved_skills: Vec<EvolvedSkill>,
    pub evolution_history: Vec<serde_json::Value>,
    pub total_sessions_analyzed: u32,
    pub patterns_detected: u32,
}

#[tauri::command]
pub async fn get_evolved_skills(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<EvolutionData, String> {
    let pool = state.harness_db.clone();

    // Evolved skills — list_skills_full_pool is already cross-project;
    // filter in-memory when a specific project is requested.
    let skills = epic_harness::store::evolved::list_skills_full_pool(&pool)
        .await
        .map_err(|e| format!("list evolved skills: {e}"))?;
    let evolved_skills: Vec<EvolvedSkill> = skills
        .into_iter()
        .filter(|s| project.as_ref().is_none_or(|p| s.project == *p))
        .map(|s| EvolvedSkill {
            name: s.name,
            origin: s.origin,
            confidence: s.confidence,
            project: s.project,
            active: s.active,
            skill_md: s.skill_md,
            created_at: s.created,
            updated_at: s.updated,
        })
        .collect();

    // Evolution history
    let records = match project {
        Some(ref slug) => {
            epic_harness::store::evolution::query_recent_records_pool(
                &pool,
                slug,
                MAX_EVOLUTION_ENTRIES,
            )
            .await
        }
        None => {
            epic_harness::store::evolution::query_recent_records_all_pool(
                &pool,
                MAX_EVOLUTION_ENTRIES,
            )
            .await
        }
    }
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
}

// ── Obs Summary ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub struct ObsSummary {
    pub recent_sessions: Vec<SessionSummary>,
    pub tool_stats: Vec<ToolStat>,
    pub total_tool_calls: u32,
    pub avg_score: f64,
    pub failure_categories: Vec<FailureCategory>,
    #[serde(default)]
    pub active_agents: Vec<serde_json::Value>,
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
pub struct FailureCategory {
    pub category: String,
    pub count: u32,
}

#[tauri::command]
pub async fn get_obs_summary(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<ObsSummary, String> {
    let pool = state.harness_db.clone();

    // Use a wide date range to cover all stored data.
    let stats = match project {
        Some(ref slug) => {
            epic_harness::store::observations::query_obs_stats_pool(
                &pool,
                slug,
                "2000-01-01",
                "2099-12-31",
            )
            .await
        }
        None => {
            epic_harness::store::observations::query_obs_stats_all_pool(
                &pool,
                "2000-01-01",
                "2099-12-31",
            )
            .await
        }
    }
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

    let failure_categories: Vec<FailureCategory> = stats
        .error_stats
        .iter()
        .map(|(cat, cnt)| FailureCategory {
            category: cat.clone(),
            count: *cnt as u32,
        })
        .collect();

    Ok(ObsSummary {
        recent_sessions: session_summaries,
        tool_stats,
        total_tool_calls: total_calls,
        avg_score: (stats.avg_score * 1000.0).round() / 1000.0,
        failure_categories,
        active_agents: vec![],
    })
}

// ── Session Snapshots ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct SessionSnapshotResponse {
    pub timestamp: String,
    #[serde(rename = "type")]
    pub snap_type: String,
    pub summary: String,
    pub pending_tasks: Vec<String>,
    pub context_usage: Option<f64>,
    pub pipeline_state: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn get_session_snapshots(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSnapshotResponse>, String> {
    let pool = state.harness_db.clone();
    let snaps = match project {
        Some(ref slug) => {
            epic_harness::store::sessions::list_recent_snapshots_pool(&pool, slug, 50).await
        }
        None => epic_harness::store::sessions::list_recent_snapshots_all_pool(&pool, 50).await,
    }
    .map_err(|e| format!("query session snapshots: {e}"))?;

    Ok(snaps
        .into_iter()
        .map(|s| SessionSnapshotResponse {
            timestamp: s.timestamp,
            snap_type: s.snap_type,
            summary: s.summary,
            pending_tasks: s.pending_tasks,
            context_usage: s.context_usage,
            pipeline_state: s.pipeline_state,
        })
        .collect())
}

// ── Global Patterns ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_global_patterns(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let pool = state.harness_db.clone();
    let patterns = match project {
        Some(ref slug) => {
            // Cross-project patterns (excluding current project)
            epic_harness::store::global::query_patterns_excluding_pool(&pool, slug, 100)
                .await
        }
        None => {
            epic_harness::store::global::query_all_patterns_pool(&pool, 100)
                .await
        }
    }
    .map_err(|e| format!("query global patterns: {e}"))?;
    Ok(patterns)
}

// ── Project List ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_projects() -> Result<Vec<String>, String> {
    Ok(epic_harness::shared::paths::list_harness_project_slugs())
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
