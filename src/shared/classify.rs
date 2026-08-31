use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

// ── Failure Classification ──────────────────────────

struct FailureRule {
    pattern: &'static str,
    category: &'static str,
}

const FAILURE_RULES: &[FailureRule] = &[
    FailureRule {
        pattern: r"(?i)TypeError|type error",
        category: "type_error",
    },
    FailureRule {
        pattern: r"(?i)SyntaxError|Unexpected token|Parse error",
        category: "syntax_error",
    },
    FailureRule {
        // Count-guarded test failures. Every alternative requires a non-zero
        // failure count so green summaries never match:
        //   - cargo:  "test result: ok. 5 passed; 0 failed;"       → no
        //   - dotnet: "Passed! - Failed: 0, Passed: 10"            → no
        //   - mocha:  "5 passing, 0 failing"                       → no
        //   - Maven:  "Tests run: 4, Failures: 3, Errors: 0"       → yes (label-then-number)
        //   - jest:   "Tests: 3 failed, 5 passed"                  → yes (number-then-label)
        //   - rust:   "test result: FAILED. 0 passed; 1 failed"    → yes (loud banner)
        // `\b(?:FAIL|FAILED)\b` is case-sensitive: failure banners are loud
        // (Go "FAIL", cargo "FAILED"); lowercase "failed" in prose is too
        // noisy without a count. Bare `assertion` was dropped — it matched the
        // word in grep/docs success output.
        pattern: r"\b(?:FAIL|FAILED)\b|(?i:\b[1-9]\d*\s+(?:failed|failures?|failing)\b)|(?i:(?:failures?|errors?|failed|failing)\s*[:=]\s*[1-9]\d*)|(?i:\btests?\s+failed\b)|AssertionError|(?i:assert\.\w+)",
        category: "test_fail",
    },
    FailureRule {
        pattern: r"(?i)\blint\b.*(?:error|fail)|eslint.*error|biome.*error|oxlint.*error",
        category: "lint_fail",
    },
    FailureRule {
        pattern: r"(?i)build.*fail|tsc.*error|error TS\d+|compilation.*fail",
        category: "build_fail",
    },
    FailureRule {
        pattern: r"(?i)EACCES|permission denied",
        category: "permission_denied",
    },
    FailureRule {
        pattern: r"(?i)timeout|ETIMEDOUT|timed out",
        category: "timeout",
    },
    FailureRule {
        pattern: r"(?i)ENOENT|No such file or directory",
        category: "not_found",
    },
    FailureRule {
        pattern: r"(?m)(?:^|\n)\s*(?:Error|error|ERROR):|Traceback|at [\w.]+\s*\(|Unhandled|uncaught exception",
        category: "runtime_error",
    },
];

static COMPILED_RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    FAILURE_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.category)))
        .collect()
});

pub fn classify_failure(output: &str) -> Option<&'static str> {
    if output.is_empty() {
        return None;
    }
    let sample = &output[..output.len().min(2000)];
    for (rx, cat) in COMPILED_RULES.iter() {
        if rx.is_match(sample) {
            return Some(cat);
        }
    }
    None
}

/// Commands whose stdout is *quoted material* — file contents, diffs, search
/// hits — rather than a report on the command's own outcome.
///
/// Reading a test file that contains `assert.equal`, or grepping for
/// `TypeError`, is a successful read; scoring it as a failed tool call
/// poisons every downstream pattern (repeated-error, thrashing, weak-tool
/// rates). The keyword rules cannot tell the two apart, because the words are
/// genuinely in the output either way — so the command has to be the guard.
///
/// Deliberately conservative: a pipeline that *also* runs a real build/test
/// (`rg foo && cargo test`) must not be treated as read-only, so anything
/// containing a shell chain operator is excluded.
static READ_ONLY_CMD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^\s*
        (?:sudo\s+)?
        (?:
            cat|bat|head|tail|nl|less|more|
            rg|grep|egrep|fgrep|ag|ack|
            find|fd|ls|tree|stat|file|wc|du|
            sed|awk|cut|sort|uniq|diff|
            echo|printf|pwd|whoami|date|env
        )\b
        |^\s*git\s+(?:diff|log|show|status|blame|branch|remote|describe|rev-parse)\b
        ",
    )
    .unwrap()
});

/// Shell operators that can append a non-read-only stage to a read-only head.
static CHAINED_CMD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[;&|]|\$\(|`").unwrap());

/// True when `command`'s output should not be keyword-scanned for failures.
pub fn is_read_only_command(command: &str) -> bool {
    !CHAINED_CMD.is_match(command) && READ_ONLY_CMD.is_match(command)
}

/// Classify a Bash tool result.
///
/// `stderr` — when the host reports it separately — is the command's own
/// diagnostic channel and stays trustworthy even for read-only commands.
/// `stdout` from a read-only command is quoted material and is not scanned.
/// Hosts that merge the two streams (Codex exposes one response string) get
/// the safe answer for read-only commands: no failure claimed.
pub fn classify_bash_failure(
    command: &str,
    stdout: &str,
    stderr: &str,
    streams_separated: bool,
) -> Option<&'static str> {
    if !is_read_only_command(command) {
        return classify_failure(&format!("{stdout}\n{stderr}"));
    }
    if streams_separated {
        return classify_failure(stderr);
    }
    None
}

pub fn classify_tool(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "bash" => "bash",
        "edit" => "edit",
        "write" => "write",
        "read" => "read",
        "glob" => "glob",
        "grep" => "grep",
        _ => "other",
    }
}

pub fn extract_file_ext(input: &serde_json::Value) -> Option<String> {
    let file_path = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !file_path.is_empty() {
        return Path::new(file_path)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()));
    }

    let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    static EXT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\.(ts|js|py|go|rs|java|c|cpp|rb|sh|json|yaml|yml|md|css|html|tsx|jsx)\b")
            .unwrap()
    });
    EXT_RE.find(cmd).map(|m| m.as_str().to_string())
}

// ── Guard Rules ─────────────────────────────────────

pub struct GuardRule {
    pub pattern: Regex,
    pub msg: String,
}

/// Parse simple guard-rules.yaml
pub fn parse_guard_rules(content: &str) -> (Vec<GuardRule>, Vec<GuardRule>) {
    let mut blocked = vec![];
    let mut warned = vec![];
    let mut section: Option<&str> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "blocked:" {
            section = Some("blocked");
            continue;
        }
        if trimmed == "warned:" {
            section = Some("warned");
            continue;
        }
        let Some(sec) = section else { continue };
        if !trimmed.starts_with("- ") {
            continue;
        }

        let entry = &trimmed[2..];
        // Format: "pattern: <regex> | msg: <message>"
        if let Some((pat_part, msg_part)) = entry.split_once(" | msg: ") {
            let pat_str = pat_part.trim_start_matches("pattern:").trim();
            if let Ok(rx) = Regex::new(pat_str) {
                let rule = GuardRule {
                    pattern: rx,
                    msg: msg_part.trim().to_string(),
                };
                match sec {
                    "blocked" => blocked.push(rule),
                    "warned" => warned.push(rule),
                    _ => {}
                }
            }
        }
    }
    (blocked, warned)
}

pub fn extract_file(action: &str) -> Option<&str> {
    static FILE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(/[\w./-]+\.\w+)").unwrap());
    FILE_RE.find(action).map(|m| m.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_recognized() {
        for cmd in [
            "cat src/main.rs",
            "rg TypeError src/",
            "sed -n '1,80p' file.ts",
            "git diff HEAD~1",
            "  find . -name '*.rs'",
            "nl -ba README.md",
        ] {
            assert!(is_read_only_command(cmd), "should be read-only: {cmd}");
        }
    }

    #[test]
    fn mutating_and_chained_commands_not_read_only() {
        for cmd in [
            "cargo test",
            "npm run build",
            "rg foo && cargo test",
            "cat a.txt | tee b.txt",
            "git commit -m x",
            "echo $(cargo test)",
        ] {
            assert!(!is_read_only_command(cmd), "must not be read-only: {cmd}");
        }
    }

    /// The core of issue #113 finding 10: reading a file that *contains*
    /// failure words is a successful read, not a failed tool call.
    #[test]
    fn reading_file_containing_failure_words_is_not_a_failure() {
        let contents = "TypeError: expected\nassert.equal(a, b)\nFAILED";
        assert!(
            classify_bash_failure("cat tests/fixtures/errors.txt", contents, "", true).is_none(),
            "quoted file contents must not be scored as a tool failure"
        );
        // Same payload from a merged-stream host (Codex) — still not a failure.
        assert!(
            classify_bash_failure("rg TypeError src/", contents, "", false).is_none(),
            "merged-stream hosts must default to no-failure for read-only commands"
        );
    }

    /// A read-only command that genuinely fails still counts, when the host
    /// gives stderr its own channel.
    #[test]
    fn read_only_command_real_stderr_failure_still_counts() {
        assert_eq!(
            classify_bash_failure(
                "cat missing.txt",
                "",
                "cat: missing.txt: No such file or directory",
                true
            ),
            Some("not_found")
        );
    }

    /// Non-read-only commands keep the previous behavior exactly.
    #[test]
    fn build_failure_still_classified() {
        assert_eq!(
            classify_bash_failure("cargo build", "error TS2304: cannot find name", "", true),
            Some("build_fail")
        );
    }
}
