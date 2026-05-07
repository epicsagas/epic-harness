use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::common::{self, CONFLICT_LOOKBACK, HookInput, PROFILE_GUARD, hint, should_run};
use super::telemetry::{RuleKind, Telemetry};

struct BuiltinRule {
    pattern: &'static str,
    msg: &'static str,
}

const BLOCKED_RULES: &[BuiltinRule] = &[
    BuiltinRule {
        pattern: r"git\s+push\s+.*--force\s+(origin\s+)?(main|master)\b",
        msg: "Force push to main/master blocked",
    },
    BuiltinRule {
        pattern: r"rm\s+-rf\s+/([^a-zA-Z0-9_]|$)",
        msg: "rm -rf / blocked",
    },
    BuiltinRule {
        pattern: r"(?i)DROP\s+(DATABASE|TABLE)\s+.*prod",
        msg: "DROP on production DB blocked",
    },
];

/// Conventional Commits pattern: `type(scope): desc` or `type: desc`
/// Optional `!` before `:` for breaking changes.
/// Types: feat, fix, build, chore, ci, docs, style, refactor, perf, test
static CC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(feat|fix|build|chore|ci|docs|style|refactor|perf|test)(\([a-zA-Z0-9_/.,:-]+\))?!?:\s.+",
    )
    .unwrap()
});

/// Extract the commit message from a `git commit -m "..."` command.
/// Handles single quotes, double quotes, and HEREDOC `$(cat <<'EOF' ... EOF)` patterns.
fn extract_commit_message(cmd: &str) -> Option<String> {
    // Normalize CRLF → LF so HEREDOC parsing works on Windows or git CRLF output.
    let normalized;
    let cmd = if cmd.contains('\r') {
        normalized = cmd.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        cmd
    };

    // HEREDOC: find delimiter after <<, then find that delimiter on its own line
    static HEREDOC_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*-m\s+"\$\(cat\s+<<'?(\w+)'?"#).unwrap());
    if let Some(caps) = HEREDOC_START.captures(cmd) {
        let delim = caps[1].to_string();
        let match_end = caps.get(0).unwrap().end();
        // Body starts on the line after the HEREDOC declaration line.
        let after_match = &cmd[match_end..];
        if let Some(nl) = after_match.find('\n') {
            let body_start = match_end + nl + 1;
            if let Some(end_pos) = cmd[body_start..].find(&format!("\n{delim}")) {
                return Some(cmd[body_start..body_start + end_pos].trim().to_string());
            }
        }
        return None;
    }

    // Simple: git commit -m "msg" or git commit -m 'msg'
    // Split into two patterns to avoid mismatched quote matching
    // and to allow the other quote type inside the message.
    // Use non-greedy .*? so we capture the FIRST -m argument (the subject),
    // not the last one (which would be the body in `git commit -m "subj" -m "body"`).
    static SIMPLE_DQ: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*?-m\s+"([^"]+)""#).unwrap());
    static SIMPLE_SQ: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*?-m\s+'([^']+)'"#).unwrap());
    if let Some(caps) = SIMPLE_DQ.captures(cmd) {
        return Some(caps[1].trim().to_string());
    }
    if let Some(caps) = SIMPLE_SQ.captures(cmd) {
        return Some(caps[1].trim().to_string());
    }

    None
}

/// Validate commit message against Conventional Commits.
/// Returns an error message if invalid, None if valid or not a commit command.
fn check_conventional_commit(cmd: &str) -> Option<String> {
    let msg = extract_commit_message(cmd)?;
    let first_line = msg.lines().next().unwrap_or("").trim();
    if first_line.is_empty() || CC_RE.is_match(first_line) {
        None
    } else {
        Some(format!(
            "Commit message does not follow Conventional Commits: \"{first_line}\"\n\
             Expected: type(scope): description  (types: feat|fix|build|chore|ci|docs|style|refactor|perf|test)\n\
             Rewrite the message as: type(scope): description"
        ))
    }
}

const WARNED_RULES: &[BuiltinRule] = &[
    BuiltinRule {
        pattern: r"git\s+push\s+.*--force",
        msg: "Force push — ensure this is intentional",
    },
    BuiltinRule {
        pattern: r"git\s+reset\s+--hard",
        msg: "Hard reset will discard local changes",
    },
    BuiltinRule {
        pattern: r"rm\s+-rf\s+",
        msg: "Recursive delete — double-check the path",
    },
];

static COMPILED_BLOCKED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    BLOCKED_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

static COMPILED_WARNED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    WARNED_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

fn check_blocked(cmd: &str) -> Option<&'static str> {
    for (rx, msg) in COMPILED_BLOCKED.iter() {
        if rx.is_match(cmd) {
            return Some(msg);
        }
    }
    None
}

fn check_warned(cmd: &str) -> Vec<&'static str> {
    COMPILED_WARNED
        .iter()
        .filter(|(rx, _)| rx.is_match(cmd))
        .map(|(_, msg)| *msg)
        .collect()
}

// ── Orchestration: Concurrent Write Conflict Detection ──────

/// Returns true when `EPIC_ORCHESTRATION=enabled` or when `HARNESS_DIR/orchestrator` exists.
fn is_orchestration_enabled() -> bool {
    if std::env::var("EPIC_ORCHESTRATION").as_deref() == Ok("enabled") {
        return true;
    }
    // Also enabled when the orchestrator directory exists
    // (used for testing without env var races)
    if orchestrator_dir().is_some() {
        return true;
    }
    false
}

/// Extract the file path targeted by an Edit or Write tool_input.
fn extract_file_path_from_tool_input(tool_input: &serde_json::Value) -> Option<String> {
    tool_input
        .get("file_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Resolve the orchestrator base directory from `$HARNESS_DIR/orchestrator`.
fn orchestrator_dir() -> Option<PathBuf> {
    std::env::var("HARNESS_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .map(|p| p.join("orchestrator"))
        .filter(|p| p.is_dir())
}

/// Return agent IDs whose `status.json` reports `"running"`.
fn running_agent_ids(orch_dir: &Path) -> Vec<String> {
    let agents_dir = orch_dir.join("agents");
    if !agents_dir.is_dir() {
        return vec![];
    }
    let mut ids = vec![];
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let agent_id = match entry.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let status_path = entry.path().join("status.json");
            if let Ok(content) = std::fs::read_to_string(&status_path)
                && let Ok(doc) = serde_json::from_str::<serde_json::Value>(&content)
                && doc.get("status").and_then(|v| v.as_str()) == Some("running")
            {
                ids.push(agent_id);
            }
        }
    }
    ids
}

/// Read the last N lines of a JSONL file, parsed as generic JSON values.
fn read_jsonl_tail(path: &Path, n: usize) -> Vec<serde_json::Value> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .rev()
        .take(n)
        .collect()
}

/// Check if `file_path` appears in the recent entries of `stream.jsonl`.
fn file_in_recent_entries(entries: &[serde_json::Value], file_path: &str) -> bool {
    entries.iter().any(|entry| {
        // Check tool_input.file_path
        if let Some(fp) = entry
            .get("tool_input")
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            && fp == file_path
        {
            return true;
        }
        // Also check action field (observe-style records)
        if let Some(action) = entry.get("action").and_then(|v| v.as_str())
            && action.contains(file_path)
        {
            return true;
        }
        false
    })
}

/// Detect concurrent write conflicts: returns agent IDs that recently modified
/// the same file path, excluding `exclude_agent_id`.
/// Accepts an explicit orchestrator directory for testability.
fn detect_concurrent_write_conflict(
    file_path: &str,
    exclude_agent_id: Option<&str>,
    orch_dir: &Path,
) -> Vec<String> {
    let running = running_agent_ids(orch_dir);
    let mut conflicts = vec![];

    for agent_id in &running {
        if let Some(exclude) = exclude_agent_id
            && agent_id == exclude
        {
            continue;
        }
        let stream_path = orch_dir.join("agents").join(agent_id).join("stream.jsonl");
        if !stream_path.is_file() {
            continue;
        }
        let recent = read_jsonl_tail(&stream_path, CONFLICT_LOOKBACK);
        if file_in_recent_entries(&recent, file_path) {
            conflicts.push(agent_id.clone());
        }
    }
    conflicts
}

/// Current agent ID from `EPIC_AGENT_ID` env var.
fn current_agent_id() -> Option<String> {
    std::env::var("EPIC_AGENT_ID").ok()
}

/// Check if `control.json` has a "pause" directive targeting the current agent.
/// Returns true if the tool call should be blocked.
/// Accepts an explicit orchestrator directory for testability.
fn check_control_json_pause(current_agent: Option<&str>, orch_dir: &Path) -> bool {
    let control_path = orch_dir.join("control.json");
    let content = match std::fs::read_to_string(&control_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let doc: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return false,
    };

    // Format 1: ControlDirective style {"action": "pause", "target": "agent-x" or "all"}
    if let Some(action) = doc.get("action").and_then(|v| v.as_str()) {
        if action == "pause" {
            let target = doc.get("target").and_then(|v| v.as_str()).unwrap_or("all");
            if target == "all" {
                return true;
            }
            if let Some(agent) = current_agent {
                return target == agent;
            }
        }
        return false;
    }

    // Format 2: Legacy {"pause": ["agent-1", "agent-2"]} or {"pause": "agent-1"} or {"pause": true}
    let pause_val = match doc.get("pause") {
        Some(v) => v,
        None => return false,
    };
    if pause_val.is_boolean() {
        return pause_val.as_bool().unwrap_or(false);
    }
    if pause_val.is_string() {
        if let Some(agent) = current_agent {
            return pause_val.as_str() == Some(agent);
        }
        return false;
    }
    if let Some(arr) = pause_val.as_array() {
        if let Some(agent) = current_agent {
            return arr.iter().any(|v| v.as_str() == Some(agent));
        }
        return false;
    }
    false
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_GUARD) {
        return 0;
    }

    // ── Orchestration checks (Edit/Write tools only) ────
    if is_orchestration_enabled() {
        let tool_name = input.tool_name.as_deref().unwrap_or("");
        let tool_lower = tool_name.to_lowercase();

        if tool_lower == "edit" || tool_lower == "write" {
            let orch_dir = orchestrator_dir();

            // control.json pause check — blocks the tool call entirely
            let agent_id = current_agent_id();
            if let Some(ref orch) = orch_dir
                && check_control_json_pause(agent_id.as_deref(), orch)
            {
                hint(
                    "guard",
                    &format!(
                        "BLOCKED: control.json pause directive active for agent {}",
                        agent_id.as_deref().unwrap_or("unknown")
                    ),
                );
                return 2;
            }

            // Concurrent write conflict detection — informational warning only
            if let Some(ref tool_input) = input.tool_input
                && let Some(file_path) = extract_file_path_from_tool_input(tool_input)
                && let Some(ref orch) = orch_dir
            {
                let conflicts =
                    detect_concurrent_write_conflict(&file_path, agent_id.as_deref(), orch);
                for other_id in &conflicts {
                    hint(
                        "guard",
                        &format!(
                            "Concurrent write conflict: agent {} recently modified {}",
                            other_id, file_path
                        ),
                    );
                }
            }
        }
    }

    let cmd = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd.is_empty() {
        return 0;
    }

    // Lazy: construct Telemetry (file I/O) only when a block or warn actually fires.
    let telemetry = std::cell::OnceCell::new();
    let get_telemetry = || telemetry.get_or_init(Telemetry::init);

    // Check built-in blocked rules first — safety-critical, must run before CC check
    // so a dangerous command appended after a CC-invalid message cannot bypass the block.
    if let Some(msg) = check_blocked(cmd) {
        hint("guard", &format!("BLOCKED: {msg}"));
        get_telemetry().track_hook_blocked(RuleKind::Builtin);
        return 2;
    }

    // Check conventional commit format
    if let Some(msg) = check_conventional_commit(cmd) {
        hint("guard", &format!("BLOCKED: {msg}"));
        get_telemetry().track_hook_blocked(RuleKind::ConventionalCommit);
        return 2;
    }

    // Check custom blocked rules
    let rules_file = common::guard_rules_file();
    if common::harness_exists()
        && rules_file.is_file()
        && let Ok(content) = std::fs::read_to_string(&rules_file)
    {
        let (custom_blocked, custom_warned) = common::parse_guard_rules(&content);
        for rule in &custom_blocked {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("BLOCKED: {}", rule.msg));
                get_telemetry().track_hook_blocked(RuleKind::Custom);
                return 2;
            }
        }
        // Evaluate builtin + custom warned together (same order as TS implementation)
        for msg in check_warned(cmd) {
            hint("guard", &format!("WARNING: {msg}"));
            get_telemetry().track_hook_warned(RuleKind::Builtin);
        }
        for rule in &custom_warned {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("WARNING: {}", rule.msg));
                get_telemetry().track_hook_warned(RuleKind::Custom);
            }
        }
        return 0;
    }

    // No custom rules file — just check builtin warned rules
    for msg in check_warned(cmd) {
        hint("guard", &format!("WARNING: {msg}"));
        get_telemetry().track_hook_warned(RuleKind::Builtin);
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Blocked commands ────────────────────────────
    #[test]
    fn blocks_force_push_main() {
        assert!(check_blocked("git push --force origin main").is_some());
    }

    #[test]
    fn blocks_force_push_master() {
        assert!(check_blocked("git push --force origin master").is_some());
    }

    #[test]
    fn blocks_rm_rf_root() {
        assert!(check_blocked("rm -rf /").is_some());
    }

    #[test]
    fn blocks_rm_rf_root_with_space() {
        assert!(check_blocked("rm -rf / --no-preserve-root").is_some());
    }

    #[test]
    fn blocks_drop_prod_database() {
        assert!(check_blocked("DROP DATABASE prod_db").is_some());
    }

    #[test]
    fn blocks_drop_prod_table() {
        assert!(check_blocked("DROP TABLE production_users").is_some());
    }

    // ── Allowed commands ────────────────────────────
    #[test]
    fn allows_normal_push() {
        assert!(check_blocked("git push origin main").is_none());
    }

    #[test]
    fn allows_force_push_feature() {
        assert!(check_blocked("git push --force origin feature/x").is_none());
    }

    #[test]
    fn allows_rm_rf_dir() {
        assert!(check_blocked("rm -rf /tmp/build").is_none());
    }

    #[test]
    fn blocks_rm_rf_double_slash() {
        assert!(check_blocked("rm -rf //").is_some());
    }

    #[test]
    fn allows_rm_rf_tmp() {
        assert!(check_blocked("rm -rf /tmp").is_none());
    }

    #[test]
    fn allows_rm_rf_var() {
        assert!(check_blocked("rm -rf /var/log").is_none());
    }

    #[test]
    fn allows_drop_dev_db() {
        assert!(check_blocked("DROP DATABASE dev_db").is_none());
    }

    #[test]
    fn allows_git_status() {
        assert!(check_blocked("git status").is_none());
    }

    #[test]
    fn allows_empty() {
        assert!(check_blocked("").is_none());
    }

    // ── Warned commands ─────────────────────────────
    #[test]
    fn warns_force_push_feature() {
        let w = check_warned("git push --force origin feature/x");
        assert!(!w.is_empty());
        assert!(w[0].contains("Force push"));
    }

    #[test]
    fn warns_hard_reset() {
        let w = check_warned("git reset --hard HEAD~3");
        assert!(!w.is_empty());
        assert!(w[0].contains("Hard reset"));
    }

    #[test]
    fn warns_rm_rf_dir() {
        let w = check_warned("rm -rf /tmp/build");
        assert!(!w.is_empty());
        assert!(w[0].contains("Recursive delete"));
    }

    #[test]
    fn no_warning_for_safe_commands() {
        assert!(check_warned("git status").is_empty());
        assert!(check_warned("ls -la").is_empty());
        assert!(check_warned("npm test").is_empty());
    }

    // ── Conventional Commits ─────────────────────────
    #[test]
    fn cc_valid_feat() {
        assert!(
            check_conventional_commit(r#"git commit -m "feat(auth): add login endpoint""#)
                .is_none()
        );
    }

    #[test]
    fn cc_valid_fix_no_scope() {
        assert!(
            check_conventional_commit(r#"git commit -m "fix: resolve null pointer""#).is_none()
        );
    }

    #[test]
    fn cc_valid_breaking() {
        assert!(
            check_conventional_commit(r#"git commit -m "refactor!: drop legacy API""#).is_none()
        );
    }

    #[test]
    fn cc_valid_heredoc() {
        let cmd = "git commit -m \"$(cat <<'EOF'\nfeat(mem): add search\nEOF\n)\"";
        assert!(check_conventional_commit(cmd).is_none());
    }

    #[test]
    fn cc_valid_heredoc_crlf() {
        // Windows CRLF line endings must not break HEREDOC parsing
        let cmd = "git commit -m \"$(cat <<'EOF'\r\nfeat(guard): fix crlf\r\nEOF\r\n)\"";
        assert!(check_conventional_commit(cmd).is_none());
    }

    #[test]
    fn cc_invalid_heredoc_crlf() {
        let cmd = "git commit -m \"$(cat <<'EOF'\r\nadded stuff\r\nEOF\r\n)\"";
        assert!(check_conventional_commit(cmd).is_some());
    }

    #[test]
    fn cc_invalid_no_type() {
        assert!(check_conventional_commit(r#"git commit -m "added login""#).is_some());
    }

    #[test]
    fn cc_invalid_uppercase() {
        assert!(check_conventional_commit(r#"git commit -m "Feat: add login""#).is_some());
    }

    #[test]
    fn cc_not_a_commit() {
        assert!(check_conventional_commit("git status").is_none());
    }

    #[test]
    fn cc_run_blocks_bad_message() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git commit -m \"added stuff\""})),
            ..Default::default()
        };
        assert_eq!(run(&input), 2);
    }

    #[test]
    fn cc_run_allows_good_message() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git commit -m \"feat: add stuff\""})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    #[test]
    fn cc_valid_single_quotes() {
        assert!(check_conventional_commit("git commit -m 'feat: add login'").is_none());
    }

    #[test]
    fn cc_message_with_apostrophe() {
        assert!(check_conventional_commit(r#"git commit -m "feat: it's done""#).is_none());
    }

    #[test]
    fn cc_valid_multi_scope() {
        assert!(
            check_conventional_commit(r#"git commit -m "fix(cli,index): prevent injection""#)
                .is_none()
        );
    }

    #[test]
    fn cc_valid_multi_m_body() {
        // Second -m is the body; subject should be validated, not the body line
        let cmd = r#"git commit -m "fix(mem): resolve injection" -m "- use rusqlite params""#;
        assert!(
            check_conventional_commit(cmd).is_none(),
            "subject is valid CC, body must be ignored"
        );
    }

    #[test]
    fn cc_subject_extracted_not_body() {
        let msg = extract_commit_message(
            r#"git commit -m "fix(mem): subject line" -m "- body line one""#,
        );
        assert_eq!(msg.as_deref(), Some("fix(mem): subject line"));
    }

    #[test]
    fn cc_mismatched_quotes_no_match() {
        assert!(extract_commit_message(r#"git commit -m "bad message'"#).is_none());
    }

    // ── run() integration ───────────────────────────
    #[test]
    fn run_empty_input_returns_0() {
        let input = HookInput::default();
        assert_eq!(run(&input), 0);
    }

    #[test]
    fn run_blocked_returns_2() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git push --force origin main"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 2);
    }

    #[test]
    fn run_safe_returns_0() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git status"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    /// Verify that a safe command (no block/warn) goes through the entire
    /// run() without touching telemetry. We confirm this indirectly: the
    /// OnceCell must still be unset after run() returns 0 for a safe command
    /// with no warnings. The cell is local to run(), so we validate the
    /// observable behaviour: run() returns 0 and does not panic even when
    /// consent/install-id files are absent (because Telemetry::init() is
    /// never called for purely safe commands with no custom rules file).
    #[test]
    fn run_safe_no_warn_no_telemetry_init() {
        // "npm install" matches none of the builtin blocked/warned rules and
        // is not a git commit, so the OnceCell must never be initialised.
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "npm install"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    // ── Orchestration: concurrent write conflict detection ──

    #[test]
    fn extract_file_path_from_edit_input() {
        let input = serde_json::json!({"file_path": "/src/main.rs", "old_string": "fn main()", "new_string": "fn main() {}"});
        assert_eq!(
            extract_file_path_from_tool_input(&input),
            Some("/src/main.rs".into())
        );
    }

    #[test]
    fn extract_file_path_from_write_input() {
        let input = serde_json::json!({"file_path": "/src/lib.ts", "content": "export {}"});
        assert_eq!(
            extract_file_path_from_tool_input(&input),
            Some("/src/lib.ts".into())
        );
    }

    #[test]
    fn extract_file_path_missing_returns_none() {
        let input = serde_json::json!({"command": "git status"});
        assert_eq!(extract_file_path_from_tool_input(&input), None);
    }

    #[test]
    fn extract_file_path_empty_returns_none() {
        let input = serde_json::json!({"file_path": "", "content": ""});
        assert_eq!(extract_file_path_from_tool_input(&input), None);
    }

    #[test]
    fn read_jsonl_tail_reads_last_n() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.jsonl");
        std::fs::write(
            &path,
            "{\"seq\":1}\n{\"seq\":2}\n{\"seq\":3}\n{\"seq\":4}\n{\"seq\":5}\n",
        )
        .unwrap();
        let entries = read_jsonl_tail(&path, 3);
        assert_eq!(entries.len(), 3);
        // rev order: 5, 4, 3
        assert_eq!(entries[0]["seq"], 5);
        assert_eq!(entries[1]["seq"], 4);
        assert_eq!(entries[2]["seq"], 3);
    }

    #[test]
    fn read_jsonl_tail_missing_file_empty() {
        let path = std::path::Path::new("/tmp/nonexistent_epic_test_file.jsonl");
        let entries = read_jsonl_tail(path, 3);
        assert!(entries.is_empty());
    }

    #[test]
    fn file_in_recent_entries_matches_tool_input_file_path() {
        let entries = vec![serde_json::json!({
            "tool_input": {"file_path": "/src/main.rs"}
        })];
        assert!(file_in_recent_entries(&entries, "/src/main.rs"));
        assert!(!file_in_recent_entries(&entries, "/src/other.rs"));
    }

    #[test]
    fn file_in_recent_entries_matches_action_field() {
        let entries = vec![serde_json::json!({
            "action": "Edit /src/lib.rs: replaced fn"
        })];
        assert!(file_in_recent_entries(&entries, "/src/lib.rs"));
        assert!(!file_in_recent_entries(&entries, "/src/main.rs"));
    }

    #[test]
    fn file_in_recent_entries_empty_no_match() {
        let entries: Vec<serde_json::Value> = vec![];
        assert!(!file_in_recent_entries(&entries, "/src/main.rs"));
    }

    #[test]
    fn detect_conflict_finds_other_agent() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        // agent-1: running, with stream containing file path
        let agent1_dir = agents_dir.join("agent-1");
        std::fs::create_dir_all(&agent1_dir).unwrap();
        std::fs::write(agent1_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent1_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/main.rs\"}}\n",
        )
        .unwrap();

        // agent-2: running, different file
        let agent2_dir = agents_dir.join("agent-2");
        std::fs::create_dir_all(&agent2_dir).unwrap();
        std::fs::write(agent2_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent2_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/other.rs\"}}\n",
        )
        .unwrap();

        // agent-1 modified /src/main.rs, we are agent-2
        let conflicts =
            detect_concurrent_write_conflict("/src/main.rs", Some("agent-2"), &orch_dir);
        assert!(conflicts.contains(&"agent-1".to_string()));
        assert!(!conflicts.contains(&"agent-2".to_string()));

        // agent-2 modified /src/other.rs, no conflict for /src/main.rs
        let no_conflicts =
            detect_concurrent_write_conflict("/src/main.rs", Some("agent-1"), &orch_dir);
        assert!(no_conflicts.is_empty());
    }

    #[test]
    fn detect_conflict_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        let conflicts = detect_concurrent_write_conflict("/src/main.rs", None, &orch_dir);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn detect_conflict_ignores_stopped_agents() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        let agent_dir = agents_dir.join("agent-stopped");
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("status.json"), "{\"status\":\"stopped\"}").unwrap();
        std::fs::write(
            agent_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/main.rs\"}}\n",
        )
        .unwrap();

        let conflicts = detect_concurrent_write_conflict("/src/main.rs", None, &orch_dir);
        assert!(conflicts.is_empty());
    }

    // ── control.json pause enforcement ──────────────────────

    #[test]
    fn control_json_pause_boolean_true() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":true}").unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir));
    }

    #[test]
    fn control_json_pause_boolean_false() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":false}").unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir));
    }

    #[test]
    fn control_json_pause_string_match() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-1\"}").unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir));
        assert!(!check_control_json_pause(Some("agent-2"), &orch_dir));
    }

    #[test]
    fn control_json_pause_array_match() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(
            orch_dir.join("control.json"),
            "{\"pause\":[\"agent-1\",\"agent-3\"]}",
        )
        .unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir));
        assert!(!check_control_json_pause(Some("agent-2"), &orch_dir));
        assert!(check_control_json_pause(Some("agent-3"), &orch_dir));
    }

    #[test]
    fn control_json_no_pause_field() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"resume\":true}").unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir));
    }

    #[test]
    fn control_json_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        // Dir exists but no control.json file
        std::fs::create_dir_all(&orch_dir).unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir));
    }

    // ── run() integration with orchestration ────────────────

    #[test]
    fn run_orchestration_pause_blocks_edit() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-x\"}").unwrap();

        // SAFETY: env mutation scoped to this test; HARNESS_DIR only read by
        // orchestrator_dir() which resolves the temp path we created above.
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-x");
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(
                serde_json::json!({"file_path": "/src/main.rs", "old_string": "x", "new_string": "y"}),
            ),
            ..Default::default()
        };
        assert_eq!(run(&input), 2, "pause directive must block Edit tool call");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    fn run_orchestration_pause_not_targeting_passes() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-other\"}").unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-x");
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(
                serde_json::json!({"file_path": "/src/main.rs", "old_string": "x", "new_string": "y"}),
            ),
            ..Default::default()
        };
        assert_eq!(run(&input), 0, "non-targeted agent must not be blocked");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    fn run_orchestration_not_enabled_skips_checks() {
        // No HARNESS_DIR set — orchestration checks are skipped entirely
        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(serde_json::json!({"file_path": "/src/main.rs"})),
            ..Default::default()
        };
        // Should return 0 (no command field -> early return, no orchestration block)
        assert_eq!(run(&input), 0);
    }

    #[test]
    fn run_orchestration_concurrent_warning_does_not_block() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        // agent-1 running, modified /src/shared.rs
        let agent1_dir = agents_dir.join("agent-1");
        std::fs::create_dir_all(&agent1_dir).unwrap();
        std::fs::write(agent1_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent1_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/shared.rs\"}}\n",
        )
        .unwrap();

        // control.json with no pause
        std::fs::write(orch_dir.join("control.json"), "{}").unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-2");
        }

        let input = HookInput {
            tool_name: Some("Write".into()),
            tool_input: Some(serde_json::json!({"file_path": "/src/shared.rs", "content": "new"})),
            ..Default::default()
        };
        // Warning is informational — must NOT block (return 0)
        assert_eq!(run(&input), 0, "concurrent write warning must not block");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
        }
    }

    #[test]
    fn run_orchestration_bash_tool_skipped() {
        // Bash tool should not trigger orchestration checks
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
        }

        let input = HookInput {
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "git status"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);

        unsafe {
            std::env::remove_var("HARNESS_DIR");
        }
    }
}
