//! eval/runner.rs — Execute eval commands and parse results

use std::time::Instant;

use serde_json::{Value, json};

use super::config::ResolvedCommands;

/// Single dimension evaluation result.
#[derive(Debug, Clone)]
pub struct DimResult {
    pub dimension: String,
    pub score: f64,
    pub passed: bool,
    pub verdict: String, // PASS, WARN, FAIL, SKIPPED
    pub details: Value,
    pub duration_ms: u64,
}

// ── Correctness ─────────────────────────────────────────────────────

pub fn run_correctness(cmds: &ResolvedCommands) -> DimResult {
    let start = Instant::now();

    let test_cmd = match &cmds.test_command {
        Some(c) => c,
        None => {
            return DimResult {
                dimension: "correctness".into(),
                score: 0.0,
                passed: false,
                verdict: "SKIPPED".into(),
                details: json!({"reason": "no test command configured or detected"}),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let output = exec_command(test_cmd);
    let duration = start.elapsed().as_millis() as u64;

    let (tests_passed, tests_total) = parse_test_output(&output.stdout, &cmds.stack);
    let exit_ok = output.exit_code == 0;

    let score = if tests_total > 0 {
        tests_passed as f64 / tests_total as f64
    } else if exit_ok {
        1.0
    } else {
        0.0
    };

    let verdict = if score >= 1.0 {
        "PASS"
    } else if score >= 0.8 {
        "WARN"
    } else {
        "FAIL"
    };

    DimResult {
        dimension: "correctness".into(),
        score,
        passed: exit_ok,
        verdict: verdict.into(),
        details: json!({
            "command": test_cmd,
            "exit_code": output.exit_code,
            "tests_passed": tests_passed,
            "tests_total": tests_total,
            "stdout_tail": truncate(&output.stdout, 500),
            "stderr_tail": truncate(&output.stderr, 500),
        }),
        duration_ms: duration,
    }
}

// ── Performance ─────────────────────────────────────────────────────

pub fn run_performance(cmds: &ResolvedCommands) -> DimResult {
    let start = Instant::now();

    let bench_cmd = match &cmds.bench_command {
        Some(c) => c,
        None => {
            return DimResult {
                dimension: "performance".into(),
                score: 0.0,
                passed: false,
                verdict: "SKIPPED".into(),
                details: json!({"reason": "no bench command configured or detected"}),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let output = exec_command(bench_cmd);
    let duration = start.elapsed().as_millis() as u64;

    let exit_ok = output.exit_code == 0;

    DimResult {
        dimension: "performance".into(),
        score: if exit_ok { 1.0 } else { 0.0 },
        passed: exit_ok,
        verdict: if exit_ok { "PASS" } else { "FAIL" }.into(),
        details: json!({
            "command": bench_cmd,
            "exit_code": output.exit_code,
            "stdout_tail": truncate(&output.stdout, 500),
        }),
        duration_ms: duration,
    }
}

// ── Quality ─────────────────────────────────────────────────────────

pub fn run_quality(cmds: &ResolvedCommands) -> DimResult {
    let start = Instant::now();

    let lint_cmd = match &cmds.lint_command {
        Some(c) => c,
        None => {
            return DimResult {
                dimension: "quality".into(),
                score: 0.0,
                passed: false,
                verdict: "SKIPPED".into(),
                details: json!({"reason": "no lint command configured or detected"}),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let output = exec_command(lint_cmd);
    let duration = start.elapsed().as_millis() as u64;

    let exit_ok = output.exit_code == 0;

    // Lint score: 1.0 if clean, 0.0 if errors
    let lint_score = if exit_ok { 1.0 } else { 0.0 };

    // LLM-as-judge is handled by SKILL.md (LLM session). CLI marks it as deferred.
    DimResult {
        dimension: "quality".into(),
        score: lint_score,
        passed: exit_ok,
        verdict: if exit_ok { "PASS" } else { "FAIL" }.into(),
        details: json!({
            "command": lint_cmd,
            "exit_code": output.exit_code,
            "lint_score": lint_score,
            "llm_judge": "SKIPPED",
            "stdout_tail": truncate(&output.stdout, 500),
            "stderr_tail": truncate(&output.stderr, 500),
        }),
        duration_ms: duration,
    }
}

// ── Regression ──────────────────────────────────────────────────────

pub fn compute_regression(
    results: &[DimResult],
    baseline: Option<&serde_json::Value>,
    threshold: f64,
) -> DimResult {
    let start = Instant::now();

    let Some(prev) = baseline.and_then(|b| b.get("dimensions")) else {
        return DimResult {
            dimension: "regression".into(),
            score: 1.0,
            passed: true,
            verdict: "PASS".into(),
            details: json!({"note": "no previous baseline — first run"}),
            duration_ms: start.elapsed().as_millis() as u64,
        };
    };

    let mut deltas = serde_json::Map::new();
    let mut any_regression = false;

    for r in results {
        if let Some(prev_dim) = prev.get(&r.dimension) {
            let prev_score = prev_dim
                .get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let delta = r.score - prev_score;
            let regressed = delta < -threshold;
            if regressed {
                any_regression = true;
            }
            deltas.insert(
                r.dimension.clone(),
                json!({
                    "baseline": prev_score,
                    "current": r.score,
                    "delta": format!("{:+.4}", delta),
                    "regressed": regressed,
                }),
            );
        }
    }

    let score = if any_regression { 0.0 } else { 1.0 };

    DimResult {
        dimension: "regression".into(),
        score,
        passed: !any_regression,
        verdict: if any_regression { "FAIL" } else { "PASS" }.into(),
        details: json!({ "deltas": deltas, "threshold": threshold }),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

struct CmdOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn exec_command(cmd: &str) -> CmdOutput {
    // Split command into program + args for safe execution
    let parts = shell_words(cmd);
    let (program, args) = match parts.split_first() {
        Some((p, a)) => (p.as_str(), a.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
        None => {
            return CmdOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("empty command: {cmd}"),
            };
        }
    };

    let output = std::process::Command::new(program).args(&args).output();

    match output {
        Ok(o) => CmdOutput {
            exit_code: o.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => CmdOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("failed to execute '{cmd}': {e}"),
        },
    }
}

/// Simple shell word splitting (handles quoted strings).
fn shell_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in s.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Parse test output for pass/fail counts.
fn parse_test_output(stdout: &str, stack: &str) -> (usize, usize) {
    match stack {
        "rust" => parse_rust_test_output(stdout),
        "node" => parse_node_test_output(stdout),
        "python" => parse_python_test_output(stdout),
        "go" => parse_go_test_output(stdout),
        _ => (0, 0),
    }
}

fn parse_rust_test_output(stdout: &str) -> (usize, usize) {
    // "test result: ok. 142 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    for line in stdout.lines().rev() {
        if line.contains("test result:") {
            let passed = extract_number(line, "passed").unwrap_or(0);
            let failed = extract_number(line, "failed").unwrap_or(0);
            return (passed, passed + failed);
        }
    }
    (0, 0)
}

fn parse_node_test_output(stdout: &str) -> (usize, usize) {
    // "Tests:  142 passed, 0 failed" (vitest/jest)
    for line in stdout.lines().rev() {
        if line.contains("passed") {
            let passed = extract_number(line, "passed").unwrap_or(0);
            let failed = extract_number(line, "failed").unwrap_or(0);
            return (passed, passed + failed);
        }
    }
    (0, 0)
}

fn parse_python_test_output(stdout: &str) -> (usize, usize) {
    // "142 passed, 0 failed" (pytest)
    for line in stdout.lines().rev() {
        if line.contains("passed") {
            let passed = extract_number(line, "passed").unwrap_or(0);
            let failed = extract_number(line, "failed")
                .or_else(|| extract_number(line, "error").or(Some(0)))
                .unwrap_or(0);
            return (passed, passed + failed);
        }
    }
    (0, 0)
}

fn parse_go_test_output(stdout: &str) -> (usize, usize) {
    // "ok  github.com/pkg  0.123s" or "FAIL  github.com/pkg  0.123s"
    let ok_count = stdout.lines().filter(|l| l.starts_with("ok\t")).count();
    let fail_count = stdout.lines().filter(|l| l.starts_with("FAIL\t")).count();
    (ok_count, ok_count + fail_count)
}

fn extract_number(haystack: &str, needle: &str) -> Option<usize> {
    // Find "needle" and extract the number just before it
    let needle_with_space = format!(" {needle}");
    if let Some(pos) = haystack.rfind(&needle_with_space) {
        let before = &haystack[..pos];
        let num_str: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return num_str.parse().ok();
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find a safe char boundary for the tail
    let end = s.len() - max;
    let end = (0..=end)
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    format!("...{}", &s[end..])
}
