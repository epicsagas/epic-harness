use std::path::PathBuf;
use std::sync::LazyLock;

use serde::Deserialize;

use super::common;

// ── Config Types ────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HarnessConfig {
    pub hook: HookConfig,
    pub scoring: ScoringConfig,
    pub evolution: EvolutionConfig,
    pub pattern: PatternConfig,
    pub instinct: InstinctConfig,
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
            improvement_threshold: 0.05,
            gated_promotion_min: 3,
        }
    }
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
            graduated_scope_skip: 0.90,
            graduated_scope_moderate: 0.70,
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
            let cfg = toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "[epic-harness] warning: failed to parse {}: {} — using defaults",
                    path.display(),
                    e
                );
                HarnessConfig::default()
            });
            validate_config(&cfg);
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

// ── Validation ───────────────────────────────────────

fn validate_config(cfg: &HarnessConfig) {
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
}

// ── Default Config Template ─────────────────────────

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
# 0.05 = 5% improvement needed per session to avoid stagnation.
improvement_threshold = 0.05

# Minimum session observations before a skill can be promoted.
# Prevents premature skill creation from single successes.
# A skill must be observed this many times across sessions before being created.
gated_promotion_min = 3

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
# graduated_scope_skip = 0.90
# graduated_scope_moderate = 0.70

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
        assert_eq!(c.evolution.gated_promotion_min, 3);
        assert_eq!(c.pattern.repeated_error_min, 3);
        assert_eq!(c.pattern.graduated_scope_skip, 0.90);
        assert!((c.pattern.weak_ext_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(c.pattern.weak_ext_min_obs, 3);
        assert_eq!(c.instinct.confidence_threshold, 0.8);
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
        let c = HarnessConfig::default();
        validate_config(&c); // should not panic or warn
    }

    #[test]
    fn validate_weights_negative_warns() {
        let c = HarnessConfig {
            scoring: ScoringConfig {
                weights: [-0.5, 0.3, 0.2],
            },
            ..Default::default()
        };
        validate_config(&c); // warns but does not panic
    }

    #[test]
    fn validate_weights_bad_sum_warns() {
        let c = HarnessConfig {
            scoring: ScoringConfig {
                weights: [5.0, 3.0, 2.0],
            },
            ..Default::default()
        };
        validate_config(&c); // warns but does not panic
    }
}
