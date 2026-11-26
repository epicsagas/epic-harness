//! orchestrate/mod.rs -- Agent orchestration hook module
//!
//! Manages the orchestration lifecycle via file-based state.
//! Gated by `EPIC_ORCHESTRATION=enabled` env var.

#![allow(dead_code)]

pub mod state;

use state::{
    self as orch_state, AgentEvent, AgentStatus, AgentStatusFile, ControlAction, InboxMessage,
};
use std::path::Path;

use crate::hooks::common::{self, HookInput, hint, now_iso};

/// Check if orchestration is enabled via env var.
pub fn is_enabled() -> bool {
    std::env::var("EPIC_ORCHESTRATION").as_deref() == Ok("enabled")
}

/// Full orchestration (control directives, inbox, dependency evaluation).
fn is_full_orchestration() -> bool {
    is_enabled()
}

/// Return the orchestrator base directory (uses harness_dir).
fn orch_base() -> std::path::PathBuf {
    common::harness_dir()
}

/// Pre-invocation hook logic (returns exit code, errors logged to stderr).
///
/// Always: record agent as "running" (or "blocked" when `spawn_block` refuses it).
/// Full orchestration only: deny a blocked spawn, read inbox, update heartbeat.
pub fn run_pre(input: &HookInput) -> i32 {
    run_pre_checked(input).unwrap_or_else(|e| {
        eprintln!("[harness] orchestrate run_pre error: {e}");
        0
    })
}

/// Pre-invocation hook logic (returns Result for callers that want error handling).
pub fn run_pre_checked(input: &HookInput) -> Result<i32, Box<dyn std::error::Error>> {
    run_pre_in(&orch_base(), input)
}

fn run_pre_in(base: &Path, input: &HookInput) -> Result<i32, Box<dyn std::error::Error>> {
    if !is_agent_event(input) {
        return Ok(0);
    }

    let agent_id = match extract_agent_id(input) {
        Some(id) => id,
        None => return Ok(0),
    };

    // Runtime validation — always active (not gated by debug_assert)
    if !orch_state::validate_agent_id(&agent_id) {
        return Ok(0);
    }

    // The same decision the Claude PreToolUse guard enforces, evaluated before
    // the upsert so the recorded status always matches the gate (I3): only an
    // allowed spawn is recorded as Running, and only that can start a new run.
    // Hosts that cannot refuse the spawn run it anyway, so they record Running.
    // Known limitation: `Blocked` is shared with dependency blocking, so a
    // paused agent can be flipped to `Pending` when its dependency finishes.
    let block = if spawn_gate_enforced(input) {
        orch_state::spawn_block(base, &agent_id, orch_state::epoch_secs_now())
    } else {
        None
    };

    // Always: record agent (agent tracking, no EPIC_ORCHESTRATION gate)
    let (description, subagent_type) = extract_agent_meta(input);
    orch_state::upsert_agent_to_run(
        base,
        &agent_id,
        &description,
        &subagent_type,
        if block.is_some() {
            AgentStatus::Blocked
        } else {
            AgentStatus::Running
        },
    )?;

    // Full orchestration: control, inbox, heartbeat
    if !is_full_orchestration() {
        return Ok(0);
    }

    if let Some(block) = block {
        for line in block.reason_lines(&agent_id) {
            hint("orchestrator", &line);
        }
        return Ok(2); // block the tool call
    }

    if let Some(directive) = orch_state::read_control(base)
        && directive.action == ControlAction::Redirect
        && directive.targets(&agent_id)
        && let Some(msg) = &directive.message
    {
        hint("orchestrator", &format!("Redirect notice: {}", msg));
    }

    // Read inbox messages
    let messages = orch_state::read_inbox(base, &agent_id);
    for msg in &messages {
        hint(
            "orchestrator",
            &format!("Inbox from {}: {}", msg.from, msg.message),
        );
    }

    // Update heartbeat
    if let Some(mut status) = orch_state::read_agent_status(base, &agent_id) {
        status.last_heartbeat = now_iso();
        let _ = orch_state::write_agent_status(base, &agent_id, &status);
    } else {
        // First time seeing this agent -- create status
        let status = AgentStatusFile {
            agent_id: agent_id.clone(),
            phase: "running".to_string(),
            progress: 0.0,
            last_heartbeat: now_iso(),
            status: AgentStatus::Running,
        };
        let _ = orch_state::write_agent_status(base, &agent_id, &status);
    }

    Ok(0)
}

/// True when the host refuses a blocked spawn synchronously: Claude's
/// `PreToolUse/Agent` guard handler, which only runs with the guard profile
/// enabled. Codex `SubagentStart` and a disabled guard cannot stop the
/// subagent, so a block there must not be recorded as Blocked.
fn spawn_gate_enforced(input: &HookInput) -> bool {
    input.hook_event_name.as_deref() == Some("PreToolUse")
        && input.hook_event_name_camel.is_none()
        && !std::env::var("GROK_HOOK_EVENT").is_ok_and(|v| !v.is_empty())
        && common::should_run(common::PROFILE_GUARD)
}

/// Post-invocation hook logic (returns exit code, errors logged to stderr).
///
/// Always: record agent completion in orchestrator state (agent tracking).
/// Full orchestration only: append events, evaluate dependencies, inbox notifications.
pub fn run_post(input: &HookInput) -> i32 {
    run_post_checked(input).unwrap_or_else(|e| {
        eprintln!("[harness] orchestrate run_post error: {e}");
        0
    })
}

/// Post-invocation hook logic (returns Result for callers that want error handling).
pub fn run_post_checked(input: &HookInput) -> Result<i32, Box<dyn std::error::Error>> {
    if !is_agent_event(input) {
        return Ok(0);
    }

    let base = orch_base();
    let agent_id = match extract_agent_id(input) {
        Some(id) => id,
        None => return Ok(0),
    };

    // Runtime validation — always active (not gated by debug_assert)
    if !orch_state::validate_agent_id(&agent_id) {
        return Ok(0);
    }

    // Extract output text
    let output = resolve_output(input);
    let new_status = orch_state::parse_agent_state(&output);

    // Always: update agent status in run.json (agent tracking, no gate).
    // A user-cancelled agent stays Cancelled (upsert_agent_to_run enforces it;
    // this also skips the dependency unblock and "completed" inbox below).
    let cancelled = orch_state::read_run(&base).is_some_and(|r| {
        r.agents
            .iter()
            .any(|a| a.id == agent_id && a.status == AgentStatus::Cancelled)
    });
    let final_status = if cancelled {
        AgentStatus::Cancelled
    } else if tool_reported_error(input) {
        AgentStatus::Failed
    } else {
        new_status.clone().unwrap_or(AgentStatus::Done)
    };
    let (description, subagent_type) = extract_agent_meta(input);
    orch_state::upsert_agent_to_run(
        &base,
        &agent_id,
        &description,
        &subagent_type,
        final_status.clone(),
    )?;

    // Full orchestration: events, dependency evaluation, inbox notifications
    if !is_full_orchestration() {
        return Ok(0);
    }

    // Append event
    let event = AgentEvent {
        timestamp: now_iso(),
        event_type: "tool_result".to_string(),
        data: serde_json::json!({
            "agent_id": agent_id,
            "status": new_status.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default()),
            "output_len": output.len(),
        }),
    };
    let _ = orch_state::append_event(&base, &agent_id, &event);

    // Unblock dependents under the run.json lock (Done or Failed). Inbox
    // messages go out only after run.json was written.
    if matches!(final_status, AgentStatus::Done | AgentStatus::Failed) {
        let unblocked = orch_state::complete_and_unblock(&base, &agent_id, final_status.clone())?;
        let outcome = if final_status == AgentStatus::Done {
            "completed"
        } else {
            "failed"
        };
        for ub_id in &unblocked {
            let msg = InboxMessage {
                from: "orchestrator".to_string(),
                timestamp: now_iso(),
                message: format!("Dependency '{agent_id}' {outcome}. You are unblocked."),
            };
            let _ = orch_state::post_inbox_message(&base, ub_id, &msg);
        }
    }

    Ok(0)
}

/// Extract the agent ID from hook input.
/// Agent tool input typically has a "prompt" or "id" field.
/// True when this hook invocation concerns a subagent.
///
/// Claude Code routes subagents through the `Agent` tool; Codex reports them
/// as dedicated `SubagentStart` / `SubagentStop` events with no such tool
/// name, so a tool-name-only gate silently ignored every Codex subagent.
pub(crate) fn is_agent_event(input: &HookInput) -> bool {
    if input
        .tool_name
        .as_deref()
        .map(|t| t.to_lowercase() == "agent")
        .unwrap_or(false)
    {
        return true;
    }
    matches!(
        input.hook_event_name.as_deref(),
        Some("SubagentStart") | Some("SubagentStop")
    )
}

pub(crate) fn extract_agent_id(input: &HookInput) -> Option<String> {
    // Lifecycle events carry the subagent's own id; prefer it over the prompt
    // hash. On tool events the payload `agent_id` names the CALLER (a nested
    // spawn inside a subagent carries that subagent's id), so it is ignored.
    if matches!(
        input.hook_event_name.as_deref(),
        Some("SubagentStart") | Some("SubagentStop")
    ) && let Some(id) = input.agent_id.as_deref()
        && !id.trim().is_empty()
    {
        return Some(id.trim().to_string());
    }
    if !is_agent_event(input) {
        return None;
    }
    input
        .tool_input
        .as_ref()?
        .get("prompt")
        .and_then(|v| v.as_str())
        .map(|s| {
            // Use first line or first 32 chars as a simple ID
            let first_line = s.lines().next().unwrap_or(s);
            let trimmed = first_line.trim();
            // Generate a stable hash-based ID
            let hash = common::hash_string(trimmed);
            format!("agent-{}", &hash[..8])
        })
        .or_else(|| {
            // Fallback: look for explicit "agent_id" field
            input
                .tool_input
                .as_ref()?
                .get("agent_id")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
}

/// Extract agent metadata from tool input (description + subagent_type).
/// Falls back to `prompt` first line when `description` is absent.
/// Truncates description to 120 chars to limit secret exposure.
fn extract_agent_meta(input: &HookInput) -> (String, String) {
    let desc: String = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("description"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            // Fallback: use first line of prompt
            input
                .tool_input
                .as_ref()
                .and_then(|v| v.get("prompt"))
                .and_then(|v| v.as_str())
                .map(|s| s.lines().next().unwrap_or(s).to_string())
        })
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect();
    // Codex reports the kind as a top-level `agent_type`; Claude nests
    // `subagent_type` inside the Agent tool input.
    let sub_type = input
        .agent_type
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            input
                .tool_input
                .as_ref()
                .and_then(|v| v.get("subagent_type"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "general-purpose".to_string());
    (desc, sub_type)
}

/// True when the host reports the tool call as an error: `tool_response` or
/// `tool_result` is an object with `is_error == true` or a non-empty string
/// `error`, or the event is `PostToolUseFailure` (which also covers
/// interrupted calls). Hosts that send none of these give false.
fn tool_reported_error(input: &HookInput) -> bool {
    input.is_failure_event()
        || [&input.tool_response, &input.tool_result]
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_object())
            .any(|o| {
                o.get("is_error").and_then(|v| v.as_bool()) == Some(true)
                    || o.get("error")
                        .and_then(|v| v.as_str())
                        .is_some_and(|e| !e.is_empty())
            })
}

/// Resolve the output text from hook input.
fn resolve_output(input: &HookInput) -> String {
    if let Some(to) = &input.tool_output {
        return format!(
            "{}\n{}",
            to.output.as_deref().unwrap_or(""),
            to.stderr.as_deref().unwrap_or("")
        );
    }
    if let Some(tr) = &input.tool_response {
        return match tr {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(obj) => {
                let out = obj.get("output").and_then(|v| v.as_str()).unwrap_or("");
                let err = obj.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                format!("{out}\n{err}")
            }
            other => other.to_string(),
        };
    }
    if let Some(tr) = &input.tool_result {
        return match tr {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(obj) => {
                let out = obj.get("output").and_then(|v| v.as_str()).unwrap_or("");
                let err = obj.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                format!("{out}\n{err}")
            }
            other => other.to_string(),
        };
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn is_enabled_defaults_to_false() {
        // SAFETY: isolated test, we ensure the var is removed
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        assert!(!is_enabled());
    }

    #[test]
    #[serial_test::serial]
    fn is_enabled_true_when_set() {
        // SAFETY: isolated test
        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        assert!(is_enabled());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    #[serial_test::serial]
    fn is_enabled_false_for_other_values() {
        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "true");
        }
        assert!(!is_enabled());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    #[serial_test::serial]
    fn run_pre_returns_0_when_disabled() {
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "test"})),
            ..Default::default()
        };
        assert_eq!(run_pre(&input), 0);
    }

    #[test]
    #[serial_test::serial]
    fn run_post_returns_0_when_disabled() {
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        let input = HookInput::default();
        assert_eq!(run_post(&input), 0);
    }

    /// A late `## Status: DONE` from an agent the user cancelled must not
    /// flip it back to Done.
    #[test]
    #[serial_test::serial]
    fn run_post_never_overwrites_a_cancelled_agent() {
        let base = common::harness_dir();
        let id = "cancelled-post-test";
        orch_state::upsert_agent_to_run(&base, id, "t", "builder", AgentStatus::Running).unwrap();
        let mut run = orch_state::read_run(&base).unwrap();
        run.agents.iter_mut().find(|a| a.id == id).unwrap().status = AgentStatus::Cancelled;
        orch_state::write_run(&base, &run).unwrap();

        let input = HookInput {
            hook_event_name: Some("SubagentStop".to_string()),
            agent_id: Some(id.to_string()),
            tool_response: Some(serde_json::json!("## Status: DONE")),
            ..Default::default()
        };
        assert_eq!(run_post(&input), 0);
        let run = orch_state::read_run(&base).unwrap();
        let agent = run.agents.iter().find(|a| a.id == id).unwrap();
        assert_eq!(agent.status, AgentStatus::Cancelled);
    }

    /// Codex has no `Agent` tool — subagents arrive as lifecycle events.
    #[test]
    fn codex_subagent_events_count_as_agent_events() {
        for ev in ["SubagentStart", "SubagentStop"] {
            let input = HookInput {
                hook_event_name: Some(ev.to_string()),
                agent_id: Some("agent-7f3a".to_string()),
                ..Default::default()
            };
            assert!(is_agent_event(&input), "{ev} must be tracked");
            assert_eq!(extract_agent_id(&input).as_deref(), Some("agent-7f3a"));
        }
    }

    /// The host-supplied id wins over the prompt hash, and agent_type maps to
    /// the Claude subagent_type slot.
    #[test]
    fn codex_identity_fields_preferred() {
        let input = HookInput {
            hook_event_name: Some("SubagentStart".to_string()),
            agent_id: Some("codex-abc123".to_string()),
            agent_type: Some("reviewer".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "Review the diff"})),
            ..Default::default()
        };
        assert_eq!(extract_agent_id(&input).as_deref(), Some("codex-abc123"));
        let (_desc, sub_type) = extract_agent_meta(&input);
        assert_eq!(sub_type, "reviewer");
    }

    /// Unrelated events must stay ignored.
    #[test]
    fn non_agent_events_ignored() {
        let input = HookInput {
            hook_event_name: Some("PostToolUse".to_string()),
            tool_name: Some("Bash".to_string()),
            ..Default::default()
        };
        assert!(!is_agent_event(&input));
    }

    #[test]
    fn extract_agent_id_from_prompt() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "Build the auth module"})),
            ..Default::default()
        };
        let id = extract_agent_id(&input);
        assert!(id.is_some());
        let id = id.unwrap();
        assert!(id.starts_with("agent-"));
        assert_eq!(id.len(), 14); // "agent-" + 8 hex chars
    }

    /// On Claude a tool event's `agent_id` names the CALLER, so a nested
    /// spawn is identified by its prompt hash, never by the parent's id.
    #[test]
    fn nested_claude_tool_events_ignore_payload_agent_id() {
        let hash_id = extract_agent_id(&HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "X"})),
            ..Default::default()
        })
        .unwrap();
        assert!(hash_id.starts_with("agent-"));
        for ev in ["PreToolUse", "PostToolUse"] {
            let input = HookInput {
                hook_event_name: Some(ev.to_string()),
                tool_name: Some("Agent".to_string()),
                agent_id: Some("a-parent".to_string()),
                tool_input: Some(serde_json::json!({"prompt": "X"})),
                ..Default::default()
            };
            assert_eq!(extract_agent_id(&input), Some(hash_id.clone()), "{ev}");
        }
    }

    /// Claude lifecycle events carry the subagent's own id.
    #[test]
    fn claude_subagent_start_uses_payload_agent_id() {
        let input = HookInput {
            hook_event_name: Some("SubagentStart".to_string()),
            agent_id: Some("a-child".to_string()),
            ..Default::default()
        };
        assert_eq!(extract_agent_id(&input).as_deref(), Some("a-child"));
    }

    /// Cancelling the parent must not block its nested spawn of another
    /// prompt, while cancelling the nested prompt-hash id does.
    #[test]
    #[serial_test::serial]
    fn nested_spawn_is_blocked_by_its_own_id_not_the_parents() {
        let input = HookInput {
            hook_event_name: Some("PreToolUse".to_string()),
            tool_name: Some("Agent".to_string()),
            agent_id: Some("a-parent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "X"})),
            ..Default::default()
        };
        let nested = extract_agent_id(&input).unwrap();
        // SAFETY: serial test, the var is removed below
        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        for (cancelled, expected) in [("a-parent", 0), (nested.as_str(), 2)] {
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path();
            gate_fixture(
                base,
                orch_state::RunStatus::Running,
                &[(cancelled, AgentStatus::Cancelled)],
                None,
            );
            assert_eq!(run_pre_in(base, &input).unwrap(), expected, "{cancelled}");
        }
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    fn extract_agent_id_from_explicit_id() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"agent_id": "builder-1"})),
            ..Default::default()
        };
        assert_eq!(extract_agent_id(&input), Some("builder-1".to_string()));
    }

    #[test]
    fn extract_agent_id_none_for_non_agent_tool() {
        let input = HookInput {
            tool_name: Some("Bash".to_string()),
            tool_input: Some(serde_json::json!({"command": "ls"})),
            ..Default::default()
        };
        assert!(extract_agent_id(&input).is_none());
    }

    #[test]
    fn extract_agent_id_stable_for_same_prompt() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "Build the auth module"})),
            ..Default::default()
        };
        let id1 = extract_agent_id(&input);
        let id2 = extract_agent_id(&input);
        assert_eq!(id1, id2);
    }

    #[test]
    fn resolve_output_from_tool_output() {
        let input = HookInput {
            tool_output: Some(common::ToolOutput {
                output: Some("## Status: DONE".to_string()),
                stderr: None,
            }),
            ..Default::default()
        };
        assert_eq!(resolve_output(&input), "## Status: DONE\n");
    }

    #[test]
    fn resolve_output_from_tool_response_string() {
        let input = HookInput {
            tool_response: Some(serde_json::json!("## Status: BLOCKED")),
            ..Default::default()
        };
        assert_eq!(resolve_output(&input), "## Status: BLOCKED");
    }

    #[test]
    fn extract_agent_meta_from_description() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({
                "description": "Build the auth module",
                "prompt": "Full prompt text here"
            })),
            ..Default::default()
        };
        let (desc, sub_type) = extract_agent_meta(&input);
        assert_eq!(desc, "Build the auth module");
        assert_eq!(sub_type, "general-purpose");
    }

    #[test]
    fn extract_agent_meta_falls_back_to_prompt() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({
                "prompt": "Review the code changes\nThis is a longer prompt"
            })),
            ..Default::default()
        };
        let (desc, _) = extract_agent_meta(&input);
        assert_eq!(desc, "Review the code changes");
    }

    #[test]
    fn extract_agent_meta_truncates_long_prompt() {
        let long_desc = "x".repeat(200);
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({
                "description": long_desc
            })),
            ..Default::default()
        };
        let (desc, _) = extract_agent_meta(&input);
        assert_eq!(desc.len(), 120);
    }

    #[test]
    fn extract_agent_meta_empty_when_no_fields() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({})),
            ..Default::default()
        };
        let (desc, sub_type) = extract_agent_meta(&input);
        assert!(desc.is_empty());
        assert_eq!(sub_type, "general-purpose");
    }

    #[test]
    fn extract_agent_meta_uses_subagent_type() {
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({
                "description": "Test task",
                "subagent_type": "code-reviewer"
            })),
            ..Default::default()
        };
        let (_, sub_type) = extract_agent_meta(&input);
        assert_eq!(sub_type, "code-reviewer");
    }

    #[test]
    fn resolve_output_empty_when_none() {
        let input = HookInput::default();
        assert_eq!(resolve_output(&input), "");
    }

    #[test]
    fn tool_reported_error_golden() {
        let resp = |v: serde_json::Value| HookInput {
            tool_response: Some(v),
            ..Default::default()
        };
        assert!(tool_reported_error(&resp(
            serde_json::json!({"is_error": true})
        )));
        assert!(tool_reported_error(&resp(
            serde_json::json!({"error": "boom"})
        )));
        assert!(!tool_reported_error(&resp(
            serde_json::json!({"error": ""})
        )));
        assert!(!tool_reported_error(&resp(serde_json::json!("text"))));
        assert!(!tool_reported_error(&HookInput::default()));
        let result = HookInput {
            tool_result: Some(serde_json::json!({"is_error": true})),
            ..Default::default()
        };
        assert!(tool_reported_error(&result));
    }

    #[test]
    fn tool_reported_error_for_failure_event() {
        let failure = |extra: serde_json::Value| HookInput {
            hook_event_name: Some("PostToolUseFailure".into()),
            tool_name: Some("Agent".into()),
            error: Some(extra),
            ..Default::default()
        };
        assert!(tool_reported_error(&failure(serde_json::json!("boom"))));
        // No usable error text still counts: the event itself is the signal.
        assert!(tool_reported_error(&failure(
            serde_json::json!({"code": 1})
        )));
        let interrupted = HookInput {
            is_interrupt: Some(serde_json::json!(true)),
            ..failure(serde_json::json!(null))
        };
        assert!(tool_reported_error(&interrupted));
    }

    // ── Spawn gate: run_pre records what the guard decides (I3) ──

    use crate::shared::helpers::iso_from_secs;

    /// run.json `auto-r1` created 100s and updated 10s ago, control.json (if
    /// any) written now.
    fn gate_fixture(
        base: &Path,
        status: orch_state::RunStatus,
        agents: &[(&str, AgentStatus)],
        control: Option<(ControlAction, Option<&str>)>,
    ) {
        let now = orch_state::epoch_secs_now();
        let run = orch_state::OrchestrationRun {
            id: "auto-r1".to_string(),
            status,
            agents: agents
                .iter()
                .map(|(id, st)| orch_state::AgentDef {
                    id: id.to_string(),
                    role: "builder".to_string(),
                    task: "t".to_string(),
                    satisfies: vec![],
                    status: st.clone(),
                    started_at: None,
                    completed_at: None,
                })
                .collect(),
            dependency_graph: Default::default(),
            created_at: iso_from_secs(now - 100),
            updated_at: iso_from_secs(now - 10),
        };
        std::fs::create_dir_all(orch_state::orchestrator_dir(base)).unwrap();
        orch_state::write_run(base, &run).unwrap();
        if let Some((action, target)) = control {
            let directive = orch_state::ControlDirective {
                action,
                target: target.map(String::from),
                message: None,
                generation: 1,
            };
            orch_state::write_control(base, &directive).unwrap();
        }
    }

    #[test]
    #[serial_test::serial]
    fn run_pre_records_blocked_iff_gate_blocks() {
        use orch_state::RunStatus::{Aborted, Running};
        struct Row {
            name: &'static str,
            status: orch_state::RunStatus,
            agents: Vec<(&'static str, AgentStatus)>,
            control: Option<(ControlAction, Option<&'static str>)>,
            blocked: bool,
            recorded: AgentStatus,
        }
        let rows = [
            Row {
                name: "3: aborted + resume all",
                status: Aborted,
                agents: vec![("a", AgentStatus::Running)],
                control: Some((ControlAction::Resume, Some("all"))),
                blocked: false,
                recorded: AgentStatus::Running,
            },
            Row {
                name: "4: aborted, no directive",
                status: Aborted,
                agents: vec![("a", AgentStatus::Running)],
                control: None,
                blocked: true,
                recorded: AgentStatus::Blocked,
            },
            Row {
                name: "6: agent cancelled (the denied respawn stays cancelled)",
                status: Running,
                agents: vec![("x", AgentStatus::Cancelled), ("y", AgentStatus::Running)],
                control: None,
                blocked: true,
                recorded: AgentStatus::Cancelled,
            },
            Row {
                name: "7: pause x",
                status: Running,
                agents: vec![("y", AgentStatus::Running)],
                control: Some((ControlAction::Pause, Some("x"))),
                blocked: true,
                recorded: AgentStatus::Blocked,
            },
            Row {
                name: "9: pause for another id",
                status: Running,
                agents: vec![("y", AgentStatus::Running)],
                control: Some((ControlAction::Pause, Some("y"))),
                blocked: false,
                recorded: AgentStatus::Running,
            },
        ];
        let input = HookInput {
            hook_event_name: Some("PreToolUse".to_string()),
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "synthetic task"})),
            ..Default::default()
        };
        // A tool event is identified by its prompt hash; rows name it "x".
        let xid = extract_agent_id(&input).unwrap();
        let sub = |id: &str| {
            if id == "x" {
                xid.clone()
            } else {
                id.to_string()
            }
        };
        for orchestration in [false, true] {
            // SAFETY: serial test, the var is restored below
            unsafe {
                if orchestration {
                    std::env::set_var("EPIC_ORCHESTRATION", "enabled");
                } else {
                    std::env::remove_var("EPIC_ORCHESTRATION");
                }
            }
            for row in &rows {
                let tmp = tempfile::tempdir().unwrap();
                let base = tmp.path();
                let agents: Vec<(String, AgentStatus)> = row
                    .agents
                    .iter()
                    .map(|(id, st)| (sub(id), st.clone()))
                    .collect();
                let agents: Vec<(&str, AgentStatus)> = agents
                    .iter()
                    .map(|(id, st)| (id.as_str(), st.clone()))
                    .collect();
                let control = row.control.clone().map(|(a, t)| (a, t.map(sub)));
                let control = control.as_ref().map(|(a, t)| (a.clone(), t.as_deref()));
                gate_fixture(base, row.status.clone(), &agents, control);
                let code = run_pre_in(base, &input).unwrap();
                let case = format!("{} (EPIC_ORCHESTRATION {orchestration})", row.name);
                assert_eq!(
                    code,
                    if orchestration && row.blocked { 2 } else { 0 },
                    "{case}"
                );
                let run = orch_state::read_run(base).unwrap();
                let x = run.agents.iter().find(|a| a.id == xid).unwrap();
                assert_eq!(x.status, row.recorded, "{case}");
                if row.status == Aborted && row.blocked {
                    assert_eq!(run.status, Aborted, "{case}: abort stays");
                }
                if row.status == Aborted && !row.blocked {
                    assert_ne!(
                        run.id, "auto-r1",
                        "{case}: resume then spawn starts a new run"
                    );
                }
            }
        }
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    /// Codex runs the subagent whatever `SubagentStart` returns, so a live
    /// pause must not be recorded as Blocked there (or hide a running agent).
    #[test]
    #[serial_test::serial]
    fn codex_subagent_start_records_running_under_a_live_pause() {
        let input = HookInput {
            hook_event_name: Some("SubagentStart".to_string()),
            agent_id: Some("x".to_string()),
            tool_input: Some(serde_json::json!({"description": "synthetic task"})),
            ..Default::default()
        };
        for orchestration in [false, true] {
            // SAFETY: serial test, the var is removed below
            unsafe {
                if orchestration {
                    std::env::set_var("EPIC_ORCHESTRATION", "enabled");
                } else {
                    std::env::remove_var("EPIC_ORCHESTRATION");
                }
            }
            let tmp = tempfile::tempdir().unwrap();
            let base = tmp.path();
            gate_fixture(
                base,
                orch_state::RunStatus::Running,
                &[("y", AgentStatus::Running)],
                Some((ControlAction::Pause, Some("all"))),
            );
            assert_eq!(run_pre_in(base, &input).unwrap(), 0);
            let run = orch_state::read_run(base).unwrap();
            assert_eq!(run.id, "auto-r1", "same live run");
            let x = run.agents.iter().find(|a| a.id == "x").unwrap();
            assert_eq!(x.status, AgentStatus::Running);
        }
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }
}
