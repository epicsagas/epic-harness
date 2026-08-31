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
    let sample = crate::shared::sanitize::truncate_bytes(output, 2000);
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
/// containing a sequencing or substitution operator is excluded. A plain `|`
/// pipe is decomposed instead: every stage must itself be read-only.
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

/// Shell operators that can append a non-read-only stage to a read-only head
/// in ways a per-stage check cannot see: `;` and `&&`/`&` sequence independent
/// commands, `$()`/`` ` `` splice one command's output into another's argv,
/// and a bare `&` backgrounds. `|` is deliberately absent: it is handled by
/// splitting into stages, since a pure-read pipeline (`rg foo | head`) is the
/// common case and must stay read-only.
static CHAINED_CMD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[;&]|\$\(|`").unwrap());

/// Redirections write to a file, so a stage carrying one is not read-only even
/// if its head command is. `2>&1` is a stream merge, not a file write, and is
/// allowed; `&` is otherwise already rejected by [`CHAINED_CMD`].
static REDIRECT_CMD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r">|<\(").unwrap());

/// True when `command`'s output should not be keyword-scanned for failures.
///
/// A `|` pipeline qualifies only when *every* stage is itself read-only:
/// `rg foo | head` is quoted material, but `cat a.txt | tee b.txt` writes.
pub fn is_read_only_command(command: &str) -> bool {
    if CHAINED_CMD.is_match(command) {
        return false;
    }
    command
        .split('|')
        .all(|stage| !REDIRECT_CMD.is_match(stage) && READ_ONLY_CMD.is_match(stage))
}

/// Files named by a Codex `apply_patch` payload.
///
/// Codex passes the patch as a command string instead of Claude Code's
/// `file_path` field, so anything keyed on `file_path` alone sees no files at
/// all on that host. Each touched file gets its own header line:
///
/// ```text
/// *** Add File: src/a.ts
/// *** Update File: src/b.py
/// *** Delete File: src/c.go
/// ```
///
/// `include_deletes` distinguishes the two callers: formatting has nothing to
/// do on a deleted file, but write-conflict detection still must count it.
pub fn apply_patch_paths(patch: &str, include_deletes: bool) -> Vec<String> {
    let verbs: &[&str] = if include_deletes {
        &["Add File:", "Update File:", "Delete File:"]
    } else {
        &["Add File:", "Update File:"]
    };
    patch
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("***")?.trim_start();
            for verb in verbs {
                if let Some(p) = rest.strip_prefix(verb) {
                    let p = p.trim();
                    if !p.is_empty() {
                        return Some(p.to_string());
                    }
                }
            }
            None
        })
        .collect()
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
        // Codex's canonical edit tool — without this every Codex edit was
        // categorized as "other" and scored by the generic path.
        "edit" | "apply_patch" => "edit",
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
    fn apply_patch_paths_respect_delete_flag() {
        let patch = "*** Begin Patch\n                     *** Add File: src/a.ts\n                     *** Update File: src/b.py\n                     *** Delete File: src/c.go\n                     *** End Patch";
        // polish: nothing to format in a deleted file
        assert_eq!(
            apply_patch_paths(patch, false),
            vec!["src/a.ts".to_string(), "src/b.py".to_string()]
        );
        // guard: a delete still races with another agent's write
        assert_eq!(
            apply_patch_paths(patch, true),
            vec![
                "src/a.ts".to_string(),
                "src/b.py".to_string(),
                "src/c.go".to_string()
            ]
        );
    }

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

    /// A pipeline whose every stage only reads is still quoted material. The
    /// blanket "any `|` means not read-only" rule reintroduced exactly the
    /// false failures the read-only guard exists to prevent.
    #[test]
    fn pure_read_pipelines_stay_read_only() {
        for cmd in [
            "rg TypeError src/ | head",
            "cat src/main.rs | wc -l",
            "git diff HEAD~1 | grep panic",
            "ls -la | sort | uniq",
        ] {
            assert!(is_read_only_command(cmd), "should be read-only: {cmd}");
        }
    }

    /// One writing stage anywhere in the pipe disqualifies the whole command.
    #[test]
    fn pipelines_with_a_writing_stage_not_read_only() {
        for cmd in [
            "cat a.txt | tee b.txt",
            "rg foo src/ | xargs sed -i s/a/b/",
            "cat a.txt > b.txt",
            "grep foo a.txt | head > out.txt",
        ] {
            assert!(!is_read_only_command(cmd), "must not be read-only: {cmd}");
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
