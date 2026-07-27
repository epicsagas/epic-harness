//! host.rs — Host protocol capabilities.
//!
//! Claude Code and the Codex-family hosts (Codex, Antigravity) consume hook
//! output differently, and the difference is per-event rather than per-host:
//!
//! * Claude Code follows the stdin-passthrough contract — `main` echoes stdin on
//!   stdout and every human-facing line belongs on stderr.
//! * Codex adds **plain text written to stdout** to the model's context, but only
//!   for `SessionStart`, `SubagentStart` and `UserPromptSubmit`. `PreToolUse` and
//!   `PostToolUse` discard plain text outright, and the remaining events
//!   (`Stop`, `SessionEnd`, `PreCompact`, `PostCompact`, `SubagentStop`) expect
//!   structured JSON, where stray text would corrupt the payload.
//!
//! So resume context sent to stderr never reached a Codex model at all, while
//! blindly switching every hook to stdout would either be dropped or break the
//! JSON events. `init` records the one bit the output helpers need.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether plain text on stdout is picked up by the host as model context.
static STDOUT_IS_CONTEXT: AtomicBool = AtomicBool::new(false);

/// Codex events whose plain stdout becomes extra developer context.
fn event_takes_plain_stdout(event: &str) -> bool {
    matches!(event, "SessionStart" | "SubagentStart" | "UserPromptSubmit")
}

/// Record the active host protocol from the parsed hook input.
///
/// `hook_event_name` is present only for Codex-family hosts; `None` means the
/// Claude Code contract, which keeps human-facing output on stderr.
pub fn init(hook_event_name: Option<&str>) {
    let takes = hook_event_name
        .map(event_takes_plain_stdout)
        .unwrap_or(false);
    STDOUT_IS_CONTEXT.store(takes, Ordering::Relaxed);
}

/// True when `hint`/`raw` should write to stdout so the model actually sees it.
pub fn stdout_is_context() -> bool {
    STDOUT_IS_CONTEXT.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_keeps_stderr() {
        // No hook_event_name => Claude Code stdin-passthrough contract.
        init(None);
        assert!(!stdout_is_context());
    }

    #[test]
    fn codex_session_start_uses_stdout() {
        init(Some("SessionStart"));
        assert!(stdout_is_context());
    }

    #[test]
    fn codex_tool_events_keep_stderr() {
        // PostToolUse/PreToolUse discard plain stdout — writing context there
        // would silently vanish.
        for ev in ["PreToolUse", "PostToolUse"] {
            init(Some(ev));
            assert!(!stdout_is_context(), "{ev} must not use stdout");
        }
    }

    #[test]
    fn codex_json_events_keep_stderr() {
        // These expect structured JSON; plain text would corrupt the payload.
        for ev in [
            "Stop",
            "SessionEnd",
            "PreCompact",
            "PostCompact",
            "SubagentStop",
        ] {
            init(Some(ev));
            assert!(!stdout_is_context(), "{ev} must not use stdout");
        }
    }

    #[test]
    fn unknown_event_defaults_to_stderr() {
        init(Some("SomeFutureEvent"));
        assert!(!stdout_is_context());
    }
}
