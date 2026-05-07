//! orchestrate/mod.rs -- Agent orchestration hook module
//!
//! Manages the orchestration lifecycle via file-based state.
//! Gated by `EPIC_ORCHESTRATION=enabled` env var.

#![allow(dead_code)]

pub mod state;

use state::{
    self as orch_state, AgentEvent, AgentStatus, AgentStatusFile, ControlAction,
};

use super::common::{self, HookInput, hint, now_iso};

/// Check if orchestration is enabled via env var.
pub fn is_enabled() -> bool {
    common::is_orchestration_enabled()
}

/// Return the orchestrator base directory (uses harness_dir).
fn orch_base() -> std::path::PathBuf {
    common::harness_dir()
}

/// Pre-invocation hook logic.
///
/// 1. Check EPIC_ORCHESTRATION env var -- if not "enabled", no-op.
/// 2. Read control.json -- if action is "pause" for this agent, output hint.
/// 3. Read agent inbox -- output unread messages as hints.
/// 4. Update agent status.json heartbeat timestamp.
pub fn run_pre(input: &HookInput) -> i32 {
    if !is_enabled() {
        return 0;
    }

    let base = orch_base();
    let agent_id = match extract_agent_id(input) {
        Some(id) => id,
        None => return 0,
    };

    // Self-register if EPIC_RUN_ID is set and agent not yet in run.json
    let _ = orch_state::self_register_agent(&base, "dynamic", "auto-spawned agent");

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
                        &format!("Agent {} is paused by user directive (gen {})", agent_id, directive.generation),
                    );
                    if let Some(msg) = &directive.message {
                        hint("orchestrator", &format!("Reason: {}", msg));
                    }
                    return 2; // block the tool call
                }
                ControlAction::Cancel => {
                    hint(
                        "orchestrator",
                        &format!("Agent {} cancelled by user directive (gen {})", agent_id, directive.generation),
                    );
                    return 2;
                }
                ControlAction::Redirect => {
                    if let Some(msg) = &directive.message {
                        hint("orchestrator", &format!("Redirect notice: {}", msg));
                    }
                }
                ControlAction::Resume => {
                    // Resume clears any prior pause; no action needed at pre-invocation
                }
                ControlAction::Reassign => {
                    // Reassign: directive.message contains the target agent_id
                    if let Some(to_id) = &directive.message {
                        let _ = orch_state::reassign_agent(&base, &agent_id, to_id);
                        hint(
                            "orchestrator",
                            &format!("Task reassigned from {} to {}", agent_id, to_id),
                        );
                    }
                }
            }
        }
    }

    // Read unread inbox messages only
    let messages = orch_state::read_inbox_unread(&base, &agent_id);
    for msg in &messages {
        hint(
            "orchestrator",
            &format!("Inbox from {}: {}", msg.from, msg.message),
        );
        // Mark as read immediately
        let _ = orch_state::mark_inbox_read(&base, &agent_id, &msg.id);
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

    0
}

/// Post-invocation hook logic.
///
/// 1. Parse the Agent tool output to extract state.
/// 2. Append event to stream.jsonl.
/// 3. Update status.json with new state.
/// 4. If agent is DONE, evaluate dependency graph.
/// 5. If all agents are done/failed, update run.json status.
pub fn run_post(input: &HookInput) -> i32 {
    if !is_enabled() {
        return 0;
    }

    let base = orch_base();
    let agent_id = match extract_agent_id(input) {
        Some(id) => id,
        None => return 0,
    };

    // Extract output text
    let output = resolve_output(input);
    let new_status = orch_state::parse_agent_state(&output);

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

    // Check for send_to directive in structured output
    if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&output)
        && let (Some(to), Some(body)) = (
            doc.get("send_to").and_then(|v| v.as_str()),
            doc.get("body").and_then(|v| v.as_str()),
        )
    {
        let msg_type = doc
            .get("msg_type")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok())
            .unwrap_or(orch_state::MessageType::Result);
        let _ = orch_state::send_message(&base, &agent_id, to, msg_type, body);
    }

    // Update agent status
    if let Some(status) = new_status.clone() {
        if let Some(mut agent_status) = orch_state::read_agent_status(&base, &agent_id) {
            agent_status.status = status.clone();
            agent_status.last_heartbeat = now_iso();
            if status == AgentStatus::Done {
                agent_status.phase = "complete".to_string();
                agent_status.progress = 1.0;
            }
            let _ = orch_state::write_agent_status(&base, &agent_id, &agent_status);
        }

        // If agent is done, evaluate dependency graph
        if status == AgentStatus::Done
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
                let _ = orch_state::send_message(
                    &base,
                    "orchestrator",
                    ub_id,
                    orch_state::MessageType::Handoff,
                    &format!("Dependency '{}' completed. You are unblocked.", agent_id),
                );

                // Update run agent status to Pending (was Blocked)
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
    }

    0
}

/// Extract the agent ID from hook input.
/// Priority: EPIC_AGENT_ID env var > explicit agent_id field > prompt hash.
fn extract_agent_id(input: &HookInput) -> Option<String> {
    let tool = input.tool_name.as_deref()?;
    if tool.to_lowercase() != "agent" {
        return None;
    }
    // 1순위: EPIC_AGENT_ID env var
    if let Ok(id) = std::env::var("EPIC_AGENT_ID")
        && !id.is_empty() {
        return Some(id);
    }
    // 2순위: tool_input의 explicit agent_id
    if let Some(id) = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("agent_id"))
        .and_then(|v| v.as_str())
        .map(String::from)
    {
        return Some(id);
    }
    // 3순위: prompt 해시
    input
        .tool_input
        .as_ref()?
        .get("prompt")
        .and_then(|v| v.as_str())
        .map(|s| {
            let first_line = s.lines().next().unwrap_or(s);
            let hash = common::hash_string(first_line.trim());
            format!("agent-{}", &hash[..8])
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
    use serial_test::serial;

    #[test]
    #[serial]
    fn is_enabled_defaults_to_false() {
        // SAFETY: serialized via #[serial] to prevent cross-test env var races
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn is_enabled_true_when_set() {
        // SAFETY: serialized via #[serial]
        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        assert!(is_enabled());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
    fn extract_agent_id_prefers_env_var() {
        unsafe { std::env::set_var("EPIC_AGENT_ID", "env-agent-1"); }
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"agent_id": "explicit-id", "prompt": "test"})),
            ..Default::default()
        };
        let id = extract_agent_id(&input);
        unsafe { std::env::remove_var("EPIC_AGENT_ID"); }
        assert_eq!(id, Some("env-agent-1".to_string()));
    }

    #[test]
    #[serial]
    fn extract_agent_id_env_var_over_prompt_hash() {
        unsafe { std::env::set_var("EPIC_AGENT_ID", "env-agent-2"); }
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"prompt": "Build auth module"})),
            ..Default::default()
        };
        let id = extract_agent_id(&input);
        unsafe { std::env::remove_var("EPIC_AGENT_ID"); }
        assert_eq!(id, Some("env-agent-2".to_string()));
    }

    #[test]
    #[serial]
    fn extract_agent_id_explicit_id_when_no_env_var() {
        unsafe { std::env::remove_var("EPIC_AGENT_ID"); }
        let input = HookInput {
            tool_name: Some("Agent".to_string()),
            tool_input: Some(serde_json::json!({"agent_id": "explicit-id"})),
            ..Default::default()
        };
        assert_eq!(extract_agent_id(&input), Some("explicit-id".to_string()));
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
    fn resolve_output_empty_when_none() {
        let input = HookInput::default();
        assert_eq!(resolve_output(&input), "");
    }
}
