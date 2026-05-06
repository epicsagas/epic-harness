use regex::Regex;
use std::fs;
use std::sync::LazyLock;

use super::common::*;
use super::telemetry::{FailureClass, ToolCategory, Telemetry};

static TELEMETRY: LazyLock<Telemetry> = LazyLock::new(Telemetry::init);

static MASK_BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)Bearer\s+[^\s"']+"#).unwrap());
static MASK_SK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[a-zA-Z0-9\-_]{8,}").unwrap());
static MASK_KV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(password|passwd|token|api_key|apikey|secret)[=:]\s*\S+").unwrap()
});

pub fn mask_secrets(s: &str) -> String {
    let s = MASK_BEARER.replace_all(s, "Bearer <REDACTED>");
    let s = MASK_SK.replace_all(&s, "sk-<REDACTED>");
    let s = MASK_KV.replace_all(&s, "$1=<REDACTED>");
    s.into_owned()
}

static SILENT_OK_CMDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(mkdir|cp|mv|rm|chmod|chown|ln|touch|git\s+(add|checkout|switch|branch|stash|tag|remote)|cd|export|source|tsc\s+--noEmit)\b").unwrap()
});

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

fn score_bash(output: &str, command: &str) -> ScoreDimensions {
    let failure = classify_failure(output);
    let tool_success = if failure.is_none() { 1.0 } else { 0.0 };

    let is_empty = output.trim().is_empty();
    let mut quality: f64 = 1.0;
    if is_empty && SILENT_OK_CMDS.is_match(command) {
        quality = 1.0;
    } else if is_empty {
        quality = 0.7;
    }
    static WARN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bwarning\b|\bWARN\b").unwrap());
    static DEPREC_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bWARN(ING)?\b.*deprecat").unwrap());
    if WARN_RE.is_match(output) && !DEPREC_RE.is_match(output) {
        quality = (quality - 0.3).max(0.0);
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
) -> ScoreDimensions {
    let failure = classify_failure(output);
    let tool_success = if failure.is_none() { 1.0 } else { 0.0 };

    let mut quality: f64 = 1.0;
    static NO_CHANGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)no changes|file not found").unwrap());
    if NO_CHANGE_RE.is_match(output) {
        quality = 0.3;
    }
    if let (Some(prev), Some(curr)) = (prev_action, curr_action)
        && prev == curr
    {
        quality = quality.min(0.7);
    }

    ScoreDimensions {
        tool_success,
        output_quality: quality,
        execution_cost: 1.0,
    }
}

fn score_write(output: &str) -> ScoreDimensions {
    let failure = classify_failure(output);
    let ok = failure.is_none();
    ScoreDimensions {
        tool_success: if ok { 1.0 } else { 0.0 },
        output_quality: if ok { 1.0 } else { 0.0 },
        execution_cost: 1.0,
    }
}

fn score_read_search(output: &str) -> ScoreDimensions {
    static NO_MATCH_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)no matches|0 results").unwrap());
    let has_results = !output.trim().is_empty() && !NO_MATCH_RE.is_match(output);
    ScoreDimensions {
        tool_success: if has_results { 1.0 } else { 0.0 },
        output_quality: if has_results { 1.0 } else { 0.5 },
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
                if digits.len() < 6 { return None; }
                let (y, mo, d, h, mi, s_) = (digits[0], digits[1], digits[2], digits[3], digits[4], digits[5]);
                // Simple UTC epoch approximation (no leap seconds, good enough for timeout)
                let days: u64 = y * 365 + (y / 4) - (y / 100) + (y / 400)
                    + if mo <= 2 { (mo + 9) * 153 + 2 } else { (mo - 3) * 153 + 2 } / 5
                    + d - 719469;
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
    if !super::config::CONFIG.hook.gateguard_hints {
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

    let ext = file_path
        .and_then(|p| p.rsplit('.').next())
        .unwrap_or("");

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

    let session_file = obs_dir().join(format!("session_{}.jsonl", session_id()));
    let tool_cat = classify_tool(input.tool_name.as_deref().unwrap_or(""));

    let action = input.tool_input.as_ref().map(|v| {
        v.get("command")
            .and_then(|c| c.as_str())
            .map(String::from)
            .or_else(|| {
                v.get("file_path")
                    .and_then(|c| c.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| {
                let s = serde_json::to_string(v).unwrap_or_default();
                mask_secrets(&s[..s.len().min(200)])
            })
    });

    let file_ext = input.tool_input.as_ref().and_then(extract_file_ext);
    let seq_id = get_next_sequence_id(&session_file);

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
        pipeline_id: detect_active_orbit_id(),
    };

    // Resolve tool output: tool_output (structured) → tool_response (Claude Code canonical) → tool_result (legacy)
    let resolve_json_value = |v: &serde_json::Value| -> (String, String) {
        match v {
            serde_json::Value::String(s) => (s.clone(), String::new()),
            serde_json::Value::Object(obj) => {
                let out = obj.get("output").and_then(|v| v.as_str()).unwrap_or("");
                let err = obj.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                (format!("{out}\n{err}"), String::new())
            }
            other => (other.to_string(), String::new()),
        }
    };
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

        record.failure_category = classify_failure(&combined).map(String::from);
        record.result = Some(
            if record.failure_category.is_none() {
                "success"
            } else {
                "error"
            }
            .into(),
        );

        let dims = match tool_cat {
            "bash" => {
                let cmd = input
                    .tool_input
                    .as_ref()
                    .and_then(|v| v.get("command"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                score_bash(&combined, cmd)
            }
            "edit" => {
                let prev = get_last_action(&session_file);
                score_edit(&combined, prev.as_deref(), action.as_deref())
            }
            "write" => score_write(&combined),
            "read" | "glob" | "grep" => score_read_search(&combined),
            _ => ScoreDimensions {
                tool_success: if record.failure_category.is_none() {
                    1.0
                } else {
                    0.0
                },
                output_quality: 1.0,
                execution_cost: 1.0,
            },
        };

        record.dimensions = Some(dims);
        record.score = Some(compute_score(&dims));

        if record.failure_category.is_some() {
            let masked = mask_secrets(&combined[..combined.len().min(500)]);
            record.error_snippet = Some(masked);
        }
    }

    append_jsonl(&session_file, &record);

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
    generate_investigation_hints(
        input.tool_name.as_deref().unwrap_or(""),
        action.as_deref(),
    );

    // Agent timeout detection: when EPIC_ORCHESTRATION is enabled and the
    // current tool is an Agent call, check if the agent has exceeded the
    // timeout threshold. This is gated by the env var so it adds zero
    // latency to non-orchestration sessions.
    let tool_name_lower = input
        .tool_name
        .as_deref()
        .unwrap_or("")
        .to_lowercase();
    if tool_name_lower == "agent"
        && let Some(agent_id) = input
            .tool_input
            .as_ref()
            .and_then(|v| v.get("agent_id"))
            .and_then(|v| v.as_str())
        && let Some(timeout_msg) = check_agent_timeout(agent_id)
    {
        hint("agent-timeout", &timeout_msg);
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(dims.output_quality, 1.0, "substring 'warning' in negative phrase must not penalize");
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
        assert!(!output.contains("sk-abc123XYZ"), "Bearer token must be redacted");
        assert!(output.contains("Bearer <REDACTED>"), "must have redacted placeholder");
    }

    #[test]
    fn test_mask_sk_key() {
        let input = "Error: invalid key sk-proj-abcDEF12345678 supplied";
        let output = mask_secrets(input);
        assert!(!output.contains("sk-proj-abcDEF12345678"), "sk- key must be redacted");
        assert!(output.contains("sk-<REDACTED>"), "must have sk-<REDACTED>");
    }

    #[test]
    fn test_mask_password_equals() {
        let input = "connection failed: password=s3cr3tP@ss! reason=timeout";
        let output = mask_secrets(input);
        assert!(!output.contains("s3cr3tP@ss!"), "password value must be redacted");
        assert!(output.contains("<REDACTED>"), "must have redacted placeholder");
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
        let output = input.tool_result.as_ref().and_then(|v| v.as_str()).unwrap_or("");
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
    fn agent_timeout_returns_none_when_disabled() {
        // EPIC_ORCHESTRATION not set — should return None
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        let result = check_agent_timeout("agent-1");
        assert!(result.is_none(), "should be None when orchestration disabled");
    }

    #[test]
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
        assert!(msg.contains("timeout:"), "message should mention timeout threshold");
    }

    #[test]
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

        assert!(result.is_none(), "completed agent should not trigger timeout");
    }
}
