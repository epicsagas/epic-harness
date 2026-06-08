//! eval/mod.rs — Project quality & regression evaluation

mod baseline;
mod config;
mod report;
mod runner;

use crate::shared::paths::harness_dir;

/// Entry point for `epic eval` subcommand.
pub fn run(args: &[String]) -> i32 {
    let json_mode = args.iter().any(|a| a == "--json");
    let init_mode = args.iter().any(|a| a == "--init");
    let baseline_update = args.iter().any(|a| a == "--baseline-update");
    let dimension_filter = parse_flag_str(args, "--dimension");

    let harness = harness_dir();
    let eval_dir = harness.join("eval");

    // --init: scaffold config and exit
    if init_mode {
        return match config::scaffold(&eval_dir) {
            Ok(path) => {
                println!("Scaffolded: {}", path.display());
                if json_mode {
                    println!(r#"{{"action":"init","path":"{}"}}"#, path.display());
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }

    // Load config (fail gracefully if missing)
    let cfg = match config::load(&eval_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}\nRun `epic eval --init` to scaffold config.");
            return 1;
        }
    };

    // Resolve effective commands (auto-detect where needed)
    let effective = match config::resolve_commands(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // Run enabled dimensions
    let mut results = Vec::new();

    let dims = &cfg.dimensions;
    let run_all = dimension_filter.is_none();

    if (run_all || dimension_filter.as_deref() == Some("correctness")) && dims.correctness.enabled {
        results.push(runner::run_correctness(&effective));
    }
    if (run_all || dimension_filter.as_deref() == Some("performance")) && dims.performance.enabled {
        results.push(runner::run_performance(&effective));
    }
    if (run_all || dimension_filter.as_deref() == Some("quality")) && dims.quality.enabled {
        results.push(runner::run_quality(&effective));
    }

    // Load baseline for regression comparison
    let baseline_path = eval_dir.join(
        dims.regression
            .extra
            .baseline_dir
            .as_deref()
            .unwrap_or("eval/baselines"),
    );
    let baseline_path = if baseline_path.is_absolute() {
        baseline_path
    } else {
        harness.join(
            dims.regression
                .extra
                .baseline_dir
                .as_deref()
                .unwrap_or("eval/baselines"),
        )
    };

    let prev = if dims.regression.enabled {
        baseline::load_latest(&baseline_path).ok()
    } else {
        None
    };

    // Compute regression dimension
    if dims.regression.enabled && (run_all || dimension_filter.as_deref() == Some("regression")) {
        results.push(runner::compute_regression(
            &results,
            prev.as_ref(),
            dims.regression.extra.threshold,
        ));
    }

    // Build report
    let rpt = report::build(&results, &prev);

    // --baseline-update: save current as new baseline
    if baseline_update {
        let ts = chrono_now();
        let dir = &baseline_path;
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("warning: could not create baseline dir: {e}");
        }
        let file = dir.join(format!("BASELINE-{ts}.json"));
        let latest = dir.join("latest.json");
        if let Ok(json) = serde_json::to_string_pretty(&rpt) {
            let _ = std::fs::write(&file, &json);
            let _ = std::fs::write(&latest, &json);
            eprintln!("baseline saved: {}", file.display());
        }
    }

    // Save result
    let results_dir = eval_dir.join("results");
    let _ = std::fs::create_dir_all(&results_dir);
    let ts = chrono_now();
    let result_file = results_dir.join(format!("EVAL-{ts}.json"));
    if let Ok(json) = serde_json::to_string_pretty(&rpt) {
        let _ = std::fs::write(&result_file, &json);
    }

    // Output
    if json_mode {
        if let Ok(json) = serde_json::to_string(&rpt) {
            println!("{json}");
        }
    } else {
        report::print_table(&rpt);
    }

    if rpt.overall_verdict == "FAIL" { 1 } else { 0 }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn parse_flag_str(args: &[String], flag: &str) -> Option<String> {
    let eq = format!("{flag}=");
    args.iter()
        .find(|a| a.starts_with(&eq))
        .map(|a| a[eq.len()..].to_string())
        .or_else(|| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        })
}

fn chrono_now() -> String {
    // Use a simple timestamp without chrono dependency
    let output = std::process::Command::new("date")
        .args(["+%Y%m%dT%H%M%S"])
        .output()
        .ok();
    output
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
