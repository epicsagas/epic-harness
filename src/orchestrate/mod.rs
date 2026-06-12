//! orchestrate/mod.rs -- Agent orchestration hook module
//!
//! Manages the orchestration lifecycle via file-based state.
//! Gated by `EPIC_ORCHESTRATION=enabled` env var.

#![allow(dead_code)]

pub mod state;

use state::{
    self as orch_state, AgentEvent, AgentStatus, AgentStatusFile, ControlAction, InboxMessage,
};

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
/// Always: record agent as "running" in orchestrator state (agent tracking).
/// Full orchestration only: check control directives, read inbox, update heartbeat.
pub fn run_pre(input: &HookInput) -> i32 {
    run_pre_checked(input).unwrap_or_else(|e| {
        eprintln!("[harness] orchestrate run_pre error: {e}");
        0
    })
}

/// Pre-invocation hook logic (returns Result for callers that want error handling).
pub fn run_pre_checked(input: &HookInput) -> Result<i32, Box<dyn std::error::Error>> {
    let tool = input.tool_name.as_deref().unwrap_or("");
    if tool.to_lowercase() != "agent" {
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

    // Always: record agent as running (agent tracking — no EPIC_ORCHESTRATION gate)
    let (description, subagent_type) = extract_agent_meta(input);
    orch_state::upsert_agent_to_run(
        &base,
        &agent_id,
        &description,
        &subagent_type,
        orch_state::AgentStatus::Running,
    )?;

    // Full orchestration: control, inbox, heartbeat
    if !is_full_orchestration() {
        return Ok(0);
    }

    // Check control directive
    if let Some(directive) = orch_state::read_control(&base) {
        let matches_target = directive.target.as_deref() == Some(&agent_id)
            || directive.target.as_deref() == Some("all")
            || directive.target.is_none();

        if matches_target {
            match directive.action {
                ControlAction::Pause => {
                    hint(
                        "orchestrator",
                        &format!(
                            "Agent {} is paused by user directive (gen {})",
                            agent_id, directive.generation
                        ),
                    );
                    if let Some(msg) = &directive.message {
                        hint("orchestrator", &format!("Reason: {}", msg));
                    }
                    return Ok(2); // block the tool call
                }
                ControlAction::Cancel => {
                    hint(
                        "orchestrator",
                        &format!(
                            "Agent {} cancelled by user directive (gen {})",
                            agent_id, directive.generation
                        ),
                    );
                    return Ok(2);
                }
                ControlAction::Redirect => {
                    if let Some(msg) = &directive.message {
                        hint("orchestrator", &format!("Redirect notice: {}", msg));
                    }
                }
                ControlAction::Resume => {
                    // Resume clears any prior pause; no action needed at pre-invocation
                }
            }
        }
    }

    // Read inbox messages
    let messages = orch_state::read_inbox(&base, &agent_id);
    for msg in &messages {
        hint(
            "orchestrator",
            &format!("Inbox from {}: {}", msg.from, msg.message),
        );
    }

    // Update heartbeat
    if let Some(mut status) = orch_state::read_agent_status(&base, &agent_id) {
        status.last_heartbeat = now_iso();
        let _ = orch_state::write_agent_status(&base, &agent_id, &status);
    } else {
        // First time seeing this agent -- create status
        let status = AgentStatusFile {
            agent_id: agent_id.clone(),
            phase: "running".to_string(),
            progress: 0.0,
            last_heartbeat: now_iso(),
            status: AgentStatus::Running,
        };
        let _ = orch_state::write_agent_status(&base, &agent_id, &status);
    }

    Ok(0)
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
    let tool = input.tool_name.as_deref().unwrap_or("");
    if tool.to_lowercase() != "agent" {
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

    // Always: update agent status in run.json (agent tracking — no gate)
    let final_status = new_status.clone().unwrap_or(orch_state::AgentStatus::Done);
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

    // Evaluate dependency graph if agent is done
    if final_status == AgentStatus::Done
        && let Some(mut run) = orch_state::read_run(&base)
    {
        // Update agent status in run
        for agent in &mut run.agents {
            if agent.id == agent_id {
                agent.status = AgentStatus::Done;
                agent.completed_at = Some(now_iso());
            }
        }

        // Evaluate deps
        let unblocked = orch_state::evaluate_dependencies(&run, &agent_id);
        for ub_id in &unblocked {
            let msg = InboxMessage {
                from: "orchestrator".to_string(),
                timestamp: now_iso(),
                message: format!("Dependency '{}' completed. You are unblocked.", agent_id),
            };
            let _ = orch_state::post_inbox_message(&base, ub_id, &msg);

            // Update run agent status
            for agent in &mut run.agents {
                if agent.id == *ub_id {
                    agent.status = AgentStatus::Pending;
                }
            }
        }

        // Check if run is complete
        if orch_state::is_run_complete(&run) {
            run.status = orch_state::RunStatus::Complete;
        }
        run.updated_at = now_iso();
        let _ = orch_state::write_run(&base, &run);
    }

    Ok(0)
}

/// Extract the agent ID from hook input.
/// Agent tool input typically has a "prompt" or "id" field.
fn extract_agent_id(input: &HookInput) -> Option<String> {
    let tool = input.tool_name.as_deref()?;
    if tool.to_lowercase() != "agent" {
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
    let sub_type = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("subagent_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("general-purpose")
        .to_string();
    (desc, sub_type)
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
}
