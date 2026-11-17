//! `epic eval` CLI behavior through the real binary: argument validation,
//! baseline loading, persistence, timestamps and recursion. Every run uses a temp
//! HOME, HARNESS_DIR and cwd; commands are small shell scripts in the temp cwd.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SLUG: &str = "fixture_project";
const BENCH: &str = "benchmarks:\n  - name: demo\n    command: \"sh b.sh\"\n";
const NO_REGRESSION: &str = "dimensions:\n  regression:\n    enabled: false\n";

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Env {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::create_dir_all(harness.join("projects").join(SLUG).join("eval")).unwrap();
        Self {
            _temp: temp,
            root,
            cwd,
            home,
            harness,
        }
    }

    fn project_eval_dir(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG).join("eval")
    }

    fn config(&self, yaml: &str) {
        fs::write(self.project_eval_dir().join("eval.yaml"), yaml).unwrap();
    }

    /// A passing benchmark that records that it ran in `ran.txt`.
    fn marker_benchmark(&self, extra_yaml: &str) {
        fs::write(
            self.cwd.join("b.sh"),
            "touch ran.txt\necho '{\"score\": 0.9}'\n",
        )
        .unwrap();
        self.config(&format!("{BENCH}    result_type: composite\n{extra_yaml}"));
    }

    fn ran(&self) -> bool {
        self.cwd.join("ran.txt").exists()
    }

    fn run(&self, args: &[&str]) -> Out {
        self.run_with_path(args, None)
    }

    fn run_with_path(&self, args: &[&str], path: Option<&std::path::Path>) -> Out {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        cmd.arg("eval")
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("EPIC_BIN", env!("CARGO_BIN_EXE_epic-harness"))
            .env("GIT_CEILING_DIRECTORIES", &self.root);
        if let Some(path) = path {
            cmd.env("PATH", path);
        }
        let out = cmd.output().unwrap();
        Out {
            code: out.status.code().unwrap(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }
}

fn names(dir: &std::path::Path, prefix: &str) -> Vec<String> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(prefix))
                .collect()
        })
        .unwrap_or_default()
}

// ── #196 argument validation ─────────────────────────────────────────

#[test]
fn help_prints_usage_without_running_benchmarks() {
    let env = Env::new();
    env.marker_benchmark(NO_REGRESSION);
    for flag in ["--help", "-h"] {
        let out = env.run(&[flag, "--json"]);
        assert_eq!(out.code, 0, "{flag}: {}", out.stderr);
        assert!(out.stdout.contains("usage: epic eval"), "{flag}");
        assert!(!env.ran(), "{flag} executed a benchmark");
    }
}

#[test]
fn unknown_arguments_and_dimensions_are_rejected() {
    let env = Env::new();
    env.marker_benchmark(NO_REGRESSION);
    for args in [
        vec!["--dimension", "benchmakrs", "--json"],
        vec!["--dimension=benchmakrs"],
        vec!["--dimension"],
        vec!["--dimension", "--json"],
        vec!["--bogus"],
        vec!["stray"],
    ] {
        let out = env.run(&args);
        assert_eq!(out.code, 2, "{args:?}: {}", out.stderr);
        assert!(out.stderr.contains("error:"), "{args:?}: {}", out.stderr);
        assert!(!env.ran(), "{args:?} executed a benchmark");
    }
}

#[test]
fn known_arguments_still_run() {
    let env = Env::new();
    env.marker_benchmark(NO_REGRESSION);
    for args in [
        vec!["--json"],
        vec!["--dimension", "benchmarks", "--json"],
        vec!["--dimension=benchmarks", "--json"],
    ] {
        let out = env.run(&args);
        assert_eq!(out.code, 0, "{args:?}: {}", out.stderr);
    }
    assert!(env.ran());
}

// ── #197 baseline loading ────────────────────────────────────────────

#[test]
fn corrupted_baseline_is_an_error_but_missing_is_a_first_run() {
    let env = Env::new();
    env.marker_benchmark("");
    let first = env.run(&["--json"]);
    assert_eq!(first.code, 0, "{}", first.stderr);
    assert!(first.stdout.contains("first run"), "{}", first.stdout);

    let baselines = env.cwd.join("benchmarks/baselines");
    fs::create_dir_all(&baselines).unwrap();
    fs::write(baselines.join("latest.json"), "{bad json").unwrap();
    let out = env.run(&["--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stderr.contains("baseline unreadable"), "{}", out.stderr);
    assert!(out.stderr.contains("latest.json"), "{}", out.stderr);
}

#[test]
fn corrupted_baseline_is_ignored_when_regression_does_not_run() {
    let env = Env::new();
    env.marker_benchmark("");
    let baselines = env.cwd.join("benchmarks/baselines");
    fs::create_dir_all(&baselines).unwrap();
    fs::write(baselines.join("latest.json"), "{bad json").unwrap();
    let out = env.run(&["--dimension", "benchmarks", "--json"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
}

// ── #198 persistence failures ────────────────────────────────────────

#[test]
fn baseline_write_failure_is_reported_not_confirmed() {
    let env = Env::new();
    fs::write(env.cwd.join("blocked"), "regular file").unwrap();
    env.marker_benchmark(
        "dimensions:\n  regression:\n    enabled: false\n    extra:\n      baseline_dir: blocked\n",
    );
    let out = env.run(&["--baseline-update", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stderr.contains("could not save"), "{}", out.stderr);
    assert!(!out.stderr.contains("baseline saved"), "{}", out.stderr);
}

#[test]
fn result_write_failure_is_reported() {
    let env = Env::new();
    env.marker_benchmark(NO_REGRESSION);
    fs::create_dir_all(env.cwd.join("benchmarks")).unwrap();
    fs::write(env.cwd.join("benchmarks/results"), "regular file").unwrap();
    let out = env.run(&["--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stderr.contains("could not save"), "{}", out.stderr);
    // The report itself is still printed.
    assert!(out.stdout.contains("overall_verdict"), "{}", out.stdout);
}

#[test]
fn latest_baseline_failure_keeps_previous_latest() {
    let env = Env::new();
    env.marker_benchmark(NO_REGRESSION);
    let baselines = env.project_eval_dir().join("baselines");
    fs::create_dir_all(baselines.join("latest.json")).unwrap();
    let out = env.run(&["--baseline-update", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(!out.stderr.contains("baseline saved"), "{}", out.stderr);
    assert!(baselines.join("latest.json").is_dir());
    assert!(names(&baselines, ".").iter().all(|n| !n.ends_with(".tmp")));
}

// ── #195 timestamps ──────────────────────────────────────────────────

#[test]
fn timestamps_do_not_need_date_and_runs_stay_distinct() {
    let env = Env::new();
    fs::write(env.cwd.join("b.sh"), "echo '{\"score\": 0.9}'\n").unwrap();
    env.config(&format!(
        "benchmarks:\n  - name: demo\n    command: \"/bin/sh b.sh\"\n    result_type: composite\n{NO_REGRESSION}"
    ));
    // An empty PATH has no `date` executable, as on native Windows.
    let empty = env.root.join("empty-path");
    fs::create_dir_all(&empty).unwrap();
    let mut stamps = Vec::new();
    for _ in 0..2 {
        let out = env.run_with_path(&["--baseline-update", "--json"], Some(&empty));
        assert_eq!(out.code, 0, "{}", out.stderr);
        let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        stamps.push(v["timestamp"].as_str().unwrap().to_string());
    }
    for s in &stamps {
        let (date, time) = s.split_once('T').expect(s);
        assert!(date.len() == 8 && time.len() == 6, "{s}");
        assert!(s.chars().all(|c| c.is_ascii_digit() || c == 'T'), "{s}");
    }
    let results = names(&env.cwd.join("benchmarks/results"), "EVAL-");
    let baselines = names(&env.project_eval_dir().join("baselines"), "BASELINE-");
    assert_eq!(results.len(), 2, "{results:?}");
    assert_eq!(baselines.len(), 2, "{baselines:?}");
    assert!(
        results
            .iter()
            .chain(&baselines)
            .all(|n| !n.contains("unknown"))
    );
}

// ── #350 self-wrapper discovery and recursion ────────────────────────

fn init_yaml(makefile: &str) -> String {
    let env = Env::new();
    fs::write(env.cwd.join("Makefile"), makefile).unwrap();
    let out = env.run(&["--init"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    fs::read_to_string(env.project_eval_dir().join("eval.yaml")).unwrap()
}

#[test]
fn init_skips_make_targets_that_call_epic_eval() {
    for recipe in [
        "\t@epic-harness eval --json\n",
        "\t@echo start\n\tepic eval\n",
        "\t@$(MAKE) lint\n\t@epic   eval --json\n",
    ] {
        let yaml = init_yaml(&format!(".PHONY: eval\neval:\n{recipe}"));
        assert!(!yaml.contains("make_eval"), "{recipe}\n{yaml}");
    }
}

#[test]
fn init_keeps_genuine_make_eval_targets() {
    let genuine = "eval:\n\t@python3 bench.py\n";
    assert!(init_yaml(genuine).contains("make_eval"));
    // Another target's call to `epic eval` is not part of the eval recipe.
    let other = "eval: deps\n\t@python3 bench.py\n\nci:\n\t@epic-harness eval --json\n";
    assert!(init_yaml(other).contains("make_eval"));
}

#[test]
fn recursive_eval_is_stopped_before_launching_another_workload() {
    let env = Env::new();
    // Bounded so a regression of the guard cannot loop forever.
    fs::write(
        env.cwd.join("b.sh"),
        "n=$(cat count 2>/dev/null || echo 0)\n[ \"$n\" -ge 3 ] && exit 0\necho $((n + 1)) > count\n\"$EPIC_BIN\" eval --json\n",
    )
    .unwrap();
    env.config(&format!("{BENCH}{NO_REGRESSION}"));
    let out = env.run(&["--json"]);
    assert_eq!(
        fs::read_to_string(env.cwd.join("count")).unwrap().trim(),
        "1"
    );
    assert_eq!(out.code, 1, "{}", out.stderr);
    assert!(out.stdout.contains("recursively"), "{}", out.stdout);
}

#[test]
fn nested_eval_of_another_config_is_allowed() {
    let env = Env::new();
    // The inner project has its own slug (cwd name), so its eval.yaml differs.
    let inner = env.root.join("inner_project");
    fs::create_dir_all(&inner).unwrap();
    fs::write(inner.join("ok.sh"), "echo '{\"score\": 1.0}'\n").unwrap();
    let inner_cfg = env.harness.join("projects/inner_project/eval");
    fs::create_dir_all(&inner_cfg).unwrap();
    fs::write(
        inner_cfg.join("eval.yaml"),
        format!("benchmarks:\n  - name: inner\n    command: \"sh ok.sh\"\n    result_type: composite\n{NO_REGRESSION}"),
    )
    .unwrap();
    fs::write(
        env.cwd.join("b.sh"),
        "cd ../inner_project && \"$EPIC_BIN\" eval --json\n",
    )
    .unwrap();
    env.config(&format!("{BENCH}{NO_REGRESSION}"));
    let out = env.run(&["--json"]);
    assert_eq!(out.code, 0, "{} {}", out.stdout, out.stderr);
}

// ── #199 init and scaffold JSON output ───────────────────────────────

#[test]
fn init_and_scaffold_json_print_one_document() {
    let mut env = Env::new();
    // A quote and a space in the path must survive JSON escaping.
    env.harness = env.root.join("harness \"root\"");
    fs::create_dir_all(&env.harness).unwrap();

    let out = env.run(&["--init", "--json"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let doc: serde_json::Value = serde_json::from_str(&out.stdout)
        .unwrap_or_else(|e| panic!("init stdout is not one JSON document: {e}: {}", out.stdout));
    assert_eq!(doc["action"], "init");
    let path = PathBuf::from(doc["path"].as_str().unwrap());
    assert!(path.starts_with(&env.harness), "{path:?}");
    assert!(path.exists());

    fs::write(env.cwd.join("package.json"), "{}").unwrap();
    let out = env.run(&["--scaffold", "--json"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let doc: serde_json::Value = serde_json::from_str(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "scaffold stdout is not one JSON document: {e}: {}",
            out.stdout
        )
    });
    assert_eq!(doc["action"], "scaffold");
}
