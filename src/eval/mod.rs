//! eval/mod.rs — Project quality & regression evaluation

mod baseline;
mod config;
mod report;
mod runner;
mod scaffold;

use crate::shared::paths::harness_dir;

const DIMENSIONS: [&str; 5] = [
    "correctness",
    "performance",
    "quality",
    "benchmarks",
    "regression",
];

const USAGE: &str = "usage: epic eval [--json] [--init] [--scaffold] [--baseline-update] [--dimension <dim>]\n  dim: correctness | performance | quality | benchmarks | regression";

/// Reject unknown arguments and bad `--dimension` values before anything runs.
fn validate_args(args: &[String]) -> Result<(), String> {
    let check = |dim: &str| {
        if DIMENSIONS.contains(&dim) {
            Ok(())
        } else {
            Err(format!("unknown dimension: {dim}"))
        }
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" | "--init" | "--scaffold" | "--baseline-update" => {}
            "--dimension" => check(rest.next().ok_or("--dimension requires a value")?)?,
            a if a.starts_with("--dimension=") => check(&a["--dimension=".len()..])?,
            a => return Err(format!("unknown argument: {a}")),
        }
    }
    Ok(())
}

/// Entry point for `epic eval` subcommand.
pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return 0;
    }
    if let Err(e) = validate_args(args) {
        eprintln!("error: {e}\n{USAGE}");
        return 2;
    }
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
                if !json_mode {
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
                        println!(
                            "Next: review the generated files, fill in TODO sections, then run:"
                        );
                        println!("  epic eval --init   # scaffold eval.yaml if not yet done");
                        println!("  epic eval          # run evaluation");
                    }
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
                if json_mode {
                    println!(
                        "{}",
                        serde_json::json!({"action": "init", "path": path.display().to_string()})
                    );
                } else {
                    println!("Scaffolded: {}", path.display());
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

    for w in config::warnings(&cfg) {
        eprintln!("warning: {w}");
    }

    if let Err(e) = enter_context(&eval_dir) {
        eprintln!("error: {e}");
        return 1;
    }

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
    // `--dimension regression` measures first: it runs every producer, like an unfiltered run.
    let regression_only = dimension_filter.as_deref() == Some("regression");
    let producers = run_all || (regression_only && dims.regression.enabled);
    if regression_only && !dims.regression.enabled {
        results.push(runner::skipped_regression(
            "regression_disabled",
            dims.regression.extra.threshold,
        ));
    }

    // Resolve baseline directory: in-repo benchmarks/baselines/ first, then harness-local
    let baseline_path = resolve_baseline_dir(&cwd, &harness, dims);

    // A missing baseline is a first run; an unreadable one must not pass silently.
    let regression_runs = dims.regression.enabled && (run_all || regression_only);
    let prev = if regression_runs {
        match baseline::load_latest(&baseline_path) {
            Ok(prev) => prev,
            Err(e) => {
                eprintln!(
                    "error: baseline unreadable: {e}\nFix or delete the file, then run `epic eval --baseline-update`."
                );
                return 1;
            }
        }
    } else {
        None
    };

    if (producers || dimension_filter.as_deref() == Some("correctness")) && dims.correctness.enabled
    {
        results.push(runner::run_correctness(
            &effective,
            dims.correctness.extra.min_pass_rate,
        ));
    }
    if (producers || dimension_filter.as_deref() == Some("performance")) && dims.performance.enabled
    {
        results.push(runner::run_performance(&effective));
    }
    if (producers || dimension_filter.as_deref() == Some("quality")) && dims.quality.enabled {
        results.push(runner::run_quality(&effective));
    }

    // Run configured project benchmarks
    if producers || dimension_filter.as_deref() == Some("benchmarks") {
        for bench in &cfg.benchmarks {
            results.push(runner::run_benchmark(bench));
        }
    }

    // Compute regression dimension
    if regression_runs {
        results.push(runner::compute_regression(
            &results,
            prev.as_ref(),
            dims.regression.extra.threshold,
            dims.regression.extra.fail_on_regression,
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
    let nothing_measured = rpt.overall_verdict == "SKIPPED";
    if baseline_update && nothing_measured {
        eprintln!("warning: nothing was measured (overall SKIPPED); baseline not saved");
    }
    let saved = persist(
        &rpt,
        &results_dir,
        (baseline_update && !nothing_measured).then_some(baseline_path.as_path()),
    );

    // Output
    if json_mode {
        if let Ok(json) = serde_json::to_string(&rpt) {
            println!("{json}");
        }
    } else {
        report::print_table(&rpt);
    }

    if let Err(e) = saved {
        eprintln!("error: could not save evaluation: {e}");
        return 1;
    }

    if rpt.overall_verdict == "FAIL" { 1 } else { 0 }
}

/// Write the baseline (when requested) and the result file. Success is reported
/// only after the writes it describes have completed.
fn persist(
    rpt: &report::Report,
    results_dir: &std::path::Path,
    baseline_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(rpt).map_err(|e| format!("serialize report: {e}"))?;
    let id = run_id();
    if let Some(dir) = baseline_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let file = dir.join(format!("BASELINE-{id}.json"));
        write_atomic(&file, &json)?;
        write_atomic(&dir.join("latest.json"), &json)?;
        eprintln!("baseline saved: {}", file.display());
    }
    std::fs::create_dir_all(results_dir)
        .map_err(|e| format!("create {}: {e}", results_dir.display()))?;
    write_atomic(&results_dir.join(format!("EVAL-{id}.json")), &json)
}

/// Write via a temp file and rename so a failed write never leaves a partial file.
fn write_atomic(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)
        .and_then(|()| std::fs::rename(&tmp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
        .map_err(|e| format!("write {}: {e}", path.display()))
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Refuse to run when this config is already being evaluated further up the process
/// tree (a benchmark or wrapper that calls `epic eval`). Evaluations of other
/// configs may still nest. Otherwise record this config for the commands we run.
fn enter_context(eval_dir: &std::path::Path) -> Result<(), String> {
    let config = eval_dir.join("eval.yaml");
    let key = std::fs::canonicalize(&config)
        .unwrap_or(config)
        .to_string_lossy()
        .into_owned();
    let mut chain = std::env::var(runner::CHAIN_ENV).unwrap_or_default();
    if chain.lines().any(|c| c == key) {
        return Err(format!(
            "eval is already running for {key}; a benchmark or wrapper calls `epic eval` recursively"
        ));
    }
    if !chain.is_empty() {
        chain.push('\n');
    }
    chain.push_str(&key);
    runner::set_chain(chain);
    Ok(())
}

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
    // Also check Makefile for eval target
    if config::has_eval_target(&cwd.join("Makefile")) {
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

/// UTC timestamp as `YYYYMMDDTHHMMSS`, from std time (no external `date`).
fn chrono_now() -> String {
    crate::shared::helpers::now_iso()
        .replace(['-', ':'], "")
        .trim_end_matches('Z')
        .to_string()
}

/// Timestamp plus sub-second nanos: file names stay distinct and ordered across runs.
fn run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{}-{nanos:09}", chrono_now())
}
