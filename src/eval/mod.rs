//! eval/mod.rs — Project quality & regression evaluation

mod baseline;
mod config;
mod report;
mod runner;
mod scaffold;

use crate::shared::paths::harness_dir;

/// Entry point for `epic eval` subcommand.
pub fn run(args: &[String]) -> i32 {
    let json_mode = args.iter().any(|a| a == "--json");
    let init_mode = args.iter().any(|a| a == "--init");
    let scaffold_mode = args.iter().any(|a| a == "--scaffold");
    let baseline_update = args.iter().any(|a| a == "--baseline-update");
    let dimension_filter = parse_flag_str(args, "--dimension");

    let harness = harness_dir();
    let eval_dir = harness.join("eval");
    let cwd = std::env::current_dir().unwrap_or_default();

    // --scaffold: generate benchmark files for detected stack
    if scaffold_mode {
        let stack = config::detect_stack(&cwd);
        eprintln!("[eval] detected stack: {stack}");
        return match scaffold::scaffold_benchmarks(&cwd, &stack) {
            Ok(result) => {
                for path in &result.created {
                    println!("created: {}", path.display());
                }
                for path in &result.skipped {
                    println!("skipped (exists): {}", path.display());
                }
                if result.created.is_empty() {
                    println!("nothing to create — all benchmark files already exist");
                } else {
                    println!(
                        "\n{} file(s) created for stack '{}'",
                        result.created.len(),
                        result.stack
                    );
                    println!("Next: review the generated files, fill in TODO sections, then run:");
                    println!("  epic eval --init   # scaffold eval.yaml if not yet done");
                    println!("  epic eval          # run evaluation");
                }
                if json_mode {
                    let created: Vec<_> = result
                        .created
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    let skipped: Vec<_> = result
                        .skipped
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    if let Ok(j) = serde_json::to_string(&serde_json::json!({
                        "action": "scaffold",
                        "stack": result.stack,
                        "created": created,
                        "skipped": skipped,
                    })) {
                        println!("{j}");
                    }
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }

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

    // Warn if benchmark files exist but none are configured
    check_unconfigured_benchmarks(&cwd, &cfg);

    // Resolve effective commands (explicit config only — no stack defaults)
    let effective = match config::resolve_commands(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // Run enabled dimensions (explicit commands only — build/test/lint belong to verify)
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

    // Run configured project benchmarks
    if run_all || dimension_filter.as_deref() == Some("benchmarks") {
        for bench in &cfg.benchmarks {
            results.push(runner::run_benchmark(bench));
        }
    }

    // Resolve baseline directory: in-repo benchmarks/baselines/ first, then harness-local
    let baseline_path = resolve_baseline_dir(&cwd, &harness, dims);

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

    // Resolve results directory: in-repo benchmarks/results/ if benchmarks configured
    let results_dir = if !cfg.benchmarks.is_empty() {
        cwd.join("benchmarks").join("results")
    } else {
        eval_dir.join("results")
    };
    if let Err(error) = persist_report(
        &rpt,
        &results_dir,
        baseline_update.then_some(&baseline_path),
    ) {
        eprintln!("error: could not save evaluation: {error}");
        return 1;
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

fn persist_report(
    report: &report::Report,
    results_dir: &std::path::Path,
    baseline_dir: Option<&std::path::PathBuf>,
) -> Result<(), String> {
    use std::io::Write;
    let json = serde_json::to_vec_pretty(report).map_err(|e| format!("serialize report: {e}"))?;
    let archive = |path: &std::path::Path| -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("create {}: {e}", path.display()))?;
        file.write_all(&json)
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("write {}: {e}", path.display()))
    };
    std::fs::create_dir_all(results_dir).map_err(|e| format!("results directory: {e}"))?;
    archive(&results_dir.join(format!("EVAL-{}.json", report.timestamp)))?;
    if let Some(dir) = baseline_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("baseline directory: {e}"))?;
        let file = dir.join(format!("BASELINE-{}.json", report.timestamp));
        archive(&file)?;
        // Publish latest only after a complete durable archive. A failed
        // replacement leaves the previous latest baseline intact.
        let temp = dir.join(format!(".latest-{}.tmp", uuid::Uuid::new_v4().simple()));
        let result = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(&json)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&temp, dir.join("latest.json"))
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("publish latest baseline: {error}"));
        }
        eprintln!("baseline saved: {}", file.display());
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Resolve the baseline directory. Priority:
/// 1. Explicit `baseline_dir` in config (absolute or CWD-relative)
/// 2. In-repo `{cwd}/benchmarks/baselines/` if benchmarks are configured
/// 3. Harness-local `~/.harness/eval/baselines/` as fallback
fn resolve_baseline_dir(
    cwd: &std::path::Path,
    harness: &std::path::Path,
    dims: &config::Dimensions,
) -> std::path::PathBuf {
    if let Some(dir) = &dims.regression.extra.baseline_dir {
        let p = std::path::Path::new(dir);
        return if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        };
    }
    // Default: in-repo benchmarks/baselines/
    let repo = cwd.join("benchmarks").join("baselines");
    if repo.exists() {
        return repo;
    }
    // Fallback: harness-local
    harness.join("eval").join("baselines")
}

/// Warn if benchmark infrastructure exists in the project but eval.yaml has no benchmarks.
fn check_unconfigured_benchmarks(cwd: &std::path::Path, cfg: &config::EvalConfig) {
    if !cfg.benchmarks.is_empty() {
        return;
    }
    let candidates = [cwd.join("benchmarks").join("eval_runner.py")];
    for path in &candidates {
        if path.exists() {
            eprintln!(
                "warning: {} exists but no benchmarks are configured in eval.yaml\n\
                 Run `epic eval --init` (on a fresh config) or add entries to `benchmarks:` manually.",
                path.display()
            );
            break;
        }
    }
    // Also check Makefile/justfile for eval target
    if let Ok(content) = std::fs::read_to_string(cwd.join("Makefile"))
        && content
            .lines()
            .any(|l| l.starts_with("eval:") || l.starts_with("eval :"))
    {
        eprintln!(
            "warning: Makefile has an `eval` target but no benchmarks are configured in eval.yaml"
        );
    }
}

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
    let utc = crate::shared::helpers::now_iso().replace(['-', ':'], "");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Portable UTC plus a uniqueness suffix: no executable dependency and no
    // same-second overwrite, including files created by concurrent processes.
    format!("{utc}_{nanos:09}_{}", uuid::Uuid::new_v4().simple())
}
