use regex::Regex;
use std::fs;
use std::sync::LazyLock;

use super::common::*;
use crate::telemetry::{FailureClass, Telemetry, ToolCategory};

static TELEMETRY: LazyLock<Telemetry> = LazyLock::new(Telemetry::init);

// mask_secrets is now in shared/sanitize.rs — re-exported via common
use crate::hooks::common::{mask_secrets, mask_secrets_keep_paths, truncate_utf8};

/// Cap on the persisted `action` text. Enough for pattern detection, bounded so
/// a single pasted heredoc cannot write tens of kilobytes per tool call.
const MAX_ACTION_BYTES: usize = 2000;

/// Cap on the persisted error snippet.
const MAX_SNIPPET_BYTES: usize = 500;

fn persisted_action(input: &serde_json::Value) -> String {
    let raw = input
        .get("command")
        .and_then(|value| value.as_str())
        .or_else(|| input.get("file_path").and_then(|value| value.as_str()))
        .map(String::from)
        .unwrap_or_else(|| {
            let redacted = redact_json_credentials(input);
            serde_json::to_string(&redacted).expect("JSON Value serialization cannot fail")
        });
    mask_secrets_keep_paths(truncate_utf8(&raw, MAX_ACTION_BYTES))
}

fn observation_tool_use_id(input: &HookInput) -> Option<String> {
    input
        .tool_use_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

static SILENT_OK_CMDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(mkdir|cp|mv|rm|chmod|chown|ln|touch|git\s+(add|checkout|switch|branch|stash|tag|remote)|cd|export|source|tsc\s+--noEmit)\b").unwrap()
});

/// Commands whose stdout is *file content*, not a report about the command.
///
/// Keyword classification cannot tell "this command failed" from "this command
/// successfully printed a file that mentions TypeError". Reading a log, a diff or
/// a test fixture used to be scored as a failed tool call, which is where the
/// bulk of recorded failures came from. Without a structured exit status these
/// calls record `unknown` instead of inventing a failure.
///
/// Commands that *report* on work (build, test, lint, package managers) are
/// deliberately absent — their keywords are real evidence.
static READ_ONLY_CMDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(sudo\s+)?(cat|bat|nl|head|tail|less|more|sed|awk|cut|sort|uniq|wc|tr|rg|grep|egrep|fgrep|ag|ack|find|fd|ls|tree|stat|file|jq|yq|xxd|od|strings|diff|comm|echo|printf|pwd|which|type|env|date|git\s+(diff|log|show|blame|status|ls-files|cat-file|rev-parse))\b",
    )
    .unwrap()
});

/// True when the command only reads and prints existing content.
///
/// Applies to the *first* command in a pipeline or `&&` chain: `cat x | grep y`
/// is a read, but `cargo test | tail -5` is not — the leading command decides
/// what the output is evidence about.
fn is_read_only_command(command: &str) -> bool {
    !command.trim().is_empty() && READ_ONLY_CMDS.is_match(command)
}

/// True when a call's output is content it fetched, not a report about itself.
///
/// `Read`, `Grep` and `Glob` return file text by definition, so a keyword in
/// their output describes the file, not the call. Bash depends on which command
/// ran. A genuine failure of these tools is still recorded whenever the host
/// reports one — this only governs the no-evidence fallback.
fn outputs_file_content(tool_category: &str, command: &str) -> bool {
    match tool_category {
        "read" | "grep" | "glob" => true,
        "bash" => is_read_only_command(command),
        _ => false,
    }
}

/// Explicit success/failure the host reported, if the payload carries one.
///
/// Returns `Some(true)` for a reported success, `Some(false)` for a reported
/// failure, `None` when the host gave no structured signal. A structured signal
/// is authoritative in *both* directions: it must be able to clear a false
/// keyword match, not only create a failure.
///
/// Hosts disagree on the field name, so all the known spellings are accepted.
/// Codex currently sends none of them for Bash, which is why the `None` path
/// still has to be careful.
fn reported_outcome(v: &serde_json::Value) -> Option<bool> {
    let obj = v.as_object()?;

    for key in ["exit_code", "exitCode", "returncode", "returnCode"] {
        if let Some(code) = obj.get(key).and_then(|c| c.as_i64()) {
            return Some(code == 0);
        }
    }
    for key in ["is_error", "isError", "error"] {
        if let Some(flag) = obj.get(key).and_then(|c| c.as_bool()) {
            return Some(!flag);
        }
    }
    for key in ["success", "ok"] {
        if let Some(flag) = obj.get(key).and_then(|c| c.as_bool()) {
            return Some(flag);
        }
    }
    match obj.get("status").and_then(|s| s.as_str()) {
        Some("success") | Some("ok") | Some("completed") => Some(true),
        Some("error") | Some("failed") | Some("failure") => Some(false),
        _ => None,
    }
}

/// The recorded outcome of a tool call.
pub(crate) struct Outcome {
    /// `"success"`, `"error"` or `"unknown"`.
    pub result: &'static str,
    /// Failure category, set only when `result` is `"error"`.
    pub failure: Option<&'static str>,
}

/// Decide a tool call's outcome from the strongest available evidence.
///
/// Precedence:
/// 1. A structured status from the host — authoritative both ways.
/// 2. With no structured status, a call whose output is fetched content
///    (`outputs_file_content`) yields `unknown` on a keyword match rather than a
///    fabricated failure. This is the case that produced most recorded failures.
/// 3. Otherwise text can prove a failure, but clean text cannot prove success.
///
/// A reported failure still uses the text to pick a category, falling back to
/// `runtime_error` when the text names nothing recognizable.
pub(crate) fn decide_outcome(
    reported: Option<bool>,
    text_failure: Option<&'static str>,
    tool_category: &str,
    command: &str,
) -> Outcome {
    match reported {
        Some(true) => Outcome {
            result: "success",
            failure: None,
        },
        Some(false) => Outcome {
            result: "error",
            failure: Some(text_failure.unwrap_or("runtime_error")),
        },
        None => match text_failure {
            None => Outcome {
                result: "unknown",
                failure: None,
            },
            Some(_) if outputs_file_content(tool_category, command) => Outcome {
                result: "unknown",
                failure: None,
            },
            Some(cat) => Outcome {
                result: "error",
                failure: Some(cat),
            },
        },
    }
}

fn get_next_sequence_id(session_file: &std::path::Path) -> u64 {
    std::fs::metadata(session_file)
        .map(|m| m.len())
        .unwrap_or(0)
}

fn get_last_action(session_file: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(session_file).ok()?;
    let file_len = f.seek(SeekFrom::End(0)).ok()?;
    let start = file_len.saturating_sub(1024);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((file_len - start) as usize + 1);
    f.read_to_end(&mut buf).ok()?;
    let tail = String::from_utf8_lossy(&buf);
    let last_line = tail.lines().rfind(|l| !l.is_empty())?;
    let rec: ObsRecord = serde_json::from_str(last_line).ok()?;
    rec.action
}

fn failure_quality(failure_category: Option<&str>) -> f64 {
    match failure_category {
        None => 1.0, // no failure means success — callers gate on .is_some()
        Some("syntax_error") => 0.3,
        Some("type_error") => 0.4,
        Some("runtime_error") => 0.5,
        Some("test_fail") => 0.6,
        Some("permission_denied") => 0.7,
        Some("build_fail") => 0.5,
        Some("lint_fail") => 0.6,
        Some("timeout") => 0.5,
        Some("not_found") => 0.6,
        Some(_) => 0.5, // unknown failure category
    }
}

/// Score a Bash call. `failure` is the outcome already decided by the caller —
/// the scorers do not re-classify, so text and structured evidence cannot
/// disagree between the recorded result and the recorded dimensions.
fn score_bash(output: &str, command: &str, failure: Option<&'static str>) -> ScoreDimensions {
    let tool_success = if failure.is_none() { 1.0 } else { 0.0 };

    let is_empty = output.trim().is_empty();
    let mut quality: f64 = if failure.is_some() {
        failure_quality(failure)
    } else if is_empty && SILENT_OK_CMDS.is_match(command) {
        1.0
    } else if is_empty {
        0.7
    } else {
        1.0
    };
    // Warning penalty only applies to successful calls
    if failure.is_none() {
        static WARN_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)\bwarning\b|\bWARN\b").unwrap());
        static DEPREC_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)\bWARN(ING)?\b.*deprecat").unwrap());
        if WARN_RE.is_match(output) && !DEPREC_RE.is_match(output) {
            quality = (quality - 0.3).max(0.0);
        }
    }

    let len = output.len();
    let cost = if len > 50000 {
        0.3
    } else if len > 20000 {
        0.6
    } else {
        1.0
    };

    ScoreDimensions {
        tool_success,
        output_quality: quality,
        execution_cost: cost,
    }
}

fn score_edit(
    output: &str,
    prev_action: Option<&str>,
    curr_action: Option<&str>,
    failure: Option<&'static str>,
) -> ScoreDimensions {
    let tool_success = if failure.is_none() { 1.0 } else { 0.0 };

    let quality = if failure.is_some() {
        failure_quality(failure)
    } else {
        let mut q: f64 = 1.0;
        static NO_CHANGE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)no changes|file not found").unwrap());
        if NO_CHANGE_RE.is_match(output) {
            q = 0.3;
        }
        if let (Some(prev), Some(curr)) = (prev_action, curr_action)
            && prev == curr
        {
            q = q.min(0.7);
        }
        q
    };

    ScoreDimensions {
        tool_success,
        output_quality: quality,
        execution_cost: 1.0,
    }
}

fn score_write(_output: &str, failure: Option<&'static str>) -> ScoreDimensions {
    let ok = failure.is_none();
    ScoreDimensions {
        tool_success: if ok { 1.0 } else { 0.0 },
        output_quality: if ok { 1.0 } else { failure_quality(failure) },
        execution_cost: 1.0,
    }
}

fn score_read_search(output: &str, failure: Option<&'static str>) -> ScoreDimensions {
    static NO_MATCH_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)no matches|0 results").unwrap());
    let has_results =
        failure.is_none() && !output.trim().is_empty() && !NO_MATCH_RE.is_match(output);
    ScoreDimensions {
        tool_success: if has_results { 1.0 } else { 0.0 },
        output_quality: if has_results {
            1.0
        } else if failure.is_some() {
            failure_quality(failure)
        } else {
            0.5
        },
        execution_cost: 1.0,
    }
}

/// Core counter logic — extracted for testability.
/// Returns `true` if the event should be sent (count was below the cap),
/// `false` if the cap has been reached. Atomically increments the counter file.
///
/// Counter file is per-session (includes PID in the filename via `session_id()`),
/// so concurrent sessions never share the same counter file. Within a single
/// session/process, hook invocations are sequential, making this read-write safe.
fn check_and_increment_counter(counter_file: &std::path::Path) -> bool {
    const MAX_TOOL_ERRORS: u32 = 50;
    let count: u32 = fs::read_to_string(counter_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    if count >= MAX_TOOL_ERRORS {
        return false;
    }
    let _ = fs::write(counter_file, (count + 1).to_string());
    true
}

/// Returns `true` if this `tool_error` telemetry event should be sent.
///
/// `obs_dir()` is guaranteed to exist at this call site because `run()`
/// returns early via `harness_exists()` before reaching the telemetry block.
fn should_sample_tool_error() -> bool {
    let counter_file = obs_dir().join(format!("telemetry_error_count_{}.txt", session_id()));
    check_and_increment_counter(&counter_file)
}

/// Check whether an agent has exceeded the timeout threshold.
///
/// Only active when `EPIC_ORCHESTRATION=enabled`. Reads the agent's
/// `status.json` from the orchestrator state directory and compares
/// elapsed time since `started_at` against `AGENT_TIMEOUT_SECS`.
///
/// Returns `Some(warning_message)` if the agent is overdue, `None` otherwise.
fn check_agent_timeout(agent_id: &str) -> Option<String> {
    let orch_dir = harness_dir().join("orchestrator").join("agents");
    check_agent_timeout_with_dir(agent_id, &orch_dir)
}

/// True when this hook fire concerns a subagent, on either host.
///
/// Claude Code spawns subagents through the `Agent` tool; Codex reports its
/// native subagents through `SubagentStart`/`SubagentStop` events that carry no
/// `tool_name` at all, so a tool-name-only check saw none of them.
fn is_agent_event(input: &HookInput) -> bool {
    if input.tool_name.as_deref().unwrap_or("").to_lowercase() == "agent" {
        return true;
    }
    matches!(
        input.hook_event_name.as_deref(),
        Some("SubagentStart") | Some("SubagentStop")
    )
}

/// Track agent spawn/completion via the orchestrate module.
///
/// Called on every subagent event in the observe hook.
/// - start (`SubagentStart`, or an `Agent` PreToolUse) → record "running" state
/// - stop (`SubagentStop`, or an `Agent` PostToolUse) → parse output, record final state
///
/// The event name decides when the host sends one: a `SubagentStop` carrying no
/// payload would otherwise be misread as a spawn and reset the agent to running.
fn track_agent_spawn(input: &HookInput) -> Result<(), Box<dyn std::error::Error>> {
    use crate::orchestrate;

    let is_completion = match input.hook_event_name.as_deref() {
        Some("SubagentStart") => false,
        Some("SubagentStop") => true,
        _ => {
            input.tool_output.is_some()
                || input.tool_response.is_some()
                || input.tool_result.is_some()
        }
    };

    if is_completion {
        orchestrate::run_post_checked(input)?;
    } else {
        orchestrate::run_pre_checked(input)?;
    }
    Ok(())
}

/// Testable variant that accepts an explicit orchestrator directory.
fn check_agent_timeout_with_dir(agent_id: &str, orch_dir: &std::path::Path) -> Option<String> {
    if std::env::var("EPIC_ORCHESTRATION").as_deref() != Ok("enabled") {
        return None;
    }

    let status_path = orch_dir.join(agent_id).join("status.json");
    let content = fs::read_to_string(&status_path).ok()?;
    let status: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Only check running agents
    let agent_status = status.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if agent_status != "running" {
        return None;
    }

    let started_at = status.get("started_at").and_then(|v| {
        // Support both epoch seconds (u64) and ISO-8601 string
        v.as_u64().or_else(|| {
            v.as_str().and_then(|s| {
                // Parse ISO-8601: "2026-05-07T10:00:00Z" -> epoch seconds
                // Minimal parser: extract YYYY-MM-DDTHH:MM:SS
                let digits: Vec<u64> = s
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|p| !p.is_empty())
                    .filter_map(|p| p.parse().ok())
                    .collect();
                if digits.len() < 6 {
                    return None;
                }
                let (y, mo, d, h, mi, s_) = (
                    digits[0], digits[1], digits[2], digits[3], digits[4], digits[5],
                );
                // Simple UTC epoch approximation (no leap seconds, good enough for timeout)
                let days: u64 = y * 365 + (y / 4) - (y / 100)
                    + (y / 400)
                    + if mo <= 2 {
                        (mo + 9) * 153 + 2
                    } else {
                        (mo - 3) * 153 + 2
                    } / 5
                    + d
                    - 719469;
                Some(days * 86400 + h * 3600 + mi * 60 + s_)
            })
        })
    })?;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let elapsed = now_secs.saturating_sub(started_at);
    if elapsed > AGENT_TIMEOUT_SECS {
        let elapsed_min = elapsed / 60;
        let threshold_min = AGENT_TIMEOUT_SECS / 60;
        Some(format!(
            "\u{26a0} Agent {} has been running for {} minutes (timeout: {} min)",
            agent_id, elapsed_min, threshold_min
        ))
    } else {
        None
    }
}

/// Generate concrete, fact-based investigation hints after Edit or Write tool usage.
///
/// Instead of generic "are you sure?" prompts, outputs structured questions that
/// force verification of the change's downstream impact. Only activates for
/// Edit and Write tools, and only when `GATEGUARD_HINTS` is enabled.
///
/// Uses static string slices exclusively to avoid heap allocations for hint text.
pub fn generate_investigation_hints(tool_name: &str, action: Option<&str>) {
    if !crate::config::CONFIG.hook.gateguard_hints {
        return;
    }

    let tool_lower = tool_name.to_lowercase();
    if tool_lower != "edit" && tool_lower != "write" {
        return;
    }

    // Extract file path from action string (e.g. "/src/main.rs")
    let file_path = action.and_then(|a| {
        static FILE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(/[\w./-]+\.\w+)").unwrap());
        FILE_RE.find(a).map(|m| m.as_str())
    });

    let ext = file_path.and_then(|p| p.rsplit('.').next()).unwrap_or("");

    let hints: &[&str] = match ext {
        "rs" => &[
            "List files importing this module (grep 'use' / 'mod' declarations)",
            "Run cargo check after this change to verify compilation",
            "Check if public API signatures changed and update call sites",
        ],
        "ts" | "tsx" => &[
            "Check what imports this module (grep import/require statements)",
            "Verify type compatibility — run tsc --noEmit",
            "Confirm exported types match consumer expectations",
        ],
        "go" => &[
            "Check interface implementations for signature changes",
            "Run go vet and go build to verify compilation",
            "Verify all callers of changed functions compile",
        ],
        "md" => &[
            "Verify links and cross-references in the document",
            "Check if referenced sections or headings still exist",
        ],
        _ => &[
            "Identify files importing or depending on this file",
            "Verify the change doesn't break existing tests",
        ],
    };

    for h in hints {
        hint("gateguard", h);
    }
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_OBSERVE) {
        return 0;
    }
    if !harness_exists() {
        return 0;
    }
    ensure_dir(&obs_dir());

    // Codex's `SubagentStart`/`SubagentStop` carry no tool at all. They are
    // lifecycle events, not tool calls: recording them as observations would add
    // `unknown`-tool rows with no outcome to every statistic.
    if input.tool_name.is_none() && is_agent_event(input) {
        return match track_agent_spawn(input) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("[harness] native agent tracking failed: {error}");
                1
            }
        };
    }

    let sid = session_id();

    // Open harness DB pool for SQLite writes (fallback to JSONL if DB unavailable)
    let db = crate::store::runtime::block_on(crate::store::pool::harness_pool()).ok();
    let session_file = obs_dir().join(format!("session_{}.jsonl", sid));
    let tool_cat = classify_tool(input.tool_name.as_deref().unwrap_or(""));

    // The action is persisted, so it is masked and capped first. Commands were
    // previously stored verbatim and unbounded — one recorded command reached
    // ~90 KB, and marker scans found authorization headers and key-shaped
    // strings in the stored text. Paths survive masking: they are what
    // file-level pattern detection keys on.
    let action = input.tool_input.as_ref().map(persisted_action);

    let file_ext = input.tool_input.as_ref().and_then(extract_file_ext);
    let seq_id = if db.is_none() {
        get_next_sequence_id(&session_file)
    } else {
        0
    };

    let mut record = ObsRecord {
        timestamp: now_iso(),
        tool: input.tool_name.clone().unwrap_or_else(|| "unknown".into()),
        tool_category: tool_cat.to_string(),
        action: action.clone(),
        result: None,
        score: None,
        dimensions: None,
        failure_category: None,
        error_snippet: None,
        file_ext,
        sequence_id: Some(seq_id),
        tool_use_id: observation_tool_use_id(input),
        pipeline_id: super::common::detect_active_orbit_id(),
    };

    // Resolve tool output: tool_output (structured) → tool_response (Claude Code canonical) → tool_result (legacy)
    // `stdout` is Claude Code's field name for a Bash response; `output` is the
    // legacy structured shape. Reading only `output` silently dropped every
    // object-shaped Bash result.
    let resolve_json_value = |v: &serde_json::Value| -> (String, String) {
        match v {
            serde_json::Value::String(s) => (s.clone(), String::new()),
            serde_json::Value::Object(obj) => {
                let text = |key: &str| obj.get(key).and_then(|v| v.as_str()).unwrap_or("");
                let out = match text("output") {
                    "" => text("stdout"),
                    o => o,
                };
                (out.to_string(), text("stderr").to_string())
            }
            other => (other.to_string(), String::new()),
        }
    };
    // Structured status, when the host sends one, outranks the output text.
    let reported = input
        .tool_response
        .as_ref()
        .or(input.tool_result.as_ref())
        .and_then(reported_outcome);

    let resolved_output: Option<(String, String)> = if let Some(to) = &input.tool_output {
        let out = to.output.as_deref().unwrap_or("").to_string();
        let err = to.stderr.as_deref().unwrap_or("").to_string();
        Some((out, err))
    } else if let Some(tr) = &input.tool_response {
        Some(resolve_json_value(tr))
    } else {
        input.tool_result.as_ref().map(resolve_json_value)
    };

    if let Some((output, stderr)) = resolved_output {
        let combined = format!("{output}\n{stderr}");
        let command = input
            .tool_input
            .as_ref()
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let text_failure = classify_failure(&combined);
        let outcome = decide_outcome(reported, text_failure, tool_cat, command);

        record.failure_category = outcome.failure.map(String::from);
        record.result = Some(outcome.result.into());

        // An undetermined outcome is not scored. A neutral score would be an
        // invented number, and a zero would be the false failure we are removing.
        if outcome.result != "unknown" {
            let failure = outcome.failure;
            let dims = match tool_cat {
                "bash" => score_bash(&combined, command, failure),
                "edit" => {
                    let prev = db
                        .as_ref()
                        .and_then(|pool| {
                            crate::store::runtime::block_on(
                                crate::store::observations::query_last_action_pool(pool, &sid),
                            )
                            .ok()
                            .flatten()
                        })
                        .or_else(|| get_last_action(&session_file));
                    score_edit(&combined, prev.as_deref(), action.as_deref(), failure)
                }
                "write" => score_write(&combined, failure),
                "read" => ScoreDimensions {
                    tool_success: if failure.is_none() { 1.0 } else { 0.0 },
                    output_quality: if failure.is_none() {
                        1.0
                    } else {
                        failure_quality(failure)
                    },
                    execution_cost: 1.0,
                },
                "glob" | "grep" => score_read_search(&combined, failure),
                _ => ScoreDimensions {
                    tool_success: if failure.is_none() { 1.0 } else { 0.0 },
                    output_quality: failure_quality(failure),
                    execution_cost: 1.0,
                },
            };

            record.dimensions = Some(dims);
            record.score = Some(compute_score(&dims));
        }

        if record.failure_category.is_some() {
            record.error_snippet = Some(mask_secrets(truncate_utf8(&combined, MAX_SNIPPET_BYTES)));
        }
    }

    // Storage policy: SQLite primary, JSONL fallback on transient write failure.
    // Rationale: observe runs on every tool use; a single DB write failure must not
    // drop the observation. The JSONL file acts as a circuit-breaker buffer.
    // Reflection merges this exact session's fallback file with its database
    // rows using sequence/provenance identity.
    if let Some(ref pool) = db {
        if let Err(e) =
            crate::store::runtime::block_on(crate::store::observations::insert_observation_pool(
                pool,
                &record,
                &sid,
                &crate::shared::paths::project_slug(),
            ))
        {
            eprintln!("[observe] SQLite write failed, falling back to JSONL: {e}");
            append_jsonl(&session_file, &record);
        }
    } else {
        append_jsonl(&session_file, &record);
    }

    // Fire tool_error telemetry only on failures to keep event volume low.
    // Capped at 50 events per session to avoid flooding PostHog during error loops.
    if let Some(failure_cat) = &record.failure_category {
        let tool_cat = &record.tool_category;
        if should_sample_tool_error() {
            TELEMETRY.track_tool_error(
                tool_cat.parse().unwrap_or(ToolCategory::Other),
                failure_cat.parse().unwrap_or(FailureClass::Unknown),
            );
        }
    }

    // GateGuard: emit concrete investigation hints for Edit/Write to force
    // fact-based verification instead of generic "are you sure?" prompts.
    generate_investigation_hints(input.tool_name.as_deref().unwrap_or(""), action.as_deref());

    // Agent tracking: record spawn/completion to orchestrator state for
    // dashboard display. Always active — no EPIC_ORCHESTRATION gate.
    if is_agent_event(input) {
        if let Err(error) = track_agent_spawn(input) {
            eprintln!("[harness] agent tracking failed: {error}");
            return 1;
        }

        // Timeout detection still gated by EPIC_ORCHESTRATION
        if let Some(agent_id) = crate::shared::host::agent_id().or_else(|| {
            input
                .tool_input
                .as_ref()
                .and_then(|v| v.get("agent_id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        }) && let Some(timeout_msg) = check_agent_timeout(&agent_id)
        {
            hint("agent-timeout", &timeout_msg);
        }
    }

    if let Err(error) = crate::orchestrate::record_native_agent_tool(input) {
        eprintln!("[harness] native agent tool tracking failed: {error}");
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // The scorers take the decided failure category as a parameter so the
    // recorded result and the recorded dimensions cannot disagree. These shims
    // derive it from the text, which is what the scoring tests below exercise —
    // the decision itself is covered by the `decide_outcome` tests.
    fn score_bash(output: &str, command: &str) -> ScoreDimensions {
        super::score_bash(output, command, classify_failure(output))
    }

    fn score_edit(
        output: &str,
        prev_action: Option<&str>,
        curr_action: Option<&str>,
    ) -> ScoreDimensions {
        super::score_edit(output, prev_action, curr_action, classify_failure(output))
    }

    fn score_write(output: &str) -> ScoreDimensions {
        super::score_write(output, classify_failure(output))
    }

    fn score_read_search(output: &str) -> ScoreDimensions {
        super::score_read_search(output, classify_failure(output))
    }

    // ── score_bash ──────────────────────────────────
    #[test]
    fn bash_success_full_score() {
        let dims = score_bash("all tests passed", "npm test");
        assert_eq!(dims.tool_success, 1.0);
        assert_eq!(dims.output_quality, 1.0);
    }

    #[test]
    fn bash_error_zero_success() {
        let dims = score_bash("TypeError: x is not a function", "node main.js");
        assert_eq!(dims.tool_success, 0.0);
    }

    #[test]
    fn bash_empty_output_silent_ok() {
        let dims = score_bash("", "mkdir -p /tmp/test");
        assert_eq!(dims.output_quality, 1.0);
    }

    #[test]
    fn bash_empty_output_not_silent_ok() {
        let dims = score_bash("", "echo hello");
        assert_eq!(dims.output_quality, 0.7);
    }

    #[test]
    fn bash_warning_reduces_quality() {
        let dims = score_bash("warning: unused variable", "cargo build");
        assert!(dims.output_quality < 1.0);
    }

    #[test]
    fn score_bash_no_warnings_found_not_penalized() {
        let dims = score_bash("No warnings found", "cargo check");
        assert_eq!(
            dims.output_quality, 1.0,
            "substring 'warning' in negative phrase must not penalize"
        );
    }

    #[test]
    fn bash_large_output_reduces_cost() {
        let large = "x".repeat(60000);
        let dims = score_bash(&large, "cat bigfile");
        assert_eq!(dims.execution_cost, 0.3);
    }

    #[test]
    fn bash_medium_output_mid_cost() {
        let medium = "x".repeat(30000);
        let dims = score_bash(&medium, "cat medfile");
        assert_eq!(dims.execution_cost, 0.6);
    }

    // ── score_edit ──────────────────────────────────
    #[test]
    fn edit_success() {
        let dims = score_edit("file updated", None, None);
        assert_eq!(dims.tool_success, 1.0);
        assert_eq!(dims.output_quality, 1.0);
    }

    #[test]
    fn edit_no_changes() {
        let dims = score_edit("no changes made", None, None);
        assert_eq!(dims.output_quality, 0.3);
    }

    #[test]
    fn edit_repeated_action_reduces_quality() {
        let dims = score_edit("file updated", Some("/src/main.rs"), Some("/src/main.rs"));
        assert_eq!(dims.output_quality, 0.7);
    }

    #[test]
    fn edit_different_actions_full_quality() {
        let dims = score_edit("file updated", Some("/src/main.rs"), Some("/src/lib.rs"));
        assert_eq!(dims.output_quality, 1.0);
    }

    // ── decide_outcome ──────────────────────────────
    // The reported regression: reading a file whose *content* mentions a failure
    // was scored as a failed tool call. 66% of classified errors came from
    // read-oriented commands.

    #[test]
    fn reading_a_file_containing_failure_words_is_not_a_failure() {
        for cmd in [
            "cat build.log",
            "sed -n '1,40p' src/main.rs",
            "rg 'TypeError' src/",
            "nl notes.md",
            "git diff HEAD~1",
            "find . -name '*.rs'",
            "  sudo tail -n 200 /var/log/syslog",
        ] {
            let o = decide_outcome(None, Some("type_error"), "bash", cmd);
            assert_eq!(o.result, "unknown", "{cmd} must not be scored as a failure");
            assert!(o.failure.is_none(), "{cmd} must record no failure category");
        }
    }

    #[test]
    fn a_reporting_command_still_fails_on_its_own_keywords() {
        // These commands report on work they performed; their keywords are real
        // evidence and must keep producing a failure.
        for cmd in ["cargo test", "npm run build", "pytest -q", "node main.js"] {
            let o = decide_outcome(None, Some("test_fail"), "bash", cmd);
            assert_eq!(o.result, "error", "{cmd} must stay a failure");
            assert_eq!(o.failure, Some("test_fail"));
        }
    }

    #[test]
    fn a_pipeline_is_judged_by_its_leading_command() {
        // `cat x | grep y` is a read; `cargo test | tail -5` is not.
        assert_eq!(
            decide_outcome(None, Some("not_found"), "bash", "cat a.txt | grep foo").result,
            "unknown"
        );
        assert_eq!(
            decide_outcome(None, Some("test_fail"), "bash", "cargo test | tail -5").result,
            "error"
        );
    }

    #[test]
    fn a_reported_exit_code_outranks_the_text() {
        // Both directions: a non-zero exit makes a read a real failure, and a
        // zero exit clears a false keyword match.
        let failed = decide_outcome(Some(false), None, "bash", "cat missing.txt");
        assert_eq!(failed.result, "error");
        assert_eq!(
            failed.failure,
            Some("runtime_error"),
            "a reported failure with unrecognizable text still needs a category"
        );

        let ok = decide_outcome(Some(true), Some("type_error"), "bash", "cargo test");
        assert_eq!(ok.result, "success");
        assert!(ok.failure.is_none());
    }

    #[test]
    fn a_reported_failure_keeps_the_text_category() {
        let o = decide_outcome(Some(false), Some("build_fail"), "bash", "make");
        assert_eq!(o.result, "error");
        assert_eq!(o.failure, Some("build_fail"));
    }

    #[test]
    fn clean_output_without_structured_status_is_unknown() {
        assert_eq!(
            decide_outcome(None, None, "bash", "cat a.txt").result,
            "unknown"
        );
    }

    #[test]
    fn generic_persisted_action_redacts_nested_credentials() {
        let input = serde_json::json!({
            "request": [{"github_token": "ghp_nested", "query": "visible"}]
        });
        let action = persisted_action(&input);
        assert!(!action.contains("ghp_nested"));
        assert!(action.contains("<REDACTED>"));
        assert!(action.contains("visible"));
    }

    #[test]
    fn stable_tool_use_id_is_preserved_for_cross_store_deduplication() {
        let input = HookInput {
            tool_use_id: Some("call-123".into()),
            ..HookInput::default()
        };
        assert_eq!(observation_tool_use_id(&input), Some("call-123".into()));

        let blank = HookInput {
            tool_use_id: Some("  ".into()),
            ..HookInput::default()
        };
        assert_eq!(observation_tool_use_id(&blank), None);
    }

    #[test]
    fn content_returning_tools_get_the_exemption_too() {
        // Read/Grep/Glob return file text by definition, so a keyword in their
        // output describes the file, not the call.
        for cat in ["read", "grep", "glob"] {
            let o = decide_outcome(None, Some("type_error"), cat, "");
            assert_eq!(o.result, "unknown", "{cat} must not invent a failure");
        }
    }

    #[test]
    fn tools_that_report_on_their_own_work_do_not_get_the_exemption() {
        for cat in ["edit", "write", "other"] {
            let o = decide_outcome(None, Some("permission_denied"), cat, "");
            assert_eq!(o.result, "error", "{cat} must stay a failure");
        }
    }

    #[test]
    fn a_reported_status_still_wins_for_content_tools() {
        // The exemption only governs the no-evidence fallback; a host that
        // reports a real Read failure is still believed.
        assert_eq!(
            decide_outcome(Some(false), Some("not_found"), "read", "").result,
            "error"
        );
    }

    // ── subagent events ─────────────────────────────
    #[test]
    fn codex_subagent_events_are_agent_events() {
        for ev in ["SubagentStart", "SubagentStop"] {
            let input = HookInput {
                hook_event_name: Some(ev.into()),
                ..Default::default()
            };
            assert!(is_agent_event(&input), "{ev} must be tracked");
            assert!(
                input.tool_name.is_none(),
                "{ev} carries no tool, so it must not become an observation"
            );
        }
    }

    #[test]
    fn the_claude_agent_tool_is_still_an_agent_event() {
        let input = HookInput {
            tool_name: Some("Agent".into()),
            ..Default::default()
        };
        assert!(is_agent_event(&input));
    }

    #[test]
    fn ordinary_tool_calls_are_not_agent_events() {
        for (tool, event) in [("Bash", "PostToolUse"), ("Edit", "PreToolUse")] {
            let input = HookInput {
                tool_name: Some(tool.into()),
                hook_event_name: Some(event.into()),
                ..Default::default()
            };
            assert!(!is_agent_event(&input), "{tool} must not be tracked");
        }
    }

    // ── reported_outcome ────────────────────────────
    #[test]
    fn structured_status_is_read_from_each_known_spelling() {
        let cases = [
            (serde_json::json!({"exit_code": 0}), Some(true)),
            (serde_json::json!({"exit_code": 1}), Some(false)),
            (serde_json::json!({"exitCode": 2}), Some(false)),
            (serde_json::json!({"is_error": true}), Some(false)),
            (serde_json::json!({"isError": false}), Some(true)),
            (serde_json::json!({"success": true}), Some(true)),
            (serde_json::json!({"status": "failed"}), Some(false)),
            (serde_json::json!({"status": "completed"}), Some(true)),
            (serde_json::json!({"status": "running"}), None),
            (serde_json::json!({"stdout": "hi"}), None),
            (serde_json::json!("plain string"), None),
        ];
        for (payload, want) in cases {
            assert_eq!(reported_outcome(&payload), want, "payload {payload}");
        }
    }

    // ── score_write ─────────────────────────────────
    #[test]
    fn write_success() {
        let dims = score_write("file created");
        assert_eq!(dims.tool_success, 1.0);
        assert_eq!(dims.execution_cost, 1.0);
    }

    #[test]
    fn write_error() {
        let dims = score_write("EACCES: permission denied");
        assert_eq!(dims.tool_success, 0.0);
    }

    // ── score_read_search ───────────────────────────
    #[test]
    fn read_with_results() {
        let dims = score_read_search("found: main.rs");
        assert_eq!(dims.tool_success, 1.0);
    }

    #[test]
    fn read_no_results() {
        let dims = score_read_search("0 results found");
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(dims.output_quality, 0.5);
    }

    #[test]
    fn read_empty_output() {
        let dims = score_read_search("");
        assert_eq!(dims.tool_success, 0.0);
    }

    // ── get_next_sequence_id ────────────────────────
    #[test]
    fn sequence_id_zero_for_missing_file() {
        let path = std::path::Path::new("/tmp/epic_harness_nonexistent_file_xyzzy.jsonl");
        assert_eq!(get_next_sequence_id(path), 0);
    }

    #[test]
    fn sequence_id_increases_with_content() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("epic_harness_seq_test.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        let id_empty = get_next_sequence_id(&path);
        f.write_all(b"{\"a\":1}\n").unwrap();
        f.flush().unwrap();
        let id_after = get_next_sequence_id(&path);
        assert!(id_after > id_empty);
        let _ = std::fs::remove_file(&path);
    }

    // ── compute_score integration ───────────────────
    #[test]
    fn score_bash_perfect_run() {
        let dims = score_bash("tests passed", "git add .");
        let score = compute_score(&dims);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn score_bash_failure() {
        let dims = score_bash("SyntaxError: unexpected token", "node broken.js");
        let score = compute_score(&dims);
        assert!(score <= 0.5);
        assert_eq!(dims.tool_success, 0.0);
    }

    // ── mask_secrets ────────────────────────────────
    #[test]
    fn test_mask_bearer_token() {
        let input = r#"curl -H "Authorization: Bearer sk-abc123XYZ" failed"#;
        let output = mask_secrets(input);
        assert!(
            !output.contains("sk-abc123XYZ"),
            "Bearer token must be redacted"
        );
        assert!(
            output.contains("Authorization: <REDACTED>"),
            "must have redacted placeholder"
        );
    }

    #[test]
    fn test_mask_sk_key() {
        let input = "Error: invalid key sk-proj-abcDEF12345678 supplied";
        let output = mask_secrets(input);
        assert!(
            !output.contains("sk-proj-abcDEF12345678"),
            "sk- key must be redacted"
        );
        assert!(output.contains("sk-<REDACTED>"), "must have sk-<REDACTED>");
    }

    #[test]
    fn test_mask_password_equals() {
        let input = "connection failed: password=s3cr3tP@ss! reason=timeout";
        let output = mask_secrets(input);
        assert!(
            !output.contains("s3cr3tP@ss!"),
            "password value must be redacted"
        );
        assert!(
            output.contains("<REDACTED>"),
            "must have redacted placeholder"
        );
    }

    #[test]
    fn test_mask_safe_text_unchanged() {
        let input = "all tests passed in 42ms, no errors found";
        let output = mask_secrets(input);
        assert_eq!(output, input, "safe text must not be modified");
    }

    // ── tool_result resolution ──────────────────────────
    #[test]
    fn tool_result_string_scores() {
        // Simulate Claude Code PostToolUse payload with tool_result as string
        let input = HookInput {
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "echo hello"})),
            tool_output: None,
            tool_result: Some(serde_json::json!("hello\n")),
            ..Default::default()
        };
        let output = input
            .tool_result
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn tool_result_object_scores() {
        let input = HookInput {
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "ls"})),
            tool_output: None,
            tool_result: Some(serde_json::json!({"output": "file.txt\n", "stderr": ""})),
            ..Default::default()
        };
        if let Some(serde_json::Value::Object(obj)) = &input.tool_result {
            let out = obj.get("output").and_then(|v| v.as_str()).unwrap_or("");
            assert_eq!(out, "file.txt\n");
        } else {
            panic!("expected object");
        }
    }

    // ── should_sample_tool_error / check_and_increment_counter ─────────────
    #[test]
    fn counter_allows_up_to_50_then_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let counter_file = dir.path().join("telemetry_error_count_test.txt");
        // First 50 calls must return true
        for i in 0..50 {
            let result = check_and_increment_counter(&counter_file);
            assert!(result, "call {} should return true", i + 1);
        }
        // 51st call must return false
        let result = check_and_increment_counter(&counter_file);
        assert!(!result, "51st call should return false");
    }

    #[test]
    fn counter_file_written_and_read_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let counter_file = dir.path().join("telemetry_error_count_rw.txt");
        // No file yet — first call returns true and writes "1"
        assert!(check_and_increment_counter(&counter_file));
        let content = fs::read_to_string(&counter_file).unwrap();
        assert_eq!(content.trim(), "1");
        // Second call returns true and writes "2"
        assert!(check_and_increment_counter(&counter_file));
        let content = fs::read_to_string(&counter_file).unwrap();
        assert_eq!(content.trim(), "2");
    }

    #[test]
    fn counter_treats_missing_file_as_zero() {
        let dir = tempfile::tempdir().unwrap();
        let counter_file = dir.path().join("telemetry_error_count_missing.txt");
        // File does not exist; should treat count as 0 and allow the call
        assert!(check_and_increment_counter(&counter_file));
    }

    #[test]
    fn counter_treats_corrupt_file_as_zero() {
        let dir = tempfile::tempdir().unwrap();
        let counter_file = dir.path().join("telemetry_error_count_corrupt.txt");
        fs::write(&counter_file, b"not_a_number").unwrap();
        // Parse error -> default 0 -> should allow the call
        assert!(check_and_increment_counter(&counter_file));
        let content = fs::read_to_string(&counter_file).unwrap();
        assert_eq!(content.trim(), "1");
    }

    // ── generate_investigation_hints ────────────────
    #[test]
    fn hints_for_edit_tool() {
        // Should not panic and should produce output for Edit with .rs file
        generate_investigation_hints("Edit", Some("/src/main.rs"));
    }

    #[test]
    fn hints_for_write_tool() {
        // Should not panic and should produce output for Write with .ts file
        generate_investigation_hints("Write", Some("/src/index.ts"));
    }

    #[test]
    fn hints_skip_non_edit_write_tools() {
        // Bash, Read, Glob etc. should produce no output (no panic, no hints)
        generate_investigation_hints("Bash", Some("cargo build"));
        generate_investigation_hints("Read", Some("/src/main.rs"));
        generate_investigation_hints("Glob", None);
        generate_investigation_hints("Grep", None);
        generate_investigation_hints("Agent", None);
    }

    #[test]
    fn hints_case_insensitive_tool_name() {
        // Tool names may come in various casings
        generate_investigation_hints("edit", Some("/src/lib.rs"));
        generate_investigation_hints("WRITE", Some("/src/lib.ts"));
        generate_investigation_hints("EdIt", Some("/src/lib.go"));
    }

    #[test]
    fn hints_for_rs_files() {
        // .rs files should mention cargo check
        generate_investigation_hints("Edit", Some("/project/src/hooks/observe.rs"));
    }

    #[test]
    fn hints_for_ts_files() {
        generate_investigation_hints("Write", Some("/project/src/index.ts"));
    }

    #[test]
    fn hints_for_tsx_files() {
        generate_investigation_hints("Edit", Some("/project/src/App.tsx"));
    }

    #[test]
    fn hints_for_go_files() {
        generate_investigation_hints("Write", Some("/project/cmd/main.go"));
    }

    #[test]
    fn hints_for_md_files() {
        generate_investigation_hints("Edit", Some("/project/README.md"));
    }

    #[test]
    fn hints_for_unknown_extension_fallback() {
        // Unknown extension should use generic fallback hints
        generate_investigation_hints("Edit", Some("/project/config.yaml"));
        generate_investigation_hints("Write", Some("/project/data.json"));
    }

    #[test]
    fn hints_for_no_action() {
        // None action should still work with generic fallback
        generate_investigation_hints("Edit", None);
        generate_investigation_hints("Write", None);
    }

    #[test]
    fn hints_for_empty_tool_name() {
        // Empty tool name should not match Edit/Write — no output, no panic
        generate_investigation_hints("", Some("/src/main.rs"));
    }

    // ── check_agent_timeout ─────────────────────────
    // SAFETY: All env-var mutations are serialized within individual tests.
    #[test]
    #[serial]
    fn agent_timeout_returns_none_when_disabled() {
        // EPIC_ORCHESTRATION not set — should return None
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        let result = check_agent_timeout("agent-1");
        assert!(
            result.is_none(),
            "should be None when orchestration disabled"
        );
    }

    #[test]
    #[serial]
    fn agent_timeout_returns_none_for_non_agent_tool() {
        // Even with EPIC_ORCHESTRATION=enabled, non-agent id should be fine
        // (no status file → no timeout)
        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
            let result = check_agent_timeout("nonexistent-agent");
            std::env::remove_var("EPIC_ORCHESTRATION");
            assert!(result.is_none(), "no status file means no timeout");
        }
    }

    #[test]
    #[serial]
    fn agent_timeout_detects_overdue_agent() {
        // Create a temp orchestrator dir with a status.json that has a started_at
        // far enough in the past to exceed the threshold
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-1");
        fs::create_dir_all(&agent_dir).unwrap();

        // started_at = 20 minutes ago (1200 seconds)
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started_at = now_secs - 1200;
        let status = serde_json::json!({
            "status": "running",
            "started_at": started_at
        });
        fs::write(agent_dir.join("status.json"), status.to_string()).unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-1", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(result.is_some(), "should detect timeout for overdue agent");
        let msg = result.unwrap();
        assert!(msg.contains("agent-1"), "message should mention agent id");
        assert!(
            msg.contains("timeout:"),
            "message should mention timeout threshold"
        );
    }

    #[test]
    #[serial]
    fn agent_timeout_returns_none_for_recent_agent() {
        // Agent started 1 minute ago — under threshold
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-2");
        fs::create_dir_all(&agent_dir).unwrap();

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started_at = now_secs - 60; // 1 minute ago
        let status = serde_json::json!({
            "status": "running",
            "started_at": started_at
        });
        fs::write(agent_dir.join("status.json"), status.to_string()).unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-2", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(result.is_none(), "recent agent should not trigger timeout");
    }

    #[test]
    #[serial]
    fn agent_timeout_handles_missing_status_file() {
        // Agent dir exists but no status.json
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-3");
        fs::create_dir_all(&agent_dir).unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-3", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(result.is_none(), "missing status file should not error");
    }

    #[test]
    #[serial]
    fn agent_timeout_handles_malformed_status_json() {
        // status.json contains invalid JSON
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-4");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("status.json"), "not json at all").unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-4", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(result.is_none(), "malformed JSON should not error");
    }

    #[test]
    #[serial]
    fn agent_timeout_handles_missing_started_at() {
        // status.json is valid but has no started_at field
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-5");
        fs::create_dir_all(&agent_dir).unwrap();
        let status = serde_json::json!({"status": "running"});
        fs::write(agent_dir.join("status.json"), status.to_string()).unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-5", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(result.is_none(), "missing started_at should not error");
    }

    #[test]
    #[serial]
    fn agent_timeout_handles_completed_agent() {
        // Agent completed — status is not "running"
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agent-6");
        fs::create_dir_all(&agent_dir).unwrap();

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started_at = now_secs - 1200; // 20 minutes ago, but completed
        let status = serde_json::json!({
            "status": "completed",
            "started_at": started_at
        });
        fs::write(agent_dir.join("status.json"), status.to_string()).unwrap();

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = check_agent_timeout_with_dir("agent-6", dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert!(
            result.is_none(),
            "completed agent should not trigger timeout"
        );
    }

    // ── failure_quality helper ──────────────────────
    #[test]
    fn failure_quality_maps_categories() {
        assert_eq!(failure_quality(Some("syntax_error")), 0.3);
        assert_eq!(failure_quality(Some("type_error")), 0.4);
        assert_eq!(failure_quality(Some("runtime_error")), 0.5);
        assert_eq!(failure_quality(Some("test_fail")), 0.6);
        assert_eq!(failure_quality(Some("permission_denied")), 0.7);
        assert_eq!(failure_quality(Some("build_fail")), 0.5);
        assert_eq!(failure_quality(Some("lint_fail")), 0.6);
        assert_eq!(failure_quality(Some("timeout")), 0.5);
        assert_eq!(failure_quality(Some("not_found")), 0.6);
        // None (no failure) returns 1.0 — success quality
        assert_eq!(failure_quality(None), 1.0);
        // Unknown category returns default
        assert_eq!(failure_quality(Some("unknown_category")), 0.5);
    }

    // ── bash failure differentiated quality ──────────
    #[test]
    fn bash_failure_gets_differentiated_quality() {
        let dims = score_bash("TypeError: x is not a function", "node main.js");
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(dims.output_quality, 0.4, "type_error should map to 0.4");
    }

    #[test]
    fn bash_syntax_error_quality() {
        let dims = score_bash("SyntaxError: unexpected token", "node broken.js");
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(dims.output_quality, 0.3, "syntax_error should map to 0.3");
    }

    #[test]
    fn bash_runtime_error_quality() {
        let dims = score_bash("Error: something went wrong", "node main.js");
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(dims.output_quality, 0.5, "runtime_error should map to 0.5");
    }

    // ── other tool failure differentiated quality ───
    #[test]
    fn other_tool_failure_gets_differentiated_quality() {
        // Simulate the catch-all arm logic directly
        let failure_cat = Some("type_error");
        let fq = failure_quality(failure_cat);
        let dims = ScoreDimensions {
            tool_success: if failure_cat.is_none() { 1.0 } else { 0.0 },
            output_quality: fq,
            execution_cost: 1.0,
        };
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(
            dims.output_quality, 0.4,
            "catch-all should use failure_quality"
        );
    }

    // ── bash success with warning still penalized ──
    #[test]
    fn bash_success_with_warning_still_penalized() {
        let dims = score_bash("warning: unused variable", "cargo build");
        assert_eq!(dims.tool_success, 1.0);
        assert!(
            dims.output_quality < 1.0,
            "warnings on success should still reduce quality"
        );
        assert_eq!(dims.output_quality, 0.7, "1.0 - 0.3 = 0.7");
    }

    #[test]
    fn bash_failure_with_warning_not_double_penalized() {
        // When there is a failure, warning penalty should NOT apply
        let dims = score_bash("TypeError: x\nwarning: something", "node main.js");
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(
            dims.output_quality, 0.4,
            "type_error quality, no warning penalty"
        );
    }

    // ── edit failure differentiated quality ──────────
    #[test]
    fn edit_failure_gets_differentiated_quality() {
        let dims = score_edit("TypeError: cannot read property of undefined", None, None);
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(
            dims.output_quality, 0.4,
            "type_error in edit should map to 0.4"
        );
    }

    #[test]
    fn edit_syntax_error_quality() {
        let dims = score_edit("SyntaxError: unexpected token", None, None);
        assert_eq!(dims.tool_success, 0.0);
        assert_eq!(
            dims.output_quality, 0.3,
            "syntax_error in edit should map to 0.3"
        );
    }

    #[test]
    fn edit_no_changes_still_works() {
        let dims = score_edit("no changes made", None, None);
        assert_eq!(dims.tool_success, 1.0);
        assert_eq!(
            dims.output_quality, 0.3,
            "no-changes quality should be preserved"
        );
    }
}
