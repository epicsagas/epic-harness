use serde::{Deserialize, Serialize};

// ── Hook Protocol Types ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    /// Legacy structured output (kept for forward compat)
    pub tool_output: Option<ToolOutput>,
    /// Claude Code actual PostToolUse payload field (string or object)
    pub tool_result: Option<serde_json::Value>,
    /// Claude Code canonical PostToolUse field name
    pub tool_response: Option<serde_json::Value>,
    pub conversation_summary: Option<String>,
    pub pending_tasks: Option<Vec<String>>,
    pub context_usage: Option<f64>,
    /// Codex/Antigravity hook event name (e.g. "SessionStart", "PreToolUse").
    /// When present, stdout follows the Codex hook protocol instead of Claude
    /// Code's stdin-passthrough contract.
    pub hook_event_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    pub output: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub timestamp: String,
    #[serde(rename = "type")]
    pub snap_type: String,
    pub summary: String,
    pub pending_tasks: Vec<String>,
    pub context_usage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_state: Option<serde_json::Value>,
}

// ── Hook Profile ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HookProfile {
    Minimal,
    #[default]
    Standard,
    Strict,
}

impl HookProfile {
    /// Numeric level for comparison: Minimal=0, Standard=1, Strict=2.
    pub fn level(&self) -> u8 {
        match self {
            HookProfile::Minimal => 0,
            HookProfile::Standard => 1,
            HookProfile::Strict => 2,
        }
    }
}

/// Resolve the current hook profile.
/// Priority: EPIC_HOOK_PROFILE env var > config.toml [hook].profile > default (Standard).
pub fn current_hook_profile() -> HookProfile {
    // 1. Env var override (highest priority)
    if let Ok(val) = std::env::var("EPIC_HOOK_PROFILE") {
        return match val.to_lowercase().as_str() {
            "minimal" => HookProfile::Minimal,
            "strict" => HookProfile::Strict,
            _ => HookProfile::Standard,
        };
    }
    // 2. Config file
    match crate::config::CONFIG.hook.profile.to_lowercase().as_str() {
        "minimal" => HookProfile::Minimal,
        "strict" => HookProfile::Strict,
        _ => HookProfile::Standard,
    }
}

/// Returns true if a hook with the given minimum profile should execute
/// under the current profile setting.
pub fn should_run(min_profile: HookProfile) -> bool {
    current_hook_profile().level() >= min_profile.level()
}

// Profile assignments for each hook module:
pub const PROFILE_GUARD: HookProfile = HookProfile::Minimal;
pub const PROFILE_OBSERVE: HookProfile = HookProfile::Minimal;
pub const PROFILE_POLISH: HookProfile = HookProfile::Standard;
pub const PROFILE_REFLECT: HookProfile = HookProfile::Standard;
pub const PROFILE_SNAPSHOT: HookProfile = HookProfile::Standard;
pub const PROFILE_RESUME: HookProfile = HookProfile::Minimal;
