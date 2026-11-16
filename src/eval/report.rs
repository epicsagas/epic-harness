//! eval/report.rs — Build and display evaluation reports

use serde::Serialize;

use super::runner::DimResult;

/// Complete evaluation report.
#[derive(Debug, Serialize)]
pub struct Report {
    pub timestamp: String,
    pub dimensions: serde_json::Value,
    pub overall_score: f64,
    pub overall_verdict: String,
}

/// Build a report from dimension results and optional baseline.
pub fn build(results: &[DimResult], _prev: &Option<serde_json::Value>) -> Report {
    let timestamp = super::chrono_now();

    let mut dims = serde_json::Map::new();
    let mut total_score = 0.0;
    let mut count = 0usize;

    for r in results {
        let score = r.score;
        if r.verdict != "SKIPPED" {
            total_score += score;
            count += 1;
        }
        dims.insert(
            r.dimension.clone(),
            serde_json::json!({
                "score": score,
                "passed": r.passed,
                "verdict": r.verdict,
                "details": r.details,
                "duration_ms": r.duration_ms,
            }),
        );
    }

    let overall_score = if count > 0 {
        (total_score / count as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };

    let overall_verdict = ["FAIL", "WARN", "PASS"]
        .into_iter()
        .find(|v| results.iter().any(|r| r.verdict == *v))
        .unwrap_or("SKIPPED");

    Report {
        timestamp,
        dimensions: serde_json::Value::Object(dims),
        overall_score,
        overall_verdict: overall_verdict.to_string(),
    }
}

/// Print a human-readable table to stdout.
pub fn print_table(report: &Report) {
    println!("\n  epic eval — quality & regression report");
    println!("  {}\n", "─".repeat(50));

    let dims = match &report.dimensions {
        serde_json::Value::Object(m) => m,
        _ => return,
    };

    println!("  {:<15} {:>8} {:>10}", "Dimension", "Score", "Verdict");
    println!("  {}", "─".repeat(35));

    for (name, data) in dims {
        let score = data.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let verdict = data.get("verdict").and_then(|v| v.as_str()).unwrap_or("?");

        let icon = match verdict {
            "PASS" => "✓",
            "WARN" => "⚠",
            "FAIL" => "✗",
            "SKIPPED" => "○",
            _ => "?",
        };

        println!("  {:<15} {:>7.2}  {} {}", name, score, icon, verdict);
    }

    println!("  {}", "─".repeat(35));

    let overall_icon = match report.overall_verdict.as_str() {
        "PASS" => "✓",
        "WARN" => "⚠",
        "FAIL" => "✗",
        "SKIPPED" => "○",
        _ => "?",
    };
    println!(
        "  {:<15} {:>7.2}  {} {}",
        "OVERALL", report.overall_score, overall_icon, report.overall_verdict
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(name: &str, score: f64, verdict: &str) -> DimResult {
        DimResult {
            dimension: name.into(),
            score,
            passed: verdict == "PASS",
            verdict: verdict.into(),
            details: serde_json::json!({}),
            duration_ms: 0,
        }
    }

    #[test]
    fn a_overall_score_excludes_skipped() {
        let r = build(
            &[
                dim("benchmark:a", 1.0, "PASS"),
                dim("correctness", 0.0, "SKIPPED"),
            ],
            &None,
        );
        assert_eq!(r.overall_score, 1.0);
        assert_eq!(r.overall_verdict, "PASS");
    }

    #[test]
    fn overall_verdict_precedence() {
        let v = |d: &[(&str, &str)]| {
            let rs: Vec<_> = d.iter().map(|(n, x)| dim(n, 1.0, x)).collect();
            build(&rs, &None).overall_verdict
        };
        assert_eq!(v(&[("a", "PASS"), ("b", "WARN"), ("c", "FAIL")]), "FAIL");
        assert_eq!(v(&[("a", "PASS"), ("b", "WARN")]), "WARN");
        assert_eq!(v(&[("a", "SKIPPED"), ("b", "PASS")]), "PASS");
        assert_eq!(v(&[("a", "SKIPPED")]), "SKIPPED");
        assert_eq!(v(&[]), "SKIPPED");
    }
}
