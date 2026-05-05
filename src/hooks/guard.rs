use regex::Regex;
use std::sync::LazyLock;

use super::common::{self, HookInput, hint, should_run, PROFILE_GUARD};
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
        r"^(feat|fix|build|chore|ci|docs|style|refactor|perf|test)(\([a-zA-Z0-9_/.,:-]+\))?!?:\s.+"
    ).unwrap()
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
    static HEREDOC_START: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"git\s+commit\s+.*-m\s+"\$\(cat\s+<<'?(\w+)'?"#).unwrap()
    });
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
    static SIMPLE_DQ: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"git\s+commit\s+.*?-m\s+"([^"]+)""#).unwrap()
    });
    static SIMPLE_SQ: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"git\s+commit\s+.*?-m\s+'([^']+)'"#).unwrap()
    });
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

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_GUARD) {
        return 0;
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
        assert!(check_conventional_commit(r#"git commit -m "feat(auth): add login endpoint""#).is_none());
    }

    #[test]
    fn cc_valid_fix_no_scope() {
        assert!(check_conventional_commit(r#"git commit -m "fix: resolve null pointer""#).is_none());
    }

    #[test]
    fn cc_valid_breaking() {
        assert!(check_conventional_commit(r#"git commit -m "refactor!: drop legacy API""#).is_none());
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
        assert!(check_conventional_commit(r#"git commit -m "fix(cli,index): prevent injection""#).is_none());
    }

    #[test]
    fn cc_valid_multi_m_body() {
        // Second -m is the body; subject should be validated, not the body line
        let cmd = r#"git commit -m "fix(mem): resolve injection" -m "- use rusqlite params""#;
        assert!(check_conventional_commit(cmd).is_none(), "subject is valid CC, body must be ignored");
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
}
