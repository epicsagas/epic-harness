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
    let mut any_fail = false;

    for r in results {
        let score = r.score;
        total_score += score;
        count += 1;
        if r.verdict == "FAIL" {
            any_fail = true;
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

    let overall_verdict = if any_fail {
        "FAIL"
    } else if dims.values().any(|v| {
        v.get("verdict")
            .and_then(|v| v.as_str())
            .map(|s| s == "WARN")
            .unwrap_or(false)
    }) {
        "WARN"
    } else {
        "PASS"
    };

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
        let score = data
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let verdict = data
            .get("verdict")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

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
        _ => "?",
    };
    println!(
        "  {:<15} {:>7.2}  {} {}",
        "OVERALL", report.overall_score, overall_icon, report.overall_verdict
    );
    println!();
}
