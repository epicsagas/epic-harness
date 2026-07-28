use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::scoring::ScoreDimensions;

// ── HarnessX-inspired: typed edit operations ─────────

/// The category of edit that the evolution engine applied.
/// Inspired by HarnessX's "typed builder operations" — each harness adaptation
/// is a typed operation rather than an opaque "write SKILL.md" action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditType {
    /// Create a new evolved skill (SKILL.md).
    #[default]
    AddSkill,
    /// Modify an existing skill's content (prompt tuning).
    ModifySkill,
    /// Promote a high-confidence pattern to global memory (instinct).
    AddInstinct,
    /// Change a config.toml threshold (future).
    ModifyConfig,
    /// Add a guard rule from observed failure patterns (future).
    AddGuardRule,
    /// Modify an existing skill's prompt (auto-tuning).
    ModifyPrompt,
    /// An unrecognized/legacy DB value. Not a real edit; used so unknown rows
    /// don't get silently distorted into `AddSkill` and pollute edit-type
    /// coverage. Excluded from `all()` (never an "untried" edit).
    Unknown,
}

impl EditType {
    /// All edit types — used for coverage analysis.
    pub fn all() -> &'static [EditType] {
        &[
            EditType::AddSkill,
            EditType::ModifySkill,
            EditType::AddInstinct,
            EditType::ModifyConfig,
            EditType::AddGuardRule,
            EditType::ModifyPrompt,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EditType::AddSkill => "add_skill",
            EditType::ModifySkill => "modify_skill",
            EditType::AddInstinct => "add_instinct",
            EditType::ModifyConfig => "modify_config",
            EditType::AddGuardRule => "add_guard_rule",
            EditType::ModifyPrompt => "modify_prompt",
            EditType::Unknown => "unknown",
        }
    }

    /// Parse an edit type from its database string representation.
    ///
    /// Unknown/legacy values decode to `Unknown` rather than being silently
    /// distorted into `AddSkill` (which would mis-count edit-type coverage).
    /// `Unknown` is excluded from `all()` so it never appears as an "untried"
    /// edit in the planner.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "add_skill" => EditType::AddSkill,
            "modify_skill" => EditType::ModifySkill,
            "add_instinct" => EditType::AddInstinct,
            "modify_config" => EditType::ModifyConfig,
            "add_guard_rule" => EditType::AddGuardRule,
            "modify_prompt" => EditType::ModifyPrompt,
            _ => EditType::Unknown,
        }
    }
}

// ── HarnessX-inspired: nine-dimensional taxonomy ──────

/// The nine orthogonal dimensions of the harness behavioral space.
/// Inspired by HarnessX's taxonomy — used for dimension-scoped analysis.
///
/// Reserved for P3 (dimension tags in skill frontmatter); not yet read by any
/// gate, hence `allow(dead_code)`.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HarnessDimension {
    ModelSelection,
    ContextAssembly,
    MemoryManagement,
    ToolEcosystem,
    ExecutionEnvironment,
    EvaluationAndReward,
    ControlAndSafety,
    Observability,
    TrainingBridge,
}

#[allow(dead_code)]
impl HarnessDimension {
    pub fn as_str(&self) -> &'static str {
        match self {
            HarnessDimension::ModelSelection => "model_selection",
            HarnessDimension::ContextAssembly => "context_assembly",
            HarnessDimension::MemoryManagement => "memory_management",
            HarnessDimension::ToolEcosystem => "tool_ecosystem",
            HarnessDimension::ExecutionEnvironment => "execution_environment",
            HarnessDimension::EvaluationAndReward => "evaluation_and_reward",
            HarnessDimension::ControlAndSafety => "control_and_safety",
            HarnessDimension::Observability => "observability",
            HarnessDimension::TrainingBridge => "training_bridge",
        }
    }

    /// Parse a dimension from its snake_case string. (Named `parse_dimension`
    /// rather than `from_str` to avoid shadowing the std `FromStr` trait.)
    pub fn parse_dimension(s: &str) -> Option<Self> {
        match s {
            "model_selection" => Some(Self::ModelSelection),
            "context_assembly" => Some(Self::ContextAssembly),
            "memory_management" => Some(Self::MemoryManagement),
            "tool_ecosystem" => Some(Self::ToolEcosystem),
            "execution_environment" => Some(Self::ExecutionEnvironment),
            "evaluation_and_reward" => Some(Self::EvaluationAndReward),
            "control_and_safety" => Some(Self::ControlAndSafety),
            "observability" => Some(Self::Observability),
            "training_bridge" => Some(Self::TrainingBridge),
            _ => None,
        }
    }
}

// ── SkillOpt-inspired types ───────────────────────────

/// A single entry in the negative feedback buffer — records why a skill proposal was rejected
/// so the curator can avoid proposing the same skill again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedEntry {
    pub name: String,
    pub reason: String,
    pub timestamp: String,
    pub confidence: f64,
    pub origin: String,
}

/// Insight extracted from a minibatch of observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinibatchInsight {
    pub batch_index: usize,
    pub success_rate: f64,
    pub dominant_error_category: Option<String>,
    pub dominant_tool: String,
    pub file_cluster: Vec<String>,
    /// Human-readable pattern description extracted from this batch.
    pub pattern: String,
    /// True when the same error appears in ≥ 60% of errors AND ≥ 2 distinct files.
    pub reusable: bool,
}

/// Epoch classification for slow/meta update (SkillOpt §5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EpochClass {
    Improving,
    Regressing,
    PersistentFailure,
    StableSuccess,
    /// Not enough score history to determine a meaningful epoch class (< 2 entries).
    InsufficientData,
}

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
    /// HarnessX-inspired: logical components implicated in this pattern.
    /// Derived from file paths (e.g., "src/auth/login.ts" → "auth").
    /// Empty when no component mapping is available.
    #[serde(default)]
    pub implicated_components: Vec<String>,
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
    pub minibatch_insights: Vec<MinibatchInsight>,
    pub dimension_averages: ScoreDimensions,
    /// HarnessX-inspired: true when a failure_category from this session
    /// was also present in the previous N sessions. Indicates a systemic issue.
    #[serde(default)]
    pub persistent_failure: bool,
    /// HarnessX-inspired: list of failure categories that are persistent
    /// (seen across multiple sessions). Used by the adaptation planner.
    #[serde(default)]
    pub persistent_failure_categories: Vec<String>,
    /// Representative error snippets, one per failure category (highest-count
    /// categories first). Secret-masked and truncated. Evidence for LLM skill
    /// synthesis — templates never see raw errors, synthesized skills do.
    #[serde(default)]
    pub error_snippets: Vec<String>,
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
    /// Sessions where this skill was deliberately withheld (holdout rotation).
    /// `avg_score_without` is a running average over these sessions only —
    /// a genuine counterfactual, unlike the legacy derived-from-total value.
    /// 0 means no holdout sample has been collected yet.
    #[serde(default)]
    pub sessions_holdout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// Most recent epoch classification (SkillOpt slow/meta update).
    #[serde(default)]
    pub epoch_class: Option<EpochClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_context: Option<String>,
    /// HarnessX-inspired: reward hacking detection.
    /// True when execution_cost is improving while output_quality is declining,
    /// suggesting the evolution is gaming the metric rather than improving outcomes.
    #[serde(default)]
    pub reward_hacking_suspected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionRecord {
    /// Stable SessionEnd identity used to make JSONL fallback writes replay-safe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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
    /// HarnessX-inspired: the type of edit applied during this evolution cycle.
    #[serde(default)]
    pub edit_type: EditType,
    /// HarnessX falsifiability contract (Table 9): the manifest of each edit
    /// shipped this round. Persisted to the JSONL fallback; the SQLite store
    /// writes only the scalar columns (manifests ride along via the JSONL
    /// path).
    ///
    /// STATUS: these are WRITTEN (reflect persists them + the sidecar
    /// manifests.jsonl). A Critic that READS them to verify prior predictions
    /// held is a deferred follow-up — not yet wired.
    #[serde(default)]
    pub manifests: Vec<crate::evolve::edits::EditManifest>,
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
        epoch_class: None,
        last_error_context: None,
        reward_hacking_suspected: false,
    }
}

// ── HarnessX-inspired: Task Digest (Digester) ─────────

/// Compressed summary of a task segment from execution traces.
/// Inspired by HarnessX's Digester stage — compresses voluminous raw
/// execution traces into structured per-task summaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDigest {
    /// Task identifier — orbit pipeline ID or session segment hash.
    pub task_id: String,
    /// Binary outcome of this task segment.
    pub outcome: TaskOutcome,
    /// Failure categories ranked by frequency.
    pub failure_categories: Vec<(String, u32)>,
    /// Logical components implicated in failures.
    pub implicated_components: Vec<String>,
    /// Curated error excerpts (max 3).
    pub evidence_excerpts: Vec<String>,
    /// Ordered sequence of tool categories used.
    pub tool_trajectory: Vec<String>,
    /// Number of previous iterations this task was seen (cross-iteration persistence).
    pub iterations_seen: u32,
    /// Estimated token count of the original trace segment.
    ///
    /// Scaffold (#81 item 3): populated by `digester::estimate_tokens` but not
    /// yet read by any gate/proposal. Retained for future planner
    /// cost-weighting.
    pub token_estimate: usize,
    /// Number of observations in this segment.
    pub observation_count: u64,
}

/// Outcome classification for a task segment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum TaskOutcome {
    Success,
    PartialFailure { failed_steps: u32, total_steps: u32 },
    CompleteFailure,
}

// ── HarnessX-inspired: Adaptation Landscape (Planner) ─

/// Strategic overview of the adaptation space.
/// Inspired by HarnessX's Planner stage — tracks what has been tried,
/// what persists, and what remains unexplored.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdaptationLandscape {
    /// Failures that have persisted across multiple sessions.
    pub persistent_failures: Vec<PersistentFailure>,
    /// History of attempted edits (what was tried).
    pub attempted_edits: Vec<AttemptedEdit>,
    /// Count of each edit type used so far.
    pub edit_type_coverage: HashMap<String, u32>,
    /// Edit types that have never been attempted.
    pub untried_edit_types: Vec<String>,
    /// Component name → failure rate (0.0–1.0).
    pub component_failure_heatmap: HashMap<String, f64>,
}

/// A failure that has persisted across multiple evolution cycles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentFailure {
    pub failure_category: String,
    pub first_seen: String,
    pub sessions_seen: u32,
    /// Skill names that attempted to address this failure.
    pub attempted_fixes: Vec<String>,
    pub resolved: bool,
}

/// Record of an edit that was attempted by the evolution engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptedEdit {
    pub edit_type: String,
    pub target: String,
    pub timestamp: String,
    pub success: bool,
}

// ── HarnessX-inspired: Seesaw Constraint ──────────────

/// Registry of solved tasks and their best outcome score.
///
/// Implements the HarnessX **seesaw constraint** (paper §4.1): the candidate
/// harness must not regress any previously solved task.
///
/// ## Critic-driven design note
/// The earlier draft tracked scores `per (task_id, dimension)`. The paper's
/// own analysis (§6.6, §7.6) shows that aggregate/per-dimension gating *fails*
/// to catch sub-threshold coupling — the exact regression mode seesaw is meant
/// to prevent. The paper's seesaw is **per-task** (pass@2 binary flips). We
/// therefore track a single best outcome score per task and reject any edit
/// that drops a previously-solved task below its best minus tolerance.
///
/// This is a deliberately coarse gate (the paper acknowledges even per-task
/// seesaw is insufficient for sub-threshold drift; variant isolation in R6 is
/// the real fix). Its scope here is to catch *gross* regressions cheaply, not
/// to guarantee no forgetting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SolvedTaskRegistry {
    /// task_id → best outcome score achieved (0.0–1.0).
    pub solved: HashMap<String, f64>,
    /// Count of tasks currently marked as solved.
    pub total_solved: u32,
}

impl SolvedTaskRegistry {
    /// Check whether the new per-task scores would regress any previously
    /// solved task. Returns the list of regressed task_ids.
    /// Empty = no regression (edit passes the seesaw constraint).
    pub fn check_seesaw(&self, new_scores: &HashMap<String, f64>, tolerance: f64) -> Vec<String> {
        self.solved
            .iter()
            .filter_map(|(task_id, best)| {
                let new = new_scores.get(task_id)?;
                if *new < *best - tolerance {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Update the registry with new per-task scores. Only improves best scores.
    pub fn update(&mut self, scores: &HashMap<String, f64>) {
        for (task_id, score) in scores {
            let entry = self.solved.entry(task_id.clone()).or_insert(*score);
            if *score > *entry {
                *entry = *score;
            }
        }
        self.total_solved = self.solved.len() as u32;
    }
}

// ── HarnessX-inspired: Harness Snapshot (first-class) ─
// The snapshot types below are reserved for P2 (harness as first-class object:
// `epic harness snapshot/diff/restore`). Not yet constructed, hence allow(dead_code).

/// A serializable snapshot of the entire harness state.
/// Inspired by HarnessX's "first-class object" — the harness can be
/// serialized, compared, and restored as a unit.
///
/// Constructed by [`crate::evolve::snapshot::build_snapshot`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessSnapshot {
    pub version: String,
    pub project_slug: String,
    pub timestamp: String,
    pub config_summary: ConfigSummary,
    pub active_skills: Vec<String>,
    pub evolved_skills: Vec<String>,
    pub guard_rules: Vec<String>,
    pub metrics_summary: MetricsSummary,
    /// Content hash for comparison.
    pub hash: String,
}

/// Subset of config relevant for comparison.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigSummary {
    pub hook_profile: String,
    pub scoring_weights: [f64; 3],
    pub max_skills: usize,
    pub stagnation_limit: u64,
}

/// Compact metrics summary for snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsSummary {
    pub total_sessions: u64,
    pub best_score: Option<f64>,
    pub trend: String,
    pub total_evolved: u64,
    pub stagnation_count: u64,
}

// ── HarnessX-inspired: Skill Variants ─────────────────

/// A domain-scoped variant of evolved skills.
/// Inspired by HarnessX's variant isolation — prevents catastrophic
/// forgetting on heterogeneous task sets by scoping skills per domain.
///
/// Constructed by `evolve::variants::VariantPool`; the struct itself is read
/// there but `allow(dead_code)` is needed because the bin targets don't form
/// a pool yet (wiring lands when variant isolation gates the evolve loop).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVariant {
    /// Variant identifier (e.g., "rust-backend", "python-ml").
    pub id: String,
    /// Detected stack/domain tags.
    pub domain_tags: Vec<String>,
    /// Skill names belonging to this variant.
    pub skills: Vec<String>,
    /// Average score for tasks routed to this variant.
    pub avg_score: f64,
    /// Patterns that route tasks to this variant.
    pub task_routing: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::EditType;

    #[test]
    fn from_db_str_unknown_is_not_distorted_to_add_skill() {
        // AC1 (#81 item 1): unknown DB values decode to Unknown, not AddSkill.
        assert_eq!(EditType::from_db_str("bogus_type"), EditType::Unknown);
        assert_eq!(EditType::from_db_str(""), EditType::Unknown);
        // Known values still decode — incl. add_skill, which the old wildcard
        // arm caught (must stay explicit after the fallback changed).
        assert_eq!(EditType::from_db_str("add_skill"), EditType::AddSkill);
        assert_eq!(
            EditType::from_db_str("modify_prompt"),
            EditType::ModifyPrompt
        );
    }

    #[test]
    fn unknown_excluded_from_all() {
        // Unknown must never show up as an "untried" edit in coverage analysis.
        assert!(!EditType::all().contains(&EditType::Unknown));
    }
}
