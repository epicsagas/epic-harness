use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::scoring::ScoreDimensions;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStats {
    pub tool_category: String,
    pub total: u64,
    pub successes: u64,
    pub errors: u64,
    pub avg_score: f64,
    pub failure_categories: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub pattern_type: String,
    pub description: String,
    pub count: u64,
    pub involved_files: Vec<String>,
    pub suggested_remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtStats {
    pub total: u64,
    pub errors: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionAnalysis {
    pub total_observations: u64,
    pub success_rate: f64,
    pub avg_score: f64,
    pub score_distribution: HashMap<String, u64>,
    pub per_tool_stats: HashMap<String, ToolStats>,
    pub per_error_stats: HashMap<String, u64>,
    pub per_ext_stats: HashMap<String, ExtStats>,
    pub failure_patterns: Vec<DetectedPattern>,
    pub dimension_averages: ScoreDimensions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionScoreEntry {
    pub timestamp: String,
    pub success_rate: f64,
    pub avg_score: f64,
    pub observations: u64,
    pub dimension_averages: ScoreDimensions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAttribution {
    pub skill_name: String,
    pub sessions_active: u64,
    pub avg_score_with: f64,
    pub avg_score_without: f64,
    pub first_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub total_sessions: u64,
    pub avg_success_rate: f64,
    pub total_evolved_skills: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_session: Option<String>,
    pub score_history: Vec<SessionScoreEntry>,
    pub best_score: Option<f64>,
    pub best_session: String,
    pub trend: String,
    pub stagnation_count: u64,
    #[serde(default)]
    pub skill_attribution: HashMap<String, SkillAttribution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionRecord {
    pub timestamp: String,
    pub observations: u64,
    pub success_rate: f64,
    pub avg_score: f64,
    pub error_patterns: HashMap<String, u64>,
    pub failure_patterns: Vec<DetectedPattern>,
    pub skills_seeded: u64,
    pub skills_rolled_back: u64,
    pub total_evolved: u64,
    pub analysis_summary: String,
}

pub fn default_metrics() -> Metrics {
    Metrics {
        total_sessions: 0,
        avg_success_rate: 0.0,
        total_evolved_skills: 0,
        last_session: None,
        score_history: vec![],
        best_score: None,
        best_session: String::new(),
        trend: "stable".into(),
        stagnation_count: 0,
        skill_attribution: HashMap::new(),
        last_error_context: None,
    }
}
