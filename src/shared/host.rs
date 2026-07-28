//! host.rs — Host protocol capabilities.
//!
//! Claude Code and the Codex-family hosts (Codex, Antigravity) consume hook
//! output differently, and the difference is per-event rather than per-host:
//!
//! * Claude Code follows the stdin-passthrough contract — `main` echoes stdin on
//!   stdout and every human-facing line belongs on stderr.
//! * Codex accepts model context from `SessionStart`, `SubagentStart` and
//!   `UserPromptSubmit`. SessionStart context is emitted as structured JSON
//!   because tagged text beginning with `[` is otherwise parsed as malformed
//!   JSON. The other two accept plain stdout. `PreToolUse` and `PostToolUse`
//!   discard plain text outright, and the remaining events (`Stop`,
//!   `SessionEnd`, `PreCompact`, `PostCompact`, `SubagentStop`) expect
//!   structured JSON, where stray text would corrupt the payload.
//!
//! So resume context sent to stderr never reached a Codex model at all, while
//! blindly switching every hook to stdout would either be dropped or break the
//! JSON events. `init` records the one bit the output helpers need.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use super::types::HookInput;

/// Whether plain text on stdout is picked up by the host as model context.
static STDOUT_IS_CONTEXT: AtomicBool = AtomicBool::new(false);

/// SessionStart context must be emitted as one JSON object on Codex. Buffer
/// the lines that `hint`/`raw` would otherwise print separately.
static CAPTURE_SESSION_START: AtomicBool = AtomicBool::new(false);
static SESSION_START_CONTEXT: Mutex<String> = Mutex::new(String::new());

/// Host-supplied conversation id, sanitized for use in filenames.
static HOST_SESSION_ID: RwLock<Option<String>> = RwLock::new(None);

/// Host-supplied subagent id, when the active event concerns a subagent.
static HOST_AGENT_ID: RwLock<Option<String>> = RwLock::new(None);

/// Longest host id we keep. Codex sends UUIDs (36 chars); the cap only guards
/// against a host that sends something unbounded.
const MAX_ID_LEN: usize = 64;

/// Keep `A-Za-z0-9`, `-` and `_`; drop everything else.
///
/// Host ids end up in filenames (`session_{id}.jsonl`, `resume.{id}.lock`), so a
/// value containing a path separator would escape the harness directory.
/// Returns `None` for an empty or fully-rejected id, which keeps the caller on
/// its existing fallback.
fn sanitize_id(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(MAX_ID_LEN)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Codex events whose plain stdout becomes extra developer context.
fn event_takes_plain_stdout(event: &str) -> bool {
    matches!(event, "SubagentStart" | "UserPromptSubmit")
}

/// Record the active host protocol and identity from the parsed hook input.
///
/// `hook_event_name` is present only for Codex-family hosts; `None` means the
/// Claude Code contract, which keeps human-facing output on stderr.
///
/// The identity fields are what stop `session_id()` from manufacturing one
/// "session" per hook process. Both hosts supply `session_id`; only Codex
/// supplies `agent_id`.
pub fn init(input: &HookInput) {
    let capture_session_start = input.hook_event_name.as_deref() == Some("SessionStart");
    let takes = input
        .hook_event_name
        .as_deref()
        .map(event_takes_plain_stdout)
        .unwrap_or(false);
    CAPTURE_SESSION_START.store(capture_session_start, Ordering::Relaxed);
    STDOUT_IS_CONTEXT.store(takes, Ordering::Relaxed);
    if let Ok(mut context) = SESSION_START_CONTEXT.lock() {
        context.clear();
    }

    let sid = input.session_id.as_deref().and_then(sanitize_id);
    if let Ok(mut slot) = HOST_SESSION_ID.write() {
        *slot = sid;
    }
    let aid = input.agent_id.as_deref().and_then(sanitize_id);
    if let Ok(mut slot) = HOST_AGENT_ID.write() {
        *slot = aid;
    }
}

/// True when `hint`/`raw` should write to stdout so the model actually sees it.
pub fn stdout_is_context() -> bool {
    STDOUT_IS_CONTEXT.load(Ordering::Relaxed)
}

/// True when SessionStart context must be buffered for structured output.
pub fn captures_session_start_context() -> bool {
    CAPTURE_SESSION_START.load(Ordering::Relaxed)
}

/// Add one context fragment to the current SessionStart response.
pub fn append_session_start_context(fragment: &str) {
    if let Ok(mut context) = SESSION_START_CONTEXT.lock() {
        if !context.is_empty() {
            context.push('\n');
        }
        context.push_str(fragment);
    }
}

/// Consume the buffered context as Codex's SessionStart output contract.
pub fn take_session_start_output() -> String {
    let context = SESSION_START_CONTEXT
        .lock()
        .map(|mut context| std::mem::take(&mut *context))
        .unwrap_or_default();
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": context,
        }
    })
    .to_string()
}

/// The host's conversation id, sanitized, or `None` when the host sent none.
pub fn session_id() -> Option<String> {
    HOST_SESSION_ID.read().ok().and_then(|g| g.clone())
}

/// The host's subagent id, sanitized, or `None` when the event has no subagent.
pub fn agent_id() -> Option<String> {
    HOST_AGENT_ID.read().ok().and_then(|g| g.clone())
}

/// Test-only serialization for the process-global state `init` writes.
///
/// Lives outside `mod tests` because `shared::mod`'s `session_id` tests observe
/// the same globals and must take the same lock.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::{Mutex, MutexGuard};

    static SERIAL: Mutex<()> = Mutex::new(());

    /// Hold for the duration of any test that calls `init` or reads its state.
    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Clear host identity so a test sees the no-host fallback.
    pub(crate) fn reset() {
        super::init(&super::HookInput::default());
    }
}

#[cfg(test)]
mod tests {
    use super::testing::lock;
    use super::*;

    fn event(name: Option<&str>) -> HookInput {
        HookInput {
            hook_event_name: name.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn claude_code_keeps_stderr() {
        let _g = lock();
        // No hook_event_name => Claude Code stdin-passthrough contract.
        init(&event(None));
        assert!(!stdout_is_context());
    }

    #[test]
    fn codex_session_start_captures_structured_context() {
        let _g = lock();
        init(&event(Some("SessionStart")));
        assert!(captures_session_start_context());
        assert!(!stdout_is_context());
    }

    #[test]
    fn codex_session_start_serializes_exact_context_as_one_json_object() {
        let _g = lock();
        init(&event(Some("SessionStart")));
        let context = "[resume] Previous: \"quoted\"\n\n## Evolved Skills\n`[markdown]`";

        append_session_start_context(context);
        let stdout = take_session_start_output();
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).expect("SessionStart stdout must be valid JSON");

        assert_eq!(
            parsed["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
        assert_eq!(
            parsed["hookSpecificOutput"]["additionalContext"], context,
            "quotes, newlines, Markdown, and a leading '[' must round-trip exactly"
        );
    }

    #[test]
    fn codex_tool_events_keep_stderr() {
        let _g = lock();
        // PostToolUse/PreToolUse discard plain stdout — writing context there
        // would silently vanish.
        for ev in ["PreToolUse", "PostToolUse"] {
            init(&event(Some(ev)));
            assert!(!stdout_is_context(), "{ev} must not use stdout");
        }
    }

    #[test]
    fn codex_json_events_keep_stderr() {
        let _g = lock();
        // These expect structured JSON; plain text would corrupt the payload.
        for ev in [
            "Stop",
            "SessionEnd",
            "PreCompact",
            "PostCompact",
            "SubagentStop",
        ] {
            init(&event(Some(ev)));
            assert!(!stdout_is_context(), "{ev} must not use stdout");
        }
    }

    #[test]
    fn unknown_event_defaults_to_stderr() {
        let _g = lock();
        init(&event(Some("SomeFutureEvent")));
        assert!(!stdout_is_context());
    }

    // ── identity ────────────────────────────────────

    #[test]
    fn host_session_id_is_kept() {
        let _g = lock();
        init(&HookInput {
            session_id: Some("019a2f30-1c4d-7000-8f11-2b3c4d5e6f70".into()),
            ..Default::default()
        });
        assert_eq!(
            session_id().as_deref(),
            Some("019a2f30-1c4d-7000-8f11-2b3c4d5e6f70")
        );
    }

    #[test]
    fn absent_host_session_id_is_none() {
        let _g = lock();
        init(&HookInput::default());
        assert!(session_id().is_none());
        assert!(agent_id().is_none());
    }

    #[test]
    fn host_agent_id_is_kept() {
        let _g = lock();
        init(&HookInput {
            agent_id: Some("builder_1".into()),
            ..Default::default()
        });
        assert_eq!(agent_id().as_deref(), Some("builder_1"));
    }

    #[test]
    fn path_separators_are_stripped_from_ids() {
        // A host id reaches the filesystem via session_{id}.jsonl and
        // resume.{id}.lock, so traversal characters must not survive.
        let _g = lock();
        init(&HookInput {
            session_id: Some("../../etc/passwd".into()),
            ..Default::default()
        });
        let sid = session_id().expect("id survives sanitization");
        assert_eq!(sid, "etcpasswd");
        assert!(!sid.contains('/') && !sid.contains('.'));
    }

    #[test]
    fn fully_rejected_id_falls_back_to_none() {
        let _g = lock();
        init(&HookInput {
            session_id: Some("///".into()),
            ..Default::default()
        });
        assert!(session_id().is_none());
    }

    #[test]
    fn oversized_id_is_truncated() {
        let _g = lock();
        init(&HookInput {
            session_id: Some("a".repeat(500)),
            ..Default::default()
        });
        assert_eq!(session_id().map(|s| s.len()), Some(MAX_ID_LEN));
    }
}
