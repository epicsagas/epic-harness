use regex::Regex;
use std::sync::LazyLock;

use super::common::{self, HookInput, hint};

struct BuiltinRule {
    pattern: &'static str,
    msg: &'static str,
}

const BLOCKED_RULES: &[BuiltinRule] = &[
    BuiltinRule { pattern: r"git\s+push\s+.*--force\s+(origin\s+)?(main|master)\b", msg: "Force push to main/master blocked" },
    BuiltinRule { pattern: r"rm\s+-rf\s+/\s*$|rm\s+-rf\s+/\s", msg: "rm -rf / blocked" },
    BuiltinRule { pattern: r"(?i)DROP\s+(DATABASE|TABLE)\s+.*prod", msg: "DROP on production DB blocked" },
];

const WARNED_RULES: &[BuiltinRule] = &[
    BuiltinRule { pattern: r"git\s+push\s+.*--force", msg: "Force push — ensure this is intentional" },
    BuiltinRule { pattern: r"git\s+reset\s+--hard", msg: "Hard reset will discard local changes" },
    BuiltinRule { pattern: r"rm\s+-rf\s+", msg: "Recursive delete — double-check the path" },
];

static COMPILED_BLOCKED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    BLOCKED_RULES.iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

static COMPILED_WARNED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    WARNED_RULES.iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

fn check_blocked(cmd: &str) -> Option<&'static str> {
    for (rx, msg) in COMPILED_BLOCKED.iter() {
        if rx.is_match(cmd) { return Some(msg); }
    }
    None
}

fn check_warned(cmd: &str) -> Vec<&'static str> {
    COMPILED_WARNED.iter()
        .filter(|(rx, _)| rx.is_match(cmd))
        .map(|(_, msg)| *msg)
        .collect()
}

pub fn run(input: &HookInput) -> i32 {
    let cmd = input.tool_input.as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd.is_empty() { return 0; }

    // Check built-in blocked rules
    if let Some(msg) = check_blocked(cmd) {
        hint("guard", &format!("BLOCKED: {msg}"));
        return 2;
    }

    // Check custom blocked rules
    let rules_file = common::guard_rules_file();
    if common::harness_exists() && rules_file.is_file()
        && let Ok(content) = std::fs::read_to_string(&rules_file)
    {
        let (custom_blocked, custom_warned) = common::parse_guard_rules(&content);
        for rule in &custom_blocked {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("BLOCKED: {}", rule.msg));
                return 2;
            }
        }
        for rule in &custom_warned {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("WARNING: {}", rule.msg));
            }
        }
    }

    // Check built-in warned rules
    for msg in check_warned(cmd) {
        hint("guard", &format!("WARNING: {msg}"));
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
}
