//! eval/runner.rs — Execute eval commands and parse results

use std::sync::OnceLock;
use std::time::Instant;

use serde_json::{Value, json};

use super::config::{Benchmark, Better, PerformanceExtra, ResolvedCommands};

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

/// Env var carrying the eval configs on the current call chain (newline separated),
/// so a benchmark that calls `epic eval` again is detected instead of recursing.
pub const CHAIN_ENV: &str = "EPIC_EVAL_CHAIN";

static CHAIN: OnceLock<String> = OnceLock::new();

/// Set the chain passed to every command this process runs.
pub fn set_chain(chain: String) {
    let _ = CHAIN.set(chain);
}

// ── Correctness ─────────────────────────────────────────────────────

pub fn run_correctness(cmds: &ResolvedCommands, min_pass_rate: f64) -> DimResult {
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

    let passed = exit_ok && score >= min_pass_rate;
    let verdict = if !passed {
        "FAIL"
    } else if score >= 1.0 {
        "PASS"
    } else {
        "WARN"
    };

    DimResult {
        dimension: "correctness".into(),
        score,
        passed,
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

pub fn run_performance(
    cmds: &ResolvedCommands,
    extra: &PerformanceExtra,
    baseline: Option<&Value>,
) -> DimResult {
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

    if let Some(name) = &extra.metric {
        return run_performance_metric(bench_cmd, name, extra, baseline, start);
    }

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

/// FAIL for a metric config that `config::load` would have rejected, so a caller
/// that skips validation gets a failed dimension instead of a panic.
fn metric_config_error(bench_cmd: &str, name: &str, message: &str, start: Instant) -> DimResult {
    DimResult {
        dimension: "performance".into(),
        score: 0.0,
        passed: false,
        verdict: "FAIL".into(),
        details: json!({
            "command": bench_cmd,
            "error": message,
            "metric": {"name": name, "status": "invalid_config"},
        }),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Run `trials` times, take the median of the metric, compare to the baseline median.
fn run_performance_metric(
    bench_cmd: &str,
    name: &str,
    extra: &PerformanceExtra,
    baseline: Option<&Value>,
    start: Instant,
) -> DimResult {
    let better = extra.better.unwrap_or(Better::Lower);
    let trials = extra.trials.unwrap_or(3);
    let Some(max) = extra.max_regression_pct else {
        let msg = "performance metric configured without max_regression_pct";
        return metric_config_error(bench_cmd, name, msg, start);
    };

    let mut samples = Vec::new();
    let mut failure = None;
    let mut last = None;
    for _ in 0..trials {
        let out = exec_command(bench_cmd);
        let parsed = if out.exit_code != 0 {
            Err("command_failed")
        } else {
            parse_metric(&out.stdout, name).map_err(|e| match e {
                MetricError::Missing => "metric_missing",
                MetricError::Invalid => "metric_invalid",
            })
        };
        let stderr_tail = truncate(&out.stderr, 200);
        last = Some((out, stderr_tail));
        match parsed {
            Ok(v) => samples.push(v),
            Err(s) => {
                failure = Some(s);
                break;
            }
        }
    }
    let Some((output, stderr_tail)) = last else {
        return metric_config_error(bench_cmd, name, "no trials ran", start);
    };

    let median = failure.is_none().then(|| median(&samples));
    let mut status = failure.unwrap_or("compared");
    let (mut base_median, mut pct, mut regressed) = (None, None, false);
    if let Some(m) = median {
        match baseline_metric(baseline, name, better) {
            Ok(b) => {
                let p = pct_worse(better, b, m);
                regressed = p > max;
                base_median = Some(b);
                // JSON has no non-finite numbers: write null explicitly.
                pct = p.is_finite().then_some(p);
            }
            Err(s) => status = s,
        }
    }

    let passed = !matches!(
        status,
        "command_failed" | "metric_missing" | "metric_invalid"
    ) && !regressed;
    let mut details = json!({
        "command": bench_cmd,
        "exit_code": output.exit_code,
        "stdout_tail": truncate(&output.stdout, 500),
        "metric": {
            "name": name,
            "better": better_str(better),
            "trials": trials,
            "samples": samples,
            "median": median.filter(|m| m.is_finite()),
            "baseline_median": base_median,
            "pct_worse": pct,
            "max_regression_pct": max,
            "regressed": regressed,
            "status": status,
        },
    });
    if status == "command_failed" {
        details["stderr_tail"] = json!(stderr_tail);
    }

    DimResult {
        dimension: "performance".into(),
        score: if passed { 1.0 } else { 0.0 },
        passed,
        verdict: if passed { "PASS" } else { "FAIL" }.into(),
        details,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn better_str(b: Better) -> &'static str {
    match b {
        Better::Lower => "lower",
        Better::Higher => "higher",
    }
}

fn median(samples: &[f64]) -> f64 {
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

/// How much worse `cur` is than `base`, in percent (negative = improvement).
/// Multiplies before dividing so integer inputs hit the boundary exactly.
fn pct_worse(better: Better, base: f64, cur: f64) -> f64 {
    let diff = match better {
        Better::Lower => cur - base,
        Better::Higher => base - cur,
    };
    diff * 100.0 / base.abs()
}

/// Median stored by a previous run, or the status explaining why none is usable.
fn baseline_metric(
    baseline: Option<&Value>,
    name: &str,
    better: Better,
) -> Result<f64, &'static str> {
    let baseline = baseline.ok_or("no_baseline")?;
    let m = &baseline["dimensions"]["performance"]["details"]["metric"];
    let median = m["median"]
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or("no_baseline_metric")?;
    if m["name"] != name || m["better"] != better_str(better) {
        return Err("baseline_mismatch");
    }
    if median == 0.0 {
        return Err("zero_baseline");
    }
    Ok(median)
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

/// A SKIPPED regression result. `reason` is one of
/// "no_baseline" | "no_comparable_dimensions" | "regression_disabled".
pub fn skipped_regression(reason: &str, threshold: f64) -> DimResult {
    DimResult {
        dimension: "regression".into(),
        score: 0.0,
        passed: false,
        verdict: "SKIPPED".into(),
        details: json!({
            "threshold": threshold,
            "compared": 0,
            "reason": reason,
            "deltas": {},
        }),
        duration_ms: 0,
    }
}

pub fn compute_regression(
    results: &[DimResult],
    baseline: Option<&serde_json::Value>,
    threshold: f64,
    fail_on_regression: bool,
) -> DimResult {
    let start = Instant::now();

    let Some(prev) = baseline
        .and_then(|b| b.get("dimensions"))
        .and_then(Value::as_object)
    else {
        let mut r = skipped_regression("no_baseline", threshold);
        r.details["note"] = json!("no previous baseline, first run");
        return r;
    };

    let mut deltas = serde_json::Map::new();
    let mut skipped = serde_json::Map::new();
    let mut any_regression = false;

    for r in results.iter().filter(|r| r.dimension != "regression") {
        let reason = if r.verdict == "SKIPPED" {
            "current_skipped"
        } else if let Some(prev_dim) = prev.get(&r.dimension) {
            if prev_dim.get("verdict").and_then(Value::as_str) == Some("SKIPPED") {
                "baseline_skipped"
            } else {
                match prev_dim.get("score").and_then(Value::as_f64) {
                    Some(prev_score) if prev_score.is_finite() && r.score.is_finite() => {
                        let delta = r.score - prev_score;
                        let regressed = delta < -threshold;
                        any_regression |= regressed;
                        deltas.insert(
                            r.dimension.clone(),
                            json!({
                                "baseline": prev_score,
                                "current": r.score,
                                "delta": format!("{:+.4}", delta),
                                "regressed": regressed,
                            }),
                        );
                        continue;
                    }
                    _ => "non_finite",
                }
            }
        } else {
            "no_baseline_entry"
        };
        skipped.insert(r.dimension.clone(), json!(reason));
    }

    let mut missing_in_current: Vec<&String> = prev
        .keys()
        .filter(|k| *k != "regression" && !results.iter().any(|r| &r.dimension == *k))
        .collect();
    missing_in_current.sort();

    if deltas.is_empty() {
        let mut r = skipped_regression("no_comparable_dimensions", threshold);
        r.details["skipped"] = json!(skipped);
        r.details["missing_in_current"] = json!(missing_in_current);
        r.duration_ms = start.elapsed().as_millis() as u64;
        return r;
    }

    let verdict = match (any_regression, fail_on_regression) {
        (false, _) => "PASS",
        (true, true) => "FAIL",
        (true, false) => "WARN",
    };
    DimResult {
        dimension: "regression".into(),
        score: if any_regression { 0.0 } else { 1.0 },
        passed: verdict != "FAIL",
        verdict: verdict.into(),
        details: json!({
            "threshold": threshold,
            "compared": deltas.len(),
            "deltas": deltas,
            "skipped": skipped,
            "missing_in_current": missing_in_current,
        }),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

// ── Benchmark ───────────────────────────────────────────────────────

/// Run a single configured benchmark command and return its result.
pub fn run_benchmark(bench: &Benchmark) -> DimResult {
    let start = Instant::now();
    let output = exec_command(&bench.command);
    let duration = start.elapsed().as_millis() as u64;
    let exit_ok = output.exit_code == 0;

    // A composite benchmark without a valid score is a FAIL, never a perfect 1.0.
    let (score, score_ok) = match bench.result_type.as_str() {
        "composite" => match parse_composite_score(&output.stdout) {
            Some(s) => (s, true),
            None => (0.0, false),
        },
        _ => (if exit_ok { 1.0 } else { 0.0 }, true),
    };

    let passed = exit_ok && score_ok;
    let verdict = if passed { "PASS" } else { "FAIL" };

    DimResult {
        dimension: format!("benchmark:{}", bench.name),
        score,
        passed,
        verdict: verdict.into(),
        details: json!({
            "command": bench.command,
            "result_type": bench.result_type,
            "exit_code": output.exit_code,
            "score": score,
            "stdout_tail": truncate(&output.stdout, 500),
            "stderr_tail": truncate(&output.stderr, 200),
        }),
        duration_ms: duration,
    }
}

/// Extract the composite/score value from the last JSON object in stdout that
/// carries one. Objects may span lines (pretty-printed) or follow log text.
/// Returns `None` when the score is missing, not a number, or outside 0.0..=1.0.
fn parse_composite_score(stdout: &str) -> Option<f64> {
    last_json_field(stdout, &["composite", "score"])?
        .as_f64()
        .filter(|s| s.is_finite() && (0.0..=1.0).contains(s))
}

/// Value of the first present key, taken from the last JSON object in `stdout`
/// that carries any of `keys` (top level only).
fn last_json_field(stdout: &str, keys: &[&str]) -> Option<Value> {
    let mut last = None;
    let mut rest = stdout;
    while let Some(i) = rest.find('{') {
        let mut stream = serde_json::Deserializer::from_str(&rest[i..]).into_iter::<Value>();
        match stream.next() {
            Some(Ok(v)) => {
                let end = stream.byte_offset();
                if let Some(s) = keys.iter().find_map(|k| v.get(k)) {
                    last = Some(s.clone());
                }
                rest = &rest[i + end..];
            }
            _ => rest = &rest[i + 1..],
        }
    }
    last
}

#[derive(Debug, PartialEq)]
enum MetricError {
    Missing,
    Invalid,
}

fn parse_metric(stdout: &str, name: &str) -> Result<f64, MetricError> {
    let v = last_json_field(stdout, &[name]).ok_or(MetricError::Missing)?;
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or(MetricError::Invalid)
}

// ── Helpers ─────────────────────────────────────────────────────────

struct CmdOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn exec_command(cmd: &str) -> CmdOutput {
    // Split command into program + args for safe execution
    let parts = match shell_words(cmd) {
        Ok(p) => p,
        Err(e) => {
            return CmdOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: e,
            };
        }
    };
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

    let mut command = std::process::Command::new(program);
    command.args(&args);
    if let Some(chain) = CHAIN.get() {
        command.env(CHAIN_ENV, chain);
    }
    let output = command.output();

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

/// Shell word splitting (handles quoted strings). Quotes keep empty arguments
/// (`""`). The only escapes are `\"` and, inside double quotes, `\\`, so
/// Windows paths keep their backslashes. An unmatched quote is an error.
fn shell_words(s: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut has_token = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
        } else if in_double {
            match ch {
                '"' => in_double = false,
                '\\' if matches!(chars.peek(), Some('"' | '\\')) => current.extend(chars.next()),
                _ => current.push(ch),
            }
        } else {
            match ch {
                ' ' | '\t' => {
                    if has_token {
                        words.push(std::mem::take(&mut current));
                        has_token = false;
                    }
                }
                '\'' => {
                    in_single = true;
                    has_token = true;
                }
                '"' => {
                    in_double = true;
                    has_token = true;
                }
                '\\' if chars.peek() == Some(&'"') => {
                    current.extend(chars.next());
                    has_token = true;
                }
                _ => {
                    current.push(ch);
                    has_token = true;
                }
            }
        }
    }
    if in_single || in_double {
        return Err(format!("unmatched quote in command: {s}"));
    }
    if has_token {
        words.push(current);
    }
    Ok(words)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        shell_words(s).unwrap()
    }

    #[test]
    fn shell_words_golden() {
        assert_eq!(words(r#"echo """#), ["echo", ""]);
        assert_eq!(words("echo '' x"), ["echo", "", "x"]);
        assert_eq!(words(r#"cmd "a b" c"#), ["cmd", "a b", "c"]);
        assert_eq!(words(r#"cmd a\"b"#), ["cmd", "a\"b"]);
        assert_eq!(words(r#"cmd "say \"hi\"""#), ["cmd", "say \"hi\""]);
        assert_eq!(words(r#"cmd "C:\\dir""#), ["cmd", r"C:\dir"]);
        assert_eq!(words(r"type C:\dir\x.txt"), ["type", r"C:\dir\x.txt"]);
        assert_eq!(words(r"ls \\server\share"), ["ls", r"\\server\share"]);
        assert_eq!(words("cmd 'it''s'"), ["cmd", "its"]);
        assert_eq!(words("a\tb  c"), ["a", "b", "c"]);
        assert!(words("").is_empty());
        assert!(words(" \t ").is_empty());
    }

    #[test]
    fn shell_words_unmatched_quote_is_an_error() {
        for cmd in [r#"cmd "x"#, "cmd 'x"] {
            let err = shell_words(cmd).unwrap_err();
            assert!(err.contains("unmatched quote"), "{err}");
        }
    }

    #[test]
    fn exec_command_reports_unmatched_quote() {
        let out = exec_command(r#"echo "x"#);
        assert_eq!(out.exit_code, 1);
        assert!(out.stderr.contains("unmatched quote"), "{}", out.stderr);
    }

    fn dim(name: &str, score: f64, verdict: &str) -> DimResult {
        DimResult {
            dimension: name.into(),
            score,
            passed: verdict == "PASS",
            verdict: verdict.into(),
            details: json!({}),
            duration_ms: 0,
        }
    }

    fn baseline(entries: Value) -> Value {
        json!({ "dimensions": entries })
    }

    #[test]
    fn a_missing_in_current_listed() {
        let base = baseline(json!({
            "benchmark:a": {"score": 1.0, "verdict": "PASS"},
            "benchmark:old": {"score": 1.0, "verdict": "PASS"},
        }));
        let r = compute_regression(&[dim("benchmark:a", 1.0, "PASS")], Some(&base), 0.05, true);
        assert_eq!(r.verdict, "PASS");
        assert_eq!(r.details["missing_in_current"], json!(["benchmark:old"]));
    }

    #[test]
    fn a_regression_details_golden() {
        let base = baseline(json!({
            "benchmark:a": {"score": 1.0, "verdict": "PASS"},
            "benchmark:old": {"score": 1.0, "verdict": "PASS"},
            "performance": {"score": 1.0, "verdict": "PASS"},
        }));
        let results = [
            dim("benchmark:a", 0.9, "PASS"),
            dim("performance", 0.0, "SKIPPED"),
        ];
        let r = compute_regression(&results, Some(&base), 0.05, true);
        assert_eq!(r.verdict, "FAIL");
        assert_eq!(
            r.details,
            json!({
                "threshold": 0.05,
                "compared": 1,
                "deltas": { "benchmark:a": { "baseline": 1.0, "current": 0.9, "delta": "-0.1000", "regressed": true } },
                "skipped": { "performance": "current_skipped" },
                "missing_in_current": ["benchmark:old"]
            })
        );
    }

    #[test]
    fn a_regression_grid_property() {
        let steps: Vec<f64> = (0..=20).map(|i| f64::from(i) * 0.05).collect();
        for threshold in [0.0, 0.05, 0.5, 1.0] {
            for &base in &steps {
                let b = baseline(json!({"benchmark:a": {"score": base, "verdict": "PASS"}}));
                for &cur in &steps {
                    let r = compute_regression(
                        &[dim("benchmark:a", cur, "PASS")],
                        Some(&b),
                        threshold,
                        true,
                    );
                    let want = if cur - base < -threshold {
                        "FAIL"
                    } else {
                        "PASS"
                    };
                    assert_eq!(r.verdict, want, "cur={cur} base={base} t={threshold}");
                    let s = compute_regression(
                        &[dim("benchmark:a", cur, "SKIPPED")],
                        Some(&b),
                        threshold,
                        true,
                    );
                    assert_eq!(s.verdict, "SKIPPED");
                }
            }
        }
    }

    #[test]
    fn a_fail_on_regression_flag_sets_fail_or_warn() {
        let base = baseline(json!({"benchmark:a": {"score": 1.0, "verdict": "PASS"}}));
        let cur = [dim("benchmark:a", 0.5, "PASS")];
        assert_eq!(
            compute_regression(&cur, Some(&base), 0.05, true).verdict,
            "FAIL"
        );
        let warn = compute_regression(&cur, Some(&base), 0.05, false);
        assert_eq!(warn.verdict, "WARN");
        assert_eq!(warn.details["deltas"]["benchmark:a"]["regressed"], true);
        // No regression stays PASS regardless of the flag.
        let ok = [dim("benchmark:a", 1.0, "PASS")];
        assert_eq!(
            compute_regression(&ok, Some(&base), 0.05, false).verdict,
            "PASS"
        );
    }

    #[test]
    fn a_non_finite_score_is_skipped_not_compared() {
        let base = baseline(json!({
            "benchmark:a": {"score": 1.0, "verdict": "PASS"},
            "benchmark:b": {"verdict": "PASS"},
            "benchmark:c": {"score": "high", "verdict": "PASS"},
        }));
        let results = [
            dim("benchmark:a", f64::NAN, "PASS"),
            dim("benchmark:b", 1.0, "PASS"),
            dim("benchmark:c", 1.0, "PASS"),
        ];
        let r = compute_regression(&results, Some(&base), 0.05, true);
        assert_eq!(r.verdict, "SKIPPED");
        assert_eq!(r.details["reason"], "no_comparable_dimensions");
        assert_eq!(
            r.details["skipped"],
            json!({
                "benchmark:a": "non_finite",
                "benchmark:b": "non_finite",
                "benchmark:c": "non_finite",
            })
        );
    }

    #[test]
    fn a_baseline_skipped_dimension_not_compared() {
        let base = baseline(json!({
            "performance": {"score": 0.0, "verdict": "SKIPPED"},
            "benchmark:a": {"score": 1.0, "verdict": "PASS"},
        }));
        let results = [
            dim("performance", 1.0, "PASS"),
            dim("benchmark:a", 1.0, "PASS"),
        ];
        let r = compute_regression(&results, Some(&base), 0.05, true);
        assert_eq!(r.verdict, "PASS");
        assert_eq!(r.details["compared"], 1);
        assert_eq!(
            r.details["skipped"],
            json!({"performance": "baseline_skipped"})
        );
    }

    #[test]
    fn a_no_baseline_entry_for_dimension() {
        let base = baseline(json!({"benchmark:a": {"score": 1.0, "verdict": "PASS"}}));
        let results = [
            dim("benchmark:a", 1.0, "PASS"),
            dim("benchmark:new", 0.0, "FAIL"),
        ];
        let r = compute_regression(&results, Some(&base), 0.05, true);
        assert_eq!(r.verdict, "PASS");
        assert_eq!(
            r.details["skipped"],
            json!({"benchmark:new": "no_baseline_entry"})
        );
    }

    #[test]
    fn a_all_baseline_skipped_is_no_comparable() {
        let base = baseline(json!({
            "benchmark:a": {"score": 0.0, "verdict": "SKIPPED"},
            "performance": {"score": 0.0, "verdict": "SKIPPED"},
        }));
        let results = [
            dim("benchmark:a", 1.0, "PASS"),
            dim("performance", 1.0, "PASS"),
        ];
        let r = compute_regression(&results, Some(&base), 0.05, true);
        assert_eq!(r.verdict, "SKIPPED");
        assert_eq!(r.details["reason"], "no_comparable_dimensions");
        assert_eq!(r.details["compared"], 0);
        assert_eq!(
            r.details["skipped"],
            json!({"benchmark:a": "baseline_skipped", "performance": "baseline_skipped"})
        );
    }

    fn perf_with(extra: PerformanceExtra) -> DimResult {
        let cmds = ResolvedCommands {
            test_command: None,
            lint_command: None,
            bench_command: Some("false".into()),
            mutation_command: None,
            stack: "auto".into(),
        };
        run_performance(&cmds, &extra, None)
    }

    #[test]
    fn p_metric_without_max_regression_pct_fails_not_panics() {
        let r = perf_with(PerformanceExtra {
            metric: Some("m".into()),
            ..Default::default()
        });
        assert_eq!(r.verdict, "FAIL");
        assert!(!r.passed);
        assert_eq!(r.score, 0.0);
        assert_eq!(
            r.details["error"],
            "performance metric configured without max_regression_pct"
        );
    }

    #[test]
    fn p_metric_with_zero_trials_fails_not_panics() {
        let r = perf_with(PerformanceExtra {
            metric: Some("m".into()),
            max_regression_pct: Some(20.0),
            trials: Some(0),
            ..Default::default()
        });
        assert_eq!(r.verdict, "FAIL");
        assert!(!r.passed);
        assert_eq!(r.details["error"], "no trials ran");
    }

    #[test]
    fn p_parse_metric_golden() {
        use MetricError::*;
        let cases: [(&str, Result<f64, MetricError>); 9] = [
            (r#"{"latency_ms": 7.5}"#, Ok(7.5)),
            ("log line\n{\n \"latency_ms\": 8\n}", Ok(8.0)),
            ("{\"latency_ms\":1}\n{\"latency_ms\":2}", Ok(2.0)),
            ("{\"latency_ms\":1}\n{\"other\":2}", Ok(1.0)),
            (r#"{"other":1}"#, Err(Missing)),
            (r#"{"latency_ms":"12"}"#, Err(Invalid)),
            (r#"{"latency_ms":null}"#, Err(Invalid)),
            (r#"{"m":{"latency_ms":1}}"#, Err(Missing)),
            ("", Err(Missing)),
        ];
        for (stdout, want) in cases {
            assert_eq!(parse_metric(stdout, "latency_ms"), want, "{stdout:?}");
        }
        assert_eq!(
            parse_metric(r#"{"latency_ms":true}"#, "latency_ms"),
            Err(Invalid)
        );
    }

    /// Pre-refactor implementation, kept verbatim as the differential oracle.
    fn composite_before(stdout: &str) -> Option<f64> {
        let mut last = None;
        let mut rest = stdout;
        while let Some(i) = rest.find('{') {
            let mut stream = serde_json::Deserializer::from_str(&rest[i..]).into_iter::<Value>();
            match stream.next() {
                Some(Ok(v)) => {
                    let end = stream.byte_offset();
                    if let Some(s) = v.get("composite").or_else(|| v.get("score")) {
                        last = Some(s.clone());
                    }
                    rest = &rest[i + end..];
                }
                _ => rest = &rest[i + 1..],
            }
        }
        last?
            .as_f64()
            .filter(|s| s.is_finite() && (0.0..=1.0).contains(s))
    }

    #[test]
    fn p_composite_unchanged_differential() {
        let cases = [
            "",
            "no json here",
            r#"{"composite": 0.75}"#,
            r#"{"score": 0.5}"#,
            r#"{"composite": 0.9, "score": 0.1}"#,
            r#"{"score": 0.1, "composite": 0.9}"#,
            r#"{"composite": 0.2}{"score": 0.8}"#,
            r#"{"composite": 0.2}{"other": 1}"#,
            "log line\n{\n  \"composite\": 0.75,\n  \"detail\": {\"a\": 1}\n}\n",
            r#"{"composite": 1.5}"#,
            r#"{"composite": -0.1}"#,
            r#"{"composite": "0.5"}"#,
            r#"{"composite": null}"#,
            r#"{"composite": null, "score": 0.5}"#,
            r#"{"composite": 1e999}"#,
            r#"{"detail": {"score": 0.5}}"#,
            r#"{"score": 0.3} trailing {"score": 0.6}"#,
            r#"{broken {"score": 0.4}"#,
            r#"{"composite": 0.1} {"composite": 2.0}"#,
            r#"{"score": 1.0}"#,
        ];
        for c in cases {
            assert_eq!(parse_composite_score(c), composite_before(c), "{c:?}");
        }
    }

    #[test]
    fn p_median_golden() {
        assert_eq!(median(&[3.0]), 3.0);
        assert_eq!(median(&[1.0, 3.0]), 2.0);
        assert_eq!(median(&[5.0, 1.0, 3.0]), 3.0);
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    #[test]
    fn p_pct_worse_boundary() {
        let regressed = |b, base, cur, max: f64| pct_worse(b, base, cur) > max;
        assert_eq!(pct_worse(Better::Lower, 100.0, 120.0), 20.0);
        assert!(!regressed(Better::Lower, 100.0, 120.0, 20.0));
        assert!(regressed(Better::Lower, 100.0, 120.5, 20.0));
        assert_eq!(pct_worse(Better::Higher, 100.0, 80.0), 20.0);
        assert!(!regressed(Better::Higher, 100.0, 80.0, 20.0));
        assert!(regressed(Better::Higher, 100.0, 79.5, 20.0));
        assert_eq!(pct_worse(Better::Lower, 100.0, 50.0), -50.0);
        assert!(!regressed(Better::Lower, 100.0, 50.0, 20.0));
    }

    #[test]
    fn p_pct_worse_monotonic() {
        for better in [Better::Lower, Better::Higher] {
            for base in [0.001, 1.0, 7.67, 100.0, 1e6, -5.0] {
                for max in [0.5, 5.0, 20.0, 100.0, 1000.0] {
                    assert!(pct_worse(better, base, base) <= max);
                    if base < 0.0 {
                        continue;
                    }
                    // r from 0.0 to 3.0, best to worst for the direction.
                    let mut rs: Vec<f64> = (0..=60).map(|i| i as f64 * 0.05).collect();
                    if better == Better::Higher {
                        rs.reverse();
                    }
                    let flags: Vec<bool> = rs
                        .iter()
                        .map(|r| pct_worse(better, base, base * r) > max)
                        .collect();
                    assert!(
                        flags.windows(2).all(|w| !w[0] || w[1]),
                        "{better:?} {base} {max}"
                    );
                }
            }
        }
    }

    #[test]
    fn p_baseline_metric_table() {
        let mk =
            |m: Value| Some(json!({"dimensions": {"performance": {"details": {"metric": m}}}}));
        let row = |name: &str, better: &str, median: Value| {
            mk(json!({"name": name, "better": better, "median": median}))
        };
        let cases: [(Option<Value>, Result<f64, &str>); 12] = [
            (None, Err("no_baseline")),
            (Some(json!({})), Err("no_baseline_metric")),
            (
                Some(json!({"dimensions": {"performance": {"details": {}}}})),
                Err("no_baseline_metric"),
            ),
            (mk(json!("x")), Err("no_baseline_metric")),
            (
                mk(json!({"name": "m", "better": "lower"})),
                Err("no_baseline_metric"),
            ),
            (row("m", "lower", json!(null)), Err("no_baseline_metric")),
            (row("m", "lower", json!("x")), Err("no_baseline_metric")),
            (
                row("m", "lower", json!("Infinity")),
                Err("no_baseline_metric"),
            ),
            (row("other", "lower", json!(10.0)), Err("baseline_mismatch")),
            (row("m", "higher", json!(10.0)), Err("baseline_mismatch")),
            (row("m", "lower", json!(0.0)), Err("zero_baseline")),
            (row("m", "lower", json!(10.0)), Ok(10.0)),
        ];
        for (b, want) in cases {
            assert_eq!(
                baseline_metric(b.as_ref(), "m", Better::Lower),
                want,
                "{b:?}"
            );
        }
    }
}
