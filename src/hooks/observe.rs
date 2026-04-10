use regex::Regex;
use std::sync::LazyLock;

use super::common::*;

static SILENT_OK_CMDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(mkdir|cp|mv|rm|chmod|chown|ln|touch|git\s+(add|checkout|switch|branch|stash|tag|remote)|cd|export|source|tsc\s+--noEmit)\b").unwrap()
});

fn get_next_sequence_id(session_file: &std::path::Path) -> u64 {
    std::fs::read(session_file)
        .map(|buf| buf.iter().filter(|&&b| b == b'\n').count() as u64 + 1)
        .unwrap_or(1)
}

fn get_last_action(session_file: &std::path::Path) -> Option<String> {
    let data = std::fs::read(session_file).ok()?;
    let len = data.len();
    let start = len.saturating_sub(1024);
    let tail = String::from_utf8_lossy(&data[start..]);
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
    static WARN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)warning|WARN").unwrap());
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

pub fn run(input: &HookInput) -> i32 {
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
                s[..s.len().min(200)].to_string()
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
    };

    if let Some(tool_output) = &input.tool_output {
        let output = tool_output.output.as_deref().unwrap_or("");
        let stderr = tool_output.stderr.as_deref().unwrap_or("");
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
            record.error_snippet = Some(combined[..combined.len().min(500)].to_string());
        }
    }

    append_jsonl(&session_file, &record);
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
}
