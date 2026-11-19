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
    /// Claude Code hook payload session_id. Absent on legacy hosts — callers
    /// fall back to a date+pid key.
    pub session_id: Option<String>,
    /// Claude Code SessionStart source: "startup" | "resume" | "clear" | "compact".
    pub source: Option<String>,
    /// Codex subagent identity, supplied on `SubagentStart` / `SubagentStop`.
    /// Preferred over the prompt-hash id derived for Claude's `Agent` tool,
    /// since the host guarantees it is stable across the subagent's lifetime.
    pub agent_id: Option<String>,
    /// Codex subagent kind, the analogue of Claude's `subagent_type`.
    pub agent_type: Option<String>,
    /// Claude Code / grok `PostToolUseFailure` diagnostic. Kept as a raw
    /// `Value`: a typed field that fails to deserialize makes the whole payload
    /// parse fall back to default, losing the tool name too.
    pub error: Option<serde_json::Value>,
    /// `PostToolUseFailure` flag for a user abort. Raw `Value` for the same
    /// reason as `error`.
    pub is_interrupt: Option<serde_json::Value>,
    /// grok's camelCase event name. Event detection only: it must never feed
    /// `hook_event_name`, whose presence switches the stdout protocol.
    #[serde(rename = "hookEventName")]
    pub hook_event_name_camel: Option<serde_json::Value>,
}

impl HookInput {
    /// Event name from `hook_event_name`, grok's `hookEventName`, or the
    /// `GROK_HOOK_EVENT` env var, in that order. Observe-only.
    pub fn event_name(&self) -> Option<String> {
        self.hook_event_name
            .clone()
            .or_else(|| {
                self.hook_event_name_camel
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .or_else(|| {
                std::env::var("GROK_HOOK_EVENT")
                    .ok()
                    .filter(|e| !e.is_empty())
            })
    }

    pub fn is_failure_event(&self) -> bool {
        self.event_name().as_deref() == Some("PostToolUseFailure")
    }

    /// The host's failure diagnostic when it is a non-blank string.
    pub fn failure_text(&self) -> Option<&str> {
        self.error
            .as_ref()
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|e| !e.is_empty())
    }

    /// True for a user-aborted call (`is_interrupt` is boolean true).
    pub fn interrupted(&self) -> bool {
        self.is_interrupt.as_ref().and_then(|v| v.as_bool()) == Some(true)
    }
}

/// Fill an absent snake_case key from its camelCase twin (grok sends
/// camelCase). Never overwrites, and leaves non-object payloads unchanged.
pub fn normalize_hook_payload(v: &mut serde_json::Value) {
    const PAIRS: [(&str, &str); 5] = [
        ("sessionId", "session_id"),
        ("toolName", "tool_name"),
        ("toolInput", "tool_input"),
        ("toolResponse", "tool_response"),
        ("isInterrupt", "is_interrupt"),
    ];
    let Some(obj) = v.as_object_mut() else {
        return;
    };
    for (camel, snake) in PAIRS {
        if !obj.contains_key(snake)
            && let Some(val) = obj.get(camel).cloned()
        {
            obj.insert(snake.into(), val);
        }
    }
}

/// What `adapt_agy_payload` learned from an agy (Antigravity CLI) payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgyPayload {
    /// Directory to chdir into before anything resolves the project slug.
    pub workspace: Option<std::path::PathBuf>,
}

/// Detect an agy hook payload and rewrite it in place for `HookInput`.
/// Returns `None` (and leaves `v` untouched) for every other host: agy is the
/// only one that sends a string `conversationId` and `hookEventName` without
/// a snake_case `hook_event_name` (grok sends both event keys).
///
/// `lastUserInput`, `transcriptPath` and `artifactDirectoryPath` are never
/// copied: user prompt text can hold personal data.
pub fn adapt_agy_payload(v: &mut serde_json::Value) -> Option<AgyPayload> {
    let obj = v.as_object_mut()?;
    let id = obj.get("conversationId")?.as_str()?.to_string();
    let event = obj.get("hookEventName")?.as_str()?.to_string();
    if obj.contains_key("hook_event_name") {
        return None;
    }
    obj.entry("session_id")
        .or_insert_with(|| serde_json::Value::String(id));
    obj.insert("hook_event_name".into(), serde_json::Value::String(event));

    let usable = |c: &serde_json::Value| {
        c.as_str()
            .map(std::path::Path::new)
            .filter(|p| p.is_absolute() && p.is_dir())
            .map(std::path::Path::to_path_buf)
    };
    let workspace = obj
        .get("workspacePaths")
        .and_then(|w| w.as_array())
        .and_then(|w| w.iter().find_map(usable))
        .or_else(|| obj.get("cwd").and_then(usable));
    Some(AgyPayload { workspace })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_never_overwrites() {
        let mut v = json!({"tool_name": "Bash", "toolName": "Read"});
        normalize_hook_payload(&mut v);
        assert_eq!(v["tool_name"], "Bash");
    }

    #[test]
    fn normalize_fills_absent_keys() {
        let mut v = json!({
            "sessionId": "s", "toolName": "Bash", "toolInput": {"command": "false"},
            "toolResponse": "r", "isInterrupt": true
        });
        normalize_hook_payload(&mut v);
        let input: HookInput = serde_json::from_value(v).unwrap();
        assert_eq!(input.session_id.as_deref(), Some("s"));
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert!(input.tool_input.is_some() && input.tool_response.is_some());
        assert!(input.interrupted());
    }

    #[test]
    fn normalize_ignores_non_objects() {
        for original in [json!("text"), json!([1, 2]), json!(null)] {
            let mut v = original.clone();
            normalize_hook_payload(&mut v);
            assert_eq!(v, original);
        }
    }

    #[test]
    fn camel_event_name_does_not_set_protocol_field() {
        let input: HookInput =
            serde_json::from_value(json!({"hookEventName": "PostToolUseFailure"})).unwrap();
        assert!(input.hook_event_name.is_none());
        assert!(input.is_failure_event());
    }

    #[test]
    fn failure_text_requires_non_blank_string() {
        let mk = |e| HookInput {
            error: Some(e),
            ..Default::default()
        };
        assert_eq!(mk(json!("  boom \n")).failure_text(), Some("boom"));
        assert_eq!(mk(json!("  ")).failure_text(), None);
        assert_eq!(mk(json!({"code": 1})).failure_text(), None);
    }

    fn agy_value(extra: serde_json::Value) -> serde_json::Value {
        let mut v = json!({"conversationId": "c-1", "hookEventName": "SessionStart"});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    }

    #[test]
    fn agy_payload_detected_and_rewritten() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = agy_value(json!({"workspacePaths": [tmp.path()]}));
        assert_eq!(
            adapt_agy_payload(&mut v),
            Some(AgyPayload {
                workspace: Some(tmp.path().to_path_buf())
            })
        );
        let input: HookInput = serde_json::from_value(v).unwrap();
        assert_eq!(input.session_id.as_deref(), Some("c-1"));
        assert_eq!(input.hook_event_name.as_deref(), Some("SessionStart"));
    }

    #[test]
    fn agy_never_overwrites() {
        let mut v = agy_value(json!({"session_id": "s-0"}));
        assert!(adapt_agy_payload(&mut v).is_some());
        assert_eq!(v["session_id"], "s-0");
    }

    #[test]
    fn grok_payload_is_not_agy() {
        let mut v = json!({
            "conversationId": "c",
            "hookEventName": "post_tool_use",
            "hook_event_name": "PostToolUse"
        });
        let before = v.clone();
        assert!(adapt_agy_payload(&mut v).is_none());
        assert_eq!(v, before);
    }

    #[test]
    fn claude_and_codex_payloads_are_not_agy() {
        for original in [
            json!({
                "session_id": "s", "hook_event_name": "PostToolUseFailure",
                "tool_name": "Bash", "tool_input": {"command": "false"},
                "error": "Exit code 1", "is_interrupt": false
            }),
            json!({
                "session_id": "s", "hook_event_name": "PostToolUse",
                "tool_name": "Bash", "tool_input": {"command": "ls"},
                "tool_response": "ok\n"
            }),
        ] {
            let mut v = original.clone();
            assert!(adapt_agy_payload(&mut v).is_none());
            assert_eq!(v, original);
        }
    }

    #[test]
    fn non_objects_untouched() {
        for original in [json!("text"), json!([1]), json!(null)] {
            let mut v = original.clone();
            assert!(adapt_agy_payload(&mut v).is_none());
            assert_eq!(v, original);
        }
    }

    #[test]
    fn conversation_id_must_be_string() {
        let mut v = json!({"conversationId": 7, "hookEventName": "Stop"});
        let before = v.clone();
        assert!(adapt_agy_payload(&mut v).is_none());
        assert_eq!(v, before);
    }

    #[test]
    fn workspace_order() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let missing = a.path().join("missing");
        let ws = |v: serde_json::Value| adapt_agy_payload(&mut agy_value(v)).unwrap().workspace;

        assert_eq!(
            ws(json!({"workspacePaths": [missing, a.path()], "cwd": b.path()})),
            Some(a.path().to_path_buf())
        );
        assert_eq!(
            ws(json!({"workspacePaths": [], "cwd": b.path()})),
            Some(b.path().to_path_buf())
        );
        assert_eq!(ws(json!({"workspacePaths": ["rel"]})), None);
    }
}
