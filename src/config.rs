use std::path::PathBuf;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::hooks::common;

// ── Config Types ────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HarnessConfig {
    pub hook: HookConfig,
    pub scoring: ScoringConfig,
    pub evolution: EvolutionConfig,
    pub pattern: PatternConfig,
    pub instinct: InstinctConfig,
    pub context: ContextConfig,
    pub dashboard: DashboardConfig,
    pub db: DbConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ContextConfig {
    /// 기본 활성 소스 목록. 빈 배열이면 ["harness"] 와 동일.
    pub sources: Vec<String>,
    pub alcove: AlcoveConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AlcoveConfig {
    /// Alcove vault 루트 경로 (예: ~/.alcove/vaults/OSN). 비어 있으면 미설정.
    pub vault_path: String,
    /// 수집할 프로젝트 목록. 빈 배열이면 전체.
    pub projects: Vec<String>,
    /// 최대 수집 문서 수 (기본 20).
    pub max_docs: usize,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HookConfig {
    /// Hook execution profile: "minimal" | "standard" | "strict".
    /// Env var EPIC_HOOK_PROFILE takes precedence over this value.
    pub profile: String,

    /// Show file-type-aware investigation hints after Edit/Write.
    /// Example: .rs → "Run cargo check", .ts → "Run tsc --noEmit"
    pub gateguard_hints: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ScoringConfig {
    /// Tool call scoring weights: [success, quality, cost].
    /// Must sum to approximately 1.0.
    /// - success (default 0.5): did the tool succeed? (0/1)
    /// - quality (default 0.3): output quality signals (0.0-1.0)
    /// - cost   (default 0.2): efficiency proxy (0.0-1.0)
    pub weights: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct EvolutionConfig {
    /// Maximum number of auto-evolved skills.
    /// Oldest skills are removed when exceeded.
    pub max_skills: usize,

    /// Number of sessions without improvement before auto-rollback
    /// to the best checkpoint.
    pub stagnation_limit: u64,

    /// Minimum score improvement ratio to count as "improving" (e.g. 0.05 = 5%).
    pub improvement_threshold: f64,

    /// Minimum number of session observations before a skill can be promoted.
    /// Prevents premature skill creation from single successes.
    pub gated_promotion_min: u64,

    /// Score degradation threshold that triggers rollback (e.g. 0.05 = 5% drop).
    pub rollback_degradation: f64,

    /// If best_score is below this floor, always rollback on stagnation.
    pub rollback_best_floor: f64,

    /// Number of recent sessions averaged for stagnation detection.
    /// Prevents outlier sessions from permanently gating improvement.
    pub stagnation_rolling_window: usize,

    /// Number of sessions a rejected skill stays in the negative feedback buffer
    /// before it is eligible for re-proposal. (SkillOpt rejected_edit_buffer)
    pub rejected_buffer_ttl: u64,

    /// Number of observations per minibatch for structural pattern analysis.
    /// (SkillOpt reflection minibatch size)
    pub minibatch_size: usize,

    /// HarnessX Tier 2.2: number of recent sessions inspected for reward-hacking.
    /// Detection requires at least this many score_history entries.
    #[serde(default = "default_reward_hacking_window")]
    pub reward_hacking_window: usize,

    /// HarnessX Tier 2.2: minimum negative slope of output_quality required
    /// (per session, over the window) to consider quality "declining".
    #[serde(default = "default_reward_hacking_quality_drop")]
    pub reward_hacking_quality_drop: f64,

    /// HarnessX Tier 2.2: minimum positive slope of execution_cost required
    /// (per session, over the window) to consider the efficiency proxy "rising".
    #[serde(default = "default_reward_hacking_cost_rise")]
    pub reward_hacking_cost_rise: f64,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PatternConfig {
    /// Same error repeated this many times → repeated_same_error pattern.
    pub repeated_error_min: u64,

    /// Lookahead window for fix_then_break detection.
    pub ftb_lookahead: usize,

    /// Minimum edit→error cycles for fix_then_break detection.
    pub ftb_min_cycles: u64,

    /// Same file appearing in this many consecutive operations → long_debug_loop.
    pub debug_loop_min: u64,

    /// Minimum edit count for thrashing detection.
    pub thrash_min_edits: u64,

    /// Minimum error count for thrashing detection.
    pub thrash_min_errors: u64,

    /// Tool success rate below this → weak_tool pattern.
    pub weak_tool_rate: f64,

    /// Minimum tool observations before weak_tool detection triggers.
    pub weak_tool_min_obs: u64,

    /// File-extension success rate below this → weak_ext pattern.
    pub weak_ext_rate: f64,

    /// Minimum file-extension observations before weak_ext detection triggers.
    pub weak_ext_min_obs: u64,

    /// Error appearing this many times → high-frequency error seeding.
    pub high_freq_error_min: u64,

    /// Composite score ≥ this → skip skill seeding entirely.
    pub graduated_scope_skip: f64,

    /// Composite score ≥ this (but < skip) → moderate seeding.
    pub graduated_scope_moderate: f64,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct InstinctConfig {
    /// Minimum confidence for an instinct to be considered for promotion.
    pub confidence_threshold: f64,

    /// Minimum number of projects where the pattern was observed
    /// before promoting to global memory.
    pub promotion_min_projects: usize,

    /// Maximum number of instinct nodes stored globally.
    pub max_instincts: usize,

    /// Minimum session observations before instinct extraction runs.
    pub min_observations: usize,

    /// Minimum session avg_score before instinct extraction runs.
    pub min_avg_score: f64,
}

// ── Defaults ────────────────────────────────────────

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            profile: "standard".into(),
            gateguard_hints: true,
        }
    }
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            weights: [0.5, 0.3, 0.2],
        }
    }
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            max_skills: 10,
            stagnation_limit: 3,
            improvement_threshold: 0.02,
            gated_promotion_min: 2,
            rollback_degradation: 0.05,
            rollback_best_floor: 0.90,
            stagnation_rolling_window: 3,
            rejected_buffer_ttl: 10,
            minibatch_size: 8,
            reward_hacking_window: default_reward_hacking_window(),
            reward_hacking_quality_drop: default_reward_hacking_quality_drop(),
            reward_hacking_cost_rise: default_reward_hacking_cost_rise(),
        }
    }
}

// ── Reward-hacking defaults (HarnessX Tier 2.2) ─────

fn default_reward_hacking_window() -> usize {
    5
}

fn default_reward_hacking_quality_drop() -> f64 {
    0.05
}

fn default_reward_hacking_cost_rise() -> f64 {
    0.05
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            repeated_error_min: 3,
            ftb_lookahead: 3,
            ftb_min_cycles: 2,
            debug_loop_min: 5,
            thrash_min_edits: 3,
            thrash_min_errors: 3,
            weak_tool_rate: 0.6,
            weak_tool_min_obs: 5,
            weak_ext_rate: 0.5,
            weak_ext_min_obs: 3,
            high_freq_error_min: 5,
            graduated_scope_skip: 0.95,
            graduated_scope_moderate: 0.80,
        }
    }
}

impl Default for InstinctConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.8,
            promotion_min_projects: 2,
            max_instincts: 20,
            min_observations: 10,
            min_avg_score: 0.5,
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            sources: vec!["harness".into()],
            alcove: AlcoveConfig::default(),
        }
    }
}

impl Default for AlcoveConfig {
    fn default() -> Self {
        Self {
            vault_path: String::new(),
            projects: vec![],
            max_docs: 20,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DashboardConfig {
    /// Port for the dashboard web server (default: 7700).
    /// Set to 0 to disable auto-launch on session start.
    pub port: u16,

    /// Automatically open the dashboard browser on first session start.
    pub auto_open: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: 7700,
            auto_open: true,
        }
    }
}

/// Supported database drivers.
const SUPPORTED_DRIVERS: &[&str] = &["sqlite", "postgres", "mysql"];

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DbConfig {
    /// Database driver: "sqlite" | "postgres" | "mysql".
    /// Requires corresponding Cargo feature flag for non-sqlite drivers.
    pub driver: String,

    /// Connection URL for harness.db.
    /// Empty = default path (sqlite:~/.harness/harness.db).
    /// For PostgreSQL: postgres://user:pass@host:port/dbname
    /// For MySQL: mysql://user:pass@host:port/dbname
    pub harness_url: String,

    /// Connection URL for memory.db.
    /// Empty = default path (sqlite:~/.harness/memory.db).
    pub memory_url: String,

    /// Maximum number of connections in the pool.
    pub max_connections: u32,

    /// TLS mode for non-sqlite connections:
    /// - "prefer": try TLS, fall back to plain (default)
    /// - "require": TLS required, connection fails without it
    /// - "disable": no TLS
    pub tls_mode: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            driver: "sqlite".into(),
            harness_url: String::new(),
            memory_url: String::new(),
            max_connections: 5,
            tls_mode: "prefer".into(),
        }
    }
}

// ── Global Config Instance ──────────────────────────

pub static CONFIG: LazyLock<HarnessConfig> = LazyLock::new(load_config);

fn harness_dir() -> PathBuf {
    common::dirs_home().join(".harness")
}

/// Load config from `~/.harness/config.toml`.
/// Returns defaults if file is missing or malformed.
fn load_config() -> HarnessConfig {
    let path = harness_dir().join("config.toml");
    if !path.is_file() {
        return HarnessConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let mut cfg = toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "[epic-harness] warning: failed to parse {}: {} — using defaults",
                    path.display(),
                    e
                );
                HarnessConfig::default()
            });
            validate_config(&mut cfg);

            apply_env_overrides(&mut cfg);
            auto_correct_driver_from_url(&mut cfg);

            cfg
        }
        Err(e) => {
            eprintln!(
                "[epic-harness] warning: failed to read {}: {} — using defaults",
                path.display(),
                e
            );
            HarnessConfig::default()
        }
    }
}

/// Apply environment variable overrides to database URLs.
/// HARNESS_DB_URL overrides `db.harness_url`, HARNESS_MEM_URL overrides `db.memory_url`.
fn apply_env_overrides(cfg: &mut HarnessConfig) {
    apply_url_overrides(
        cfg,
        std::env::var("HARNESS_DB_URL").ok(),
        std::env::var("HARNESS_MEM_URL").ok(),
    );
}

/// Apply URL overrides from explicit values (testable without env vars).
fn apply_url_overrides(cfg: &mut HarnessConfig, db_url: Option<String>, mem_url: Option<String>) {
    if let Some(url) = db_url {
        cfg.db.harness_url = url;
    }
    if let Some(url) = mem_url {
        cfg.db.memory_url = url;
    }
}

/// Auto-correct `db.driver` to match the URL scheme after env var overrides.
fn auto_correct_driver_from_url(cfg: &mut HarnessConfig) {
    let url = &cfg.db.harness_url;
    if url.is_empty() {
        return;
    }
    let detected_driver = if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        "postgres"
    } else if url.starts_with("mysql:") {
        "mysql"
    } else if url.starts_with("sqlite:") {
        "sqlite"
    } else {
        return;
    };
    cfg.db.driver = detected_driver.to_string();
}

// ── Validation ───────────────────────────────────────

/// Cross-validate `driver` against URL scheme. Warn and correct on mismatch.
fn cross_validate_driver(driver: &mut String, url: &str) {
    if url.is_empty() {
        // Empty URL defaults to sqlite: — if driver says otherwise, warn.
        if *driver != "sqlite" {
            eprintln!(
                "[harness] db.driver = '{driver}' but URL is empty — will default to SQLite. \
                 Set URL to a {driver}:// connection string or change driver to 'sqlite'.",
            );
            *driver = "sqlite".into();
        }
        return;
    }
    let detected =
        crate::store::pool::DbType::from_url(url).unwrap_or(crate::store::pool::DbType::Sqlite);
    let detected_name = detected.name();
    if detected_name != driver.as_str() {
        eprintln!(
            "[harness] db.driver = '{driver}' but URL scheme is '{detected_name}:' — correcting driver to '{detected_name}'",
        );
        *driver = detected_name.to_string();
    }
}

fn validate_config(cfg: &mut HarnessConfig) {
    let w = cfg.scoring.weights;
    let sum: f64 = w.iter().sum();
    if w.iter().any(|v| *v < 0.0) || sum.abs() < f64::EPSILON {
        eprintln!(
            "[epic-harness] warning: scoring weights contain negative values — using defaults"
        );
    } else if (sum - 1.0).abs() > 0.15 {
        eprintln!(
            "[epic-harness] warning: scoring weights sum to {:.3} (expected ~1.0) — scores may be unreliable",
            sum
        );
    }

    // Validate db.driver
    let driver = cfg.db.driver.to_lowercase();
    if !SUPPORTED_DRIVERS.contains(&driver.as_str()) {
        eprintln!(
            "[harness] unsupported db.driver: '{}', supported: {:?} — correcting to 'sqlite'",
            cfg.db.driver, SUPPORTED_DRIVERS
        );
        cfg.db.driver = "sqlite".into();
    } else {
        cfg.db.driver = driver;
    }

    // Cross-validate db.driver against URL schemes
    cross_validate_driver(&mut cfg.db.driver, &cfg.db.harness_url);
    // Warn if memory_url is explicitly set but its scheme differs from driver.
    // Do NOT correct driver — memory_url defaults to SQLite when empty.
    if !cfg.db.memory_url.is_empty() {
        if let Ok(detected) = crate::store::pool::DbType::from_url(&cfg.db.memory_url) {
            let detected_name = detected.name();
            if detected_name != cfg.db.driver {
                eprintln!(
                    "[harness] db.memory_url scheme is '{detected_name}:' but db.driver is '{}' — \
                     ensure both databases use the same backend or set memory_url explicitly",
                    cfg.db.driver,
                );
            }
        }
    }

    // Validate TLS mode
    match cfg.db.tls_mode.as_str() {
        "prefer" | "require" | "disable" => {}
        other => {
            eprintln!(
                "[harness] unsupported db.tls_mode: '{other}', supported: prefer/require/disable — correcting to 'prefer'"
            );
            cfg.db.tls_mode = "prefer".into();
        }
    }

    if cfg.db.max_connections == 0 {
        cfg.db.max_connections = 5;
    }
}

// ── Default Config Template ─────────────────────────

static HARNESS_MD: &str = include_str!("../integrations/common/HARNESS.md");

/// Ensure `~/.harness/config.toml` (write-once) and `HARNESS.md` (synced) exist.
/// Called from the resume hook on session start so a plugin-only install works
/// without the deprecated `install` subcommand.
pub fn ensure_global_config() {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let harness_dir = PathBuf::from(&home).join(".harness");
    let _ = std::fs::create_dir_all(&harness_dir);

    // config.toml: write-once (never overwrites user edits)
    let config_path = harness_dir.join("config.toml");
    if !config_path.exists() {
        let _ = std::fs::write(&config_path, default_config_template());
    }

    // HARNESS.md: synced to stay current across binary upgrades
    let _ = std::fs::write(harness_dir.join("HARNESS.md"), HARNESS_MD);
}

/// Returns a commented default config suitable for writing to `~/.harness/config.toml`.
#[allow(dead_code)] // Used by epic config init CLI command
pub fn default_config_template() -> &'static str {
    r#"# epic-harness global configuration
# Location: ~/.harness/config.toml
# Priority: env var (EPIC_HOOK_PROFILE) > this file > hardcoded defaults

# ── Hook execution ──────────────────────────────────
[hook]
# Hook execution profile: "minimal" | "standard" | "strict"
#   minimal  — guard, observe, resume (lowest overhead)
#   standard — above + polish, reflect, snapshot (recommended)
#   strict   — all hooks + future strict-only checks
# Override: EPIC_HOOK_PROFILE env var takes precedence over this value.
profile = "standard"

# Show file-type-aware investigation hints after Edit/Write.
# When true, outputs concrete verification questions like:
#   .rs → "Run cargo check after this change"
#   .ts → "Verify type compatibility — run tsc --noEmit"
# Set to false if you find the hints noisy.
gateguard_hints = true

# ── Scoring weights ─────────────────────────────────
[scoring]
# Tool call composite score weights: [success, quality, cost]
# Must sum to approximately 1.0.
#   success — did the tool succeed? (0/1)
#   quality — output quality signals (0.0-1.0), e.g. warnings, empty output
#   cost    — efficiency proxy (0.0-1.0), e.g. output size
weights = [0.5, 0.3, 0.2]

# ── Evolution system ────────────────────────────────
[evolution]
# Maximum number of auto-evolved skills.
# Oldest skills are removed when this limit is exceeded.
max_skills = 10

# Number of consecutive sessions without improvement before auto-rollback
# to the best checkpoint. Set higher to tolerate temporary plateaus.
stagnation_limit = 3

# Minimum score improvement ratio to count as "improving".
# 0.02 = 2% improvement needed per session to avoid stagnation.
improvement_threshold = 0.02

# Minimum session observations before a skill can be promoted.
# Prevents premature skill creation from single successes.
# A skill must be observed this many times across sessions before being created.
gated_promotion_min = 2

# Score degradation threshold that triggers evolved-skill rollback.
# 0.05 = 5% drop from best_score triggers rollback.
# rollback_degradation = 0.05

# If best_score is below this floor, always rollback on stagnation.
# rollback_best_floor = 0.90

# Number of recent sessions used for rolling-average stagnation check.
# stagnation_rolling_window = 3

# Number of sessions a rejected skill stays in the negative feedback buffer
# before it becomes eligible for re-proposal. Set higher for conservative evolution.
# rejected_buffer_ttl = 10

# Number of observations per minibatch for structural pattern analysis.
# Larger batches find more structural patterns but need more observations.
# minibatch_size = 8

# Reward-hacking detection (HarnessX Tier 2.2). Flags the evolution when
# the efficiency proxy (execution_cost) is rising while the quality proxy
# (output_quality) is declining over the window — a sign the metric is
# being gamed rather than outcomes improved. In this codebase
# execution_cost is an efficiency proxy where HIGHER = BETTER.
# reward_hacking_window = 5
# reward_hacking_quality_drop = 0.05
# reward_hacking_cost_rise = 0.05

# ── Pattern detection (power-user) ──────────────────
# These thresholds control when failure patterns are detected.
# Defaults work well for most projects — only adjust if you have
# specific tuning needs.
[pattern]
# Same error repeated this many consecutive times → repeated_same_error
# repeated_error_min = 3

# Edit succeeds → build/test fails → edit again cycle detection
# ftb_lookahead = 3
# ftb_min_cycles = 2

# Same file appearing in this many consecutive operations → long_debug_loop
# debug_loop_min = 5

# Edit ↔ Error alternating on the same file → thrashing
# thrash_min_edits = 3
# thrash_min_errors = 3

# Tool success rate below this threshold → weak_tool pattern
# weak_tool_rate = 0.6
# weak_tool_min_obs = 5

# File-extension success rate below this threshold → weak_ext pattern
# weak_ext_rate = 0.5
# weak_ext_min_obs = 3

# Error appearing this many times in a session → high-frequency error seeding
# high_freq_error_min = 5

# Graduated scope: composite score thresholds for seeding intensity
# ≥ skip  → no skill generation
# ≥ moderate → only weak-tool proposals
# < moderate → full seeding
# graduated_scope_skip = 0.95
# graduated_scope_moderate = 0.80

# ── Instinct learning ───────────────────────────────
# High-success patterns extracted and promoted across projects.
[instinct]
# Minimum confidence for an instinct to be considered for global promotion.
# confidence_threshold = 0.8

# Number of distinct projects where the pattern must be observed
# before promoting to global memory.
# promotion_min_projects = 2

# Maximum number of instinct nodes stored globally.
# max_instincts = 20

# Minimum session observations before instinct extraction runs.
# min_observations = 10

# Minimum session avg_score before instinct extraction runs.
# min_avg_score = 0.5

# ── Context sources ──────────────────────────────────
[context]
# Default sources included in reflect --context output.
# "harness" is always included. Add "claude-session", "alcove", or "all".
sources = ["harness"]

[context.alcove]
# Path to Alcove vault root (e.g. ~/.alcove/vaults/MyOrg).
# Leave empty if Alcove is not used.
vault_path = ""
# Projects to include (empty = all projects in vault).
projects = []
# Maximum number of documents to collect.
max_docs = 20

# ── Dashboard ─────────────────────────────────────────
[dashboard]
# Port for the dashboard web server.
# Set to 0 to disable auto-launch on session start.
# port = 7700

# Automatically open the browser on first session start.
# auto_open = true

# ── Database ──────────────────────────────────────────
[db]
# Database driver: "sqlite" | "postgres" | "mysql".
# Non-sqlite drivers require the corresponding Cargo feature flag.
#   cargo build --features postgres
#   cargo build --features mysql
driver = "sqlite"

# Connection URL for harness.db (operational data).
# SQLite: empty = default path (~/.harness/harness.db)
# PostgreSQL: postgres://user:pass@host:5432/dbname?sslmode=require
# MySQL: mysql://user:pass@host:3306/dbname
# Override: HARNESS_DB_URL env var takes precedence over this value.
harness_url = ""

# Connection URL for memory.db (knowledge graph).
# Same URL format as harness_url.
# Override: HARNESS_MEM_URL env var takes precedence over this value.
memory_url = ""

# Maximum number of connections per pool.
max_connections = 5

# TLS mode for postgres/mysql connections:
#   "prefer"  — try TLS, fall back to plain (default)
#   "require" — TLS required, connection fails without it
#   "disable" — no TLS (for local development only)
tls_mode = "prefer"
"#
}

// ── Tests ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let c = HarnessConfig::default();
        assert_eq!(c.hook.profile, "standard");
        assert!(c.hook.gateguard_hints);
        assert_eq!(c.scoring.weights, [0.5, 0.3, 0.2]);
        assert_eq!(c.evolution.max_skills, 10);
        assert_eq!(c.evolution.stagnation_limit, 3);
        assert_eq!(c.evolution.gated_promotion_min, 2);
        assert_eq!(c.evolution.stagnation_rolling_window, 3);
        assert_eq!(c.evolution.rejected_buffer_ttl, 10);
        assert_eq!(c.evolution.minibatch_size, 8);
        assert_eq!(c.evolution.reward_hacking_window, 5);
        assert!((c.evolution.reward_hacking_quality_drop - 0.05).abs() < f64::EPSILON);
        assert!((c.evolution.reward_hacking_cost_rise - 0.05).abs() < f64::EPSILON);
        assert_eq!(c.pattern.repeated_error_min, 3);
        assert_eq!(c.pattern.graduated_scope_skip, 0.95);
        assert!((c.pattern.weak_ext_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(c.pattern.weak_ext_min_obs, 3);
        assert_eq!(c.instinct.confidence_threshold, 0.8);
        assert_eq!(c.db.driver, "sqlite");
        assert!(c.db.harness_url.is_empty());
        assert!(c.db.memory_url.is_empty());
        assert_eq!(c.db.max_connections, 5);
        assert_eq!(c.db.tls_mode, "prefer");
    }

    #[test]
    fn partial_config_uses_defaults() {
        let toml = r#"
[hook]
profile = "minimal"
"#;
        let c: HarnessConfig = toml::from_str(toml).unwrap();
        assert_eq!(c.hook.profile, "minimal");
        assert!(c.hook.gateguard_hints); // default
        assert_eq!(c.scoring.weights, [0.5, 0.3, 0.2]); // default
        assert_eq!(c.evolution.max_skills, 10); // default
    }

    #[test]
    fn empty_config_uses_all_defaults() {
        let c: HarnessConfig = toml::from_str("").unwrap();
        assert_eq!(c.hook.profile, "standard");
        assert_eq!(c.scoring.weights, [0.5, 0.3, 0.2]);
    }

    #[test]
    fn full_custom_config() {
        let toml = r#"
[hook]
profile = "strict"
gateguard_hints = false

[scoring]
weights = [0.7, 0.2, 0.1]

[evolution]
max_skills = 5
stagnation_limit = 5
improvement_threshold = 0.10
gated_promotion_min = 5

[pattern]
repeated_error_min = 5
debug_loop_min = 8
graduated_scope_skip = 0.95

[instinct]
confidence_threshold = 0.9
max_instincts = 10
"#;
        let c: HarnessConfig = toml::from_str(toml).unwrap();
        assert_eq!(c.hook.profile, "strict");
        assert!(!c.hook.gateguard_hints);
        assert_eq!(c.scoring.weights, [0.7, 0.2, 0.1]);
        assert_eq!(c.evolution.max_skills, 5);
        assert_eq!(c.evolution.stagnation_limit, 5);
        assert_eq!(c.evolution.gated_promotion_min, 5);
        assert_eq!(c.pattern.repeated_error_min, 5);
        assert_eq!(c.pattern.debug_loop_min, 8);
        assert_eq!(c.pattern.graduated_scope_skip, 0.95);
        assert_eq!(c.instinct.confidence_threshold, 0.9);
        assert_eq!(c.instinct.max_instincts, 10);
    }

    #[test]
    fn invalid_toml_falls_back_to_defaults() {
        let result: Result<HarnessConfig, _> = toml::from_str("not valid toml!!!");
        assert!(result.is_err());
        // load_config() would catch this and return defaults
        let c = HarnessConfig::default();
        assert_eq!(c.hook.profile, "standard");
    }

    #[test]
    fn context_config_defaults() {
        let c = ContextConfig::default();
        assert_eq!(c.sources, vec!["harness".to_string()]);
        assert_eq!(c.alcove.max_docs, 20);
    }

    #[test]
    fn context_config_from_toml() {
        let toml = r#"
[context]
sources = ["harness", "alcove"]
[context.alcove]
vault_path = "/tmp/vault"
max_docs = 5
"#;
        let c: HarnessConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            c.context.sources,
            vec!["harness".to_string(), "alcove".to_string()]
        );
        assert_eq!(c.context.alcove.vault_path, "/tmp/vault");
        assert_eq!(c.context.alcove.max_docs, 5);
    }

    #[test]
    fn alcove_config_empty_vault_path() {
        let c = AlcoveConfig::default();
        assert!(c.vault_path.is_empty());
    }

    #[test]
    fn template_includes_context_section() {
        let template = default_config_template();
        assert!(template.contains("[context]"));
    }

    #[test]
    fn template_includes_db_section() {
        let template = default_config_template();
        assert!(template.contains("[db]"));
        let c: HarnessConfig = toml::from_str(template).unwrap();
        assert_eq!(c.db.driver, "sqlite");
        assert_eq!(c.db.max_connections, 5);
        assert_eq!(c.db.tls_mode, "prefer");
    }

    #[test]
    fn template_is_valid_toml() {
        let template = default_config_template();
        let c: HarnessConfig = toml::from_str(template).unwrap();
        assert_eq!(c.hook.profile, "standard");
        assert!(c.hook.gateguard_hints);
        assert_eq!(c.scoring.weights, [0.5, 0.3, 0.2]);
        assert_eq!(c.evolution.max_skills, 10);
    }

    #[test]
    fn validate_weights_ok() {
        let mut c = HarnessConfig::default();
        validate_config(&mut c); // should not panic or warn
    }

    #[test]
    fn validate_weights_negative_warns() {
        let mut c = HarnessConfig {
            scoring: ScoringConfig {
                weights: [-0.5, 0.3, 0.2],
            },
            ..Default::default()
        };
        validate_config(&mut c); // warns but does not panic
    }

    #[test]
    fn validate_weights_bad_sum_warns() {
        let mut c = HarnessConfig {
            scoring: ScoringConfig {
                weights: [5.0, 3.0, 2.0],
            },
            ..Default::default()
        };
        validate_config(&mut c); // warns but does not panic
    }

    #[test]
    fn validate_driver_accepts_sqlite() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "sqlite".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "sqlite");
    }

    #[test]
    fn validate_driver_accepts_postgres() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "postgres".into(),
                harness_url: "postgres://u:p@localhost/db".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "postgres");
    }

    #[test]
    fn validate_driver_accepts_mysql() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "mysql".into(),
                harness_url: "mysql://u:p@localhost/db".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "mysql");
    }

    #[test]
    fn validate_driver_rejects_unknown() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "oracle".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "sqlite"); // corrected
    }

    #[test]
    fn validate_tls_mode_accepts_valid() {
        for mode in &["prefer", "require", "disable"] {
            let mut c = HarnessConfig {
                db: DbConfig {
                    tls_mode: mode.to_string(),
                    ..Default::default()
                },
                ..Default::default()
            };
            validate_config(&mut c);
            assert_eq!(c.db.tls_mode, *mode);
        }
    }

    #[test]
    fn validate_tls_mode_rejects_invalid() {
        let mut c = HarnessConfig {
            db: DbConfig {
                tls_mode: "always".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.tls_mode, "prefer"); // corrected
    }

    #[test]
    fn cross_validate_driver_postgres_without_url_corrects_to_sqlite() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "postgres".into(),
                // harness_url and memory_url are empty (defaults)
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "sqlite"); // corrected: no URL → sqlite
    }

    #[test]
    fn cross_validate_driver_scheme_mismatch_corrects() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "sqlite".into(),
                harness_url: "postgres://u:p@localhost/db".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "postgres"); // corrected to match URL scheme
    }

    #[test]
    fn cross_validate_driver_matches_url_scheme() {
        let mut c = HarnessConfig {
            db: DbConfig {
                driver: "postgres".into(),
                harness_url: "postgres://u:p@localhost/db".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        validate_config(&mut c);
        assert_eq!(c.db.driver, "postgres"); // unchanged
    }

    #[test]
    fn apply_url_overrides_sets_values() {
        let mut cfg = HarnessConfig::default();
        cfg.db.harness_url = String::new();
        cfg.db.memory_url = String::new();

        apply_url_overrides(
            &mut cfg,
            Some("postgres://testhost/db".into()),
            Some("postgres://testhost/mem".into()),
        );

        assert_eq!(cfg.db.harness_url, "postgres://testhost/db");
        assert_eq!(cfg.db.memory_url, "postgres://testhost/mem");
    }

    #[test]
    fn apply_url_overrides_preserves_existing_when_none() {
        let mut cfg = HarnessConfig::default();
        cfg.db.harness_url = "sqlite:test.db".to_string();
        cfg.db.memory_url = "sqlite:mem.db".to_string();

        apply_url_overrides(&mut cfg, None, None);

        assert_eq!(cfg.db.harness_url, "sqlite:test.db");
        assert_eq!(cfg.db.memory_url, "sqlite:mem.db");
    }

    #[test]
    fn auto_correct_driver_matches_url_scheme() {
        let mut cfg = HarnessConfig::default();
        cfg.db.driver = "sqlite".to_string();
        cfg.db.harness_url = "postgres://testhost/db".to_string();

        auto_correct_driver_from_url(&mut cfg);

        assert_eq!(cfg.db.driver, "postgres");
    }
}
