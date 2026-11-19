use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
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
    harness_metrics(&state.harness_db, project.as_deref()).await
}

async fn harness_metrics(pool: &AnyPool, project: Option<&str>) -> Result<HarnessMetrics, String> {
    let m = epic_harness::store::metrics::load_metrics_scoped_pool(pool, project)
        .await
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
pub async fn get_orbit_pipelines(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<OrbitPipeline>, String> {
    orbit_pipelines(&state.harness_db, project.as_deref()).await
}

async fn orbit_pipelines(
    pool: &AnyPool,
    project: Option<&str>,
) -> Result<Vec<OrbitPipeline>, String> {
    let pipelines = epic_harness::store::orbit_store::list_pipelines_scoped_pool(
        pool,
        project,
        Some(MAX_ORBIT_PIPELINES),
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
    pub created_at: Option<String>,
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
    evolved_skills(&state.harness_db, project.as_deref()).await
}

async fn evolved_skills(pool: &AnyPool, project: Option<&str>) -> Result<EvolutionData, String> {
    // Evolved skills — list_skills_full_pool is already cross-project;
    // filter in-memory when a specific project is requested.
    let skills = epic_harness::store::evolved::list_skills_full_pool(pool)
        .await
        .map_err(|e| format!("list evolved skills: {e}"))?;
    let evolved_skills: Vec<EvolvedSkill> = skills
        .into_iter()
        .filter(|s| project.is_none_or(|p| s.project == p))
        .map(|s| EvolvedSkill {
            name: s.name,
            origin: s.origin,
            confidence: s.confidence,
            project: s.project,
            active: s.active,
            skill_md: s.skill_md,
            created_at: if s.created.is_empty() {
                None
            } else {
                Some(s.created)
            },
            updated_at: s.updated,
        })
        .collect();

    // Evolution history
    let records = epic_harness::store::evolution::query_recent_records_scoped_pool(
        pool,
        MAX_EVOLUTION_ENTRIES,
        project,
    )
    .await
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
    pub active_agents: Vec<ActiveAgent>,
}

#[derive(Serialize, Deserialize)]
pub struct ActiveAgent {
    pub name: String,
    pub last_tool: String,
    pub last_action: String,
    pub score: f64,
    pub timestamp: String,
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
    obs_summary(&state.harness_db, project.as_deref()).await
}

async fn obs_summary(pool: &AnyPool, project: Option<&str>) -> Result<ObsSummary, String> {
    // Use a wide date range to cover all stored data.
    let stats = epic_harness::store::observations::query_obs_stats_scoped_pool(
        pool,
        "2000-01-01",
        "2099-12-31",
        project,
    )
    .await
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
    session_snapshots(&state.harness_db, project.as_deref()).await
}

async fn session_snapshots(
    pool: &AnyPool,
    project: Option<&str>,
) -> Result<Vec<SessionSnapshotResponse>, String> {
    let snaps = epic_harness::store::sessions::list_recent_snapshots_scoped_pool(pool, 50, project)
        .await
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
            epic_harness::store::global::query_patterns_excluding_pool(&pool, slug, 100).await
        }
        None => epic_harness::store::global::query_all_patterns_pool(&pool, 100).await,
    }
    .map_err(|e| format!("query global patterns: {e}"))?;
    Ok(patterns)
}

// ── HarnessX evolution-engine surfaces ────────────────────────────────────────

/// File-backed readers join the project onto the data root, so a selector from
/// the webview must be a plain slug (same allowlist as the dashboard server).
/// `__all__` means no project, as on the server, so it is never used as a slug.
fn checked_project(project: Option<&str>) -> Result<Option<&str>, String> {
    match project.filter(|p| !p.is_empty() && *p != "__all__") {
        Some(p) if !epic_harness::shared::paths::is_valid_project_slug(p) => {
            Err("invalid project".to_string())
        }
        p => Ok(p),
    }
}

fn to_json(v: impl Serialize) -> Result<serde_json::Value, String> {
    serde_json::to_value(v).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn get_seesaw_registry(project: Option<String>) -> Result<serde_json::Value, String> {
    let p = checked_project(project.as_deref())?;
    to_json(epic_harness::evolve::seesaw::load_registry_for(p))
}

#[tauri::command]
pub async fn get_variant_pool(project: Option<String>) -> Result<serde_json::Value, String> {
    let p = checked_project(project.as_deref())?;
    to_json(epic_harness::evolve::variants::VariantPool::load_for(p))
}

#[tauri::command]
pub async fn get_harness_snapshot(project: Option<String>) -> Result<serde_json::Value, String> {
    let p = checked_project(project.as_deref())?;
    to_json(epic_harness::evolve::snapshot::build_snapshot_for(p))
}

#[tauri::command]
pub async fn get_manifests(project: Option<String>) -> Result<serde_json::Value, String> {
    let p = checked_project(project.as_deref())?;
    to_json(epic_harness::serve::recent_manifests(p))
}

/// Read-only: the desktop app never mutates orchestration state over IPC.
#[tauri::command]
pub async fn get_orchestrator_run(
    project: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let p = checked_project(project.as_deref())?;
    let base = epic_harness::shared::paths::resolve_harness_dir(p);
    epic_harness::orchestrate::state::read_run(&base)
        .map(to_json)
        .transpose()
}

#[tauri::command]
pub async fn get_orchestrator_agent_status(
    project: Option<String>,
    agent_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let p = checked_project(project.as_deref())?;
    if !epic_harness::orchestrate::state::validate_agent_id(&agent_id) {
        return Err("invalid agent id".to_string());
    }
    let base = epic_harness::shared::paths::resolve_harness_dir(p);
    epic_harness::orchestrate::state::read_agent_status(&base, &agent_id)
        .map(to_json)
        .transpose()
}

#[tauri::command]
pub async fn get_adaptation_landscape(
    project: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let p = checked_project(project.as_deref())?;
    // Landscape comes from evolution history alone (no per-session digests).
    let history = epic_harness::store::evolution::query_recent_records_scoped_pool(
        &state.harness_db,
        i64::MAX,
        p,
    )
    .await
    .map_err(|e| format!("query evolution records: {e}"))?;
    to_json(epic_harness::evolve::planner::build_landscape(
        &history,
        &[],
        2,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use epic_harness::shared::evolution::{EditType, EvolutionRecord, Metrics};
    use epic_harness::shared::obs::ObsRecord;
    use epic_harness::shared::types::SessionSnapshot;

    fn obs(tool: &str) -> ObsRecord {
        ObsRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            tool: tool.into(),
            tool_category: "file".into(),
            action: Some("run".into()),
            result: Some("success".into()),
            score: Some(0.9),
            dimensions: None,
            failure_category: None,
            error_snippet: None,
            file_ext: None,
            sequence_id: None,
            pipeline_id: None,
        }
    }

    fn evo(summary: &str) -> EvolutionRecord {
        EvolutionRecord {
            timestamp: "2026-06-02T10:00:00Z".into(),
            observations: 1,
            success_rate: 1.0,
            avg_score: 0.9,
            error_patterns: Default::default(),
            failure_patterns: vec![],
            skills_seeded: 0,
            skills_rolled_back: 0,
            total_evolved: 0,
            analysis_summary: summary.into(),
            edit_type: EditType::default(),
            manifests: vec![],
        }
    }

    fn snap(summary: &str) -> SessionSnapshot {
        SessionSnapshot {
            timestamp: "2026-06-02T10:00:00Z".into(),
            snap_type: "pre-compact".into(),
            summary: summary.into(),
            pending_tasks: vec![],
            context_usage: None,
            pipeline_state: None,
        }
    }

    /// Every command that takes a project must read only that project's rows.
    /// One test fn: it sets HARNESS_DIR (process-global) to a temp dir before
    /// the harness pool is first opened, so the real store is never touched.
    #[test]
    fn commands_return_project_scoped_data() {
        let tmp = std::env::temp_dir().join(format!("epic_tauri_scope_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // SAFETY: this is the only test in this crate and it runs before any
        // other thread reads HARNESS_DIR.
        unsafe { std::env::set_var("HARNESS_DIR", &tmp) };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let pool = epic_harness::store::pool::harness_pool().await.unwrap();

            for (proj, sessions, tool) in [("proj-a", 3u64, "Read"), ("proj-b", 7u64, "Edit")] {
                let m = Metrics {
                    total_sessions: sessions,
                    ..Default::default()
                };
                epic_harness::store::metrics::save_metrics_pool(&pool, &m, proj)
                    .await
                    .unwrap();
                epic_harness::store::observations::insert_observation_pool(
                    &pool,
                    &obs(tool),
                    &format!("20260602_{proj}"),
                    proj,
                )
                .await
                .unwrap();
                epic_harness::store::evolution::insert_record_pool(
                    &pool,
                    &evo(&format!("{proj} analysis")),
                    proj,
                )
                .await
                .unwrap();
                epic_harness::store::sessions::insert_snapshot_pool(
                    &pool,
                    &snap(&format!("{proj} work")),
                    1000,
                    proj,
                )
                .await
                .unwrap();
            }

            let m = harness_metrics(&pool, Some("proj-b")).await.unwrap();
            assert_eq!(m.total_sessions, 7, "metrics scoped to proj-b");

            let o = obs_summary(&pool, Some("proj-b")).await.unwrap();
            assert_eq!(o.total_tool_calls, 1, "obs stats scoped to proj-b");
            assert_eq!(o.tool_stats[0].tool, "Edit");

            let e = evolved_skills(&pool, Some("proj-b")).await.unwrap();
            assert_eq!(e.evolution_history.len(), 1, "history scoped to proj-b");
            assert_eq!(
                e.evolution_history[0]["analysis_summary"],
                "proj-b analysis"
            );

            let s = session_snapshots(&pool, Some("proj-b")).await.unwrap();
            assert_eq!(s.len(), 1, "snapshots scoped to proj-b");
            assert_eq!(s[0].summary, "proj-b work");

            for (proj, id) in [("proj-a", "PIPELINE-1"), ("proj-b", "PIPELINE-2")] {
                epic_harness::store::orbit_store::upsert_pipeline_pool(
                    &pool,
                    id,
                    proj,
                    "running",
                    Some("go"),
                    None,
                    "{}",
                )
                .await
                .unwrap();
            }
            let p = orbit_pipelines(&pool, Some("proj-b")).await.unwrap();
            assert_eq!(p.len(), 1, "orbit pipelines scoped to proj-b");
            assert_eq!(p[0].id, "PIPELINE-2");
            assert_eq!(orbit_pipelines(&pool, None).await.unwrap().len(), 2);

            // No project selected means the cross-project view.
            assert_eq!(obs_summary(&pool, None).await.unwrap().total_tool_calls, 2);
            assert_eq!(session_snapshots(&pool, None).await.unwrap().len(), 2);
        });

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn project_selector_must_be_a_plain_slug() {
        assert_eq!(checked_project(None), Ok(None));
        assert_eq!(checked_project(Some("")), Ok(None));
        assert_eq!(checked_project(Some("__all__")), Ok(None));
        assert_eq!(checked_project(Some("proj-a.1")), Ok(Some("proj-a.1")));
        for bad in ["..", ".", "../x", "a/b", "a\\b", "/abs", "C:"] {
            assert!(checked_project(Some(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn evolution_readers_reject_unsafe_project_before_touching_files() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let bad = || Some("../escape".to_string());
        rt.block_on(async {
            assert!(get_seesaw_registry(bad()).await.is_err());
            assert!(get_variant_pool(bad()).await.is_err());
            assert!(get_harness_snapshot(bad()).await.is_err());
            assert!(get_manifests(bad()).await.is_err());
        });
    }

    #[test]
    fn orchestrator_readers_reject_unsafe_selectors() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let bad = || Some("../x".to_string());
            assert!(get_orchestrator_run(bad()).await.is_err());
            assert_eq!(
                get_orchestrator_agent_status(bad(), "ag-1".into()).await,
                Err("invalid project".to_string())
            );
            assert_eq!(
                get_orchestrator_agent_status(None, "../x".into()).await,
                Err("invalid agent id".to_string())
            );
        });
    }
}
