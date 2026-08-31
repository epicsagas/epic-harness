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
