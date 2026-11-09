//! eval/config.rs — Eval configuration loading and stack detection

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalConfig {
    /// Informational only — used by `verify`, not by eval itself.
    #[serde(default = "default_stack")]
    pub stack: String,
    #[serde(default)]
    pub dimensions: Dimensions,
    /// Project-specific benchmark commands. Auto-detected by `--init`.
    #[serde(default)]
    pub benchmarks: Vec<Benchmark>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dimensions {
    /// Run a custom correctness command (no default — delegate to verify).
    #[serde(default)]
    pub correctness: DimensionConfig<CorrectnessExtra>,
    #[serde(default)]
    pub performance: DimensionConfig<PerformanceExtra>,
    /// Run a custom quality command (no default — delegate to verify).
    #[serde(default)]
    pub quality: DimensionConfig<QualityExtra>,
    #[serde(default = "default_regression")]
    pub regression: DimensionConfig<RegressionExtra>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DimensionConfig<E> {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub extra: E,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CorrectnessExtra {
    #[serde(default)]
    pub mutation_tool: Option<String>,
    #[serde(default = "default_pass_rate")]
    pub min_pass_rate: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PerformanceExtra {
    #[serde(default = "default_regression_pct")]
    pub max_regression_pct: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QualityExtra {
    #[serde(default = "default_true")]
    pub llm_judge: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegressionExtra {
    /// Baseline directory. `None` = auto-resolve (in-repo `benchmarks/baselines/` first).
    #[serde(default)]
    pub baseline_dir: Option<String>,
    #[serde(default = "default_true")]
    pub fail_on_regression: bool,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Benchmark {
    pub name: String,
    /// Shell command to execute.
    pub command: String,
    /// How to interpret the result:
    /// - `"exit_code"` (default): 0 = PASS, non-zero = FAIL, score 1.0 / 0.0
    /// - `"composite"`: parse JSON stdout for `composite` or `score` field (0.0–1.0)
    #[serde(default = "default_result_type")]
    pub result_type: String,
}

// ── Resolved commands (after auto-detection) ────────────────────────

pub struct ResolvedCommands {
    /// Explicit correctness command only — no stack-based default.
    pub test_command: Option<String>,
    /// Explicit quality/lint command only — no stack-based default.
    pub lint_command: Option<String>,
    /// Explicit performance/bench command only — no stack-based default.
    pub bench_command: Option<String>,
    #[expect(dead_code)]
    pub mutation_command: Option<String>,
    pub stack: String,
}

// ── Defaults ────────────────────────────────────────────────────────

fn default_stack() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_pass_rate() -> f64 {
    1.0
}
fn default_regression_pct() -> f64 {
    10.0
}
fn default_threshold() -> f64 {
    0.05
}
fn default_result_type() -> String {
    "exit_code".to_string()
}

impl Default for Dimensions {
    fn default() -> Self {
        Self {
            correctness: DimensionConfig::default(),
            performance: DimensionConfig::default(),
            quality: DimensionConfig::default(),
            regression: default_regression(),
        }
    }
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            stack: "auto".to_string(),
            dimensions: Dimensions::default(),
            benchmarks: vec![],
        }
    }
}

fn default_regression() -> DimensionConfig<RegressionExtra> {
    DimensionConfig {
        enabled: true,
        command: None,
        extra: RegressionExtra::default(),
    }
}

impl<E: Default> Default for DimensionConfig<E> {
    fn default() -> Self {
        Self {
            enabled: false,
            command: None,
            extra: E::default(),
        }
    }
}

impl Default for CorrectnessExtra {
    fn default() -> Self {
        Self {
            mutation_tool: None,
            min_pass_rate: 1.0,
        }
    }
}

impl Default for PerformanceExtra {
    fn default() -> Self {
        Self {
            max_regression_pct: 10.0,
        }
    }
}

impl Default for QualityExtra {
    fn default() -> Self {
        Self { llm_judge: true }
    }
}

impl Default for RegressionExtra {
    fn default() -> Self {
        Self {
            baseline_dir: None,
            fail_on_regression: true,
            threshold: 0.05,
        }
    }
}

// ── Load / Scaffold ─────────────────────────────────────────────────

pub fn load(eval_dir: &Path) -> Result<EvalConfig, String> {
    let path = eval_dir.join("eval.yaml");
    if !path.exists() {
        return Err(format!("eval config not found: {}", path.display()));
    }
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Scaffold a minimal eval.yaml with auto-detected stack and benchmarks.
pub fn scaffold(eval_dir: &Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(eval_dir).map_err(|e| format!("create dir: {e}"))?;

    let path = eval_dir.join("eval.yaml");
    if path.exists() {
        return Err(format!("eval.yaml already exists: {}", path.display()));
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let stack = detect_stack(&cwd);
    let benchmarks = detect_benchmarks(&cwd);

    if benchmarks.is_empty() {
        eprintln!(
            "warning: no benchmark files detected in {}\n\
             Add commands to `benchmarks:` in eval.yaml to enable domain-specific evaluation.",
            cwd.display()
        );
    }

    let cfg = EvalConfig {
        stack,
        benchmarks,
        ..EvalConfig::default()
    };

    let yaml = serde_yaml::to_string(&cfg).map_err(|e| format!("serialize: {e}"))?;

    let commented = format!(
        "# eval.yaml — auto-generated by `epic eval --init`\n\
         # Correctness/quality dimensions are disabled by default.\n\
         # eval delegates build/test/lint to `verify` and focuses on domain benchmarks.\n\
         # Add benchmark commands to the `benchmarks:` list to measure domain quality.\n\n\
         {yaml}"
    );

    std::fs::write(&path, &commented).map_err(|e| format!("write {}: {e}", path.display()))?;

    // Create in-repo baseline and results dirs
    let bench_dir = cwd.join("benchmarks");
    let _ = std::fs::create_dir_all(bench_dir.join("baselines"));
    let _ = std::fs::create_dir_all(bench_dir.join("results"));

    Ok(path)
}

/// Detect project stack from marker files.
pub fn detect_stack(cwd: &Path) -> String {
    if cwd.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if cwd.join("mix.exs").exists() {
        "elixir".to_string()
    } else if cwd.join("Package.swift").exists() {
        "swift".to_string()
    } else if cwd.join("CMakeLists.txt").exists() {
        "cpp".to_string()
    } else if cwd.join("Gemfile").exists() {
        "ruby".to_string()
    } else if cwd.join("composer.json").exists() {
        "php".to_string()
    } else if cwd
        .read_dir()
        .ok()
        .and_then(|mut e| {
            e.find(|f| {
                f.as_ref()
                    .ok()
                    .map(|f| {
                        let n = f.file_name();
                        let s = n.to_string_lossy();
                        s.ends_with(".csproj") || s.ends_with(".sln")
                    })
                    .unwrap_or(false)
            })
        })
        .is_some()
    {
        "csharp".to_string()
    } else if cwd.join("build.gradle.kts").exists() {
        // Kotlin DSL build takes precedence over plain Gradle
        "kotlin".to_string()
    } else if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        // Check for Kotlin source files before falling back to Java
        if cwd.join("src").exists()
            && std::fs::read_dir(cwd.join("src"))
                .ok()
                .map(|e| {
                    e.flatten()
                        .any(|f| f.path().extension().map(|x| x == "kt").unwrap_or(false))
                })
                .unwrap_or(false)
        {
            "kotlin".to_string()
        } else {
            "java".to_string()
        }
    } else if cwd.join("go.mod").exists() {
        "go".to_string()
    } else if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        "python".to_string()
    } else if cwd.join("tsconfig.json").exists() {
        "typescript".to_string()
    } else if cwd.join("package.json").exists() {
        "node".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Scan for existing benchmark infrastructure and pre-populate benchmark entries.
fn detect_benchmarks(cwd: &Path) -> Vec<Benchmark> {
    let mut found = Vec::new();

    // Python eval runner (e.g. Episteme pattern)
    if cwd.join("benchmarks").join("eval_runner.py").exists() {
        found.push(Benchmark {
            name: "eval_runner".into(),
            command: "python3 benchmarks/eval_runner.py full".into(),
            result_type: "composite".into(),
        });
    }

    // Makefile `eval` target
    if let Ok(content) = std::fs::read_to_string(cwd.join("Makefile"))
        && has_domain_eval_target(&content)
    {
        found.push(Benchmark {
            name: "make_eval".into(),
            command: "make eval".into(),
            result_type: "exit_code".into(),
        });
    }

    // justfile `eval` recipe
    if let Ok(content) = std::fs::read_to_string(cwd.join("justfile"))
        && has_domain_eval_target(&content)
    {
        found.push(Benchmark {
            name: "just_eval".into(),
            command: "just eval".into(),
            result_type: "exit_code".into(),
        });
    }

    found
}

/// Inspect only the eval recipe. Mentions in comments/echoes or other targets
/// must not hide real benchmarks; indirect wrappers are checked at runtime.
fn has_domain_eval_target(content: &str) -> bool {
    let mut found = false;
    let mut in_eval = false;
    let mut recipes = Vec::new();
    let opaque_context = content.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with(".ONESHELL:")
            || line.starts_with("set working-directory")
            || line.starts_with("[script")
            || line.starts_with("[working-directory")
            || line.starts_with("export HARNESS_DIR")
    });
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("#!")) {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            in_eval = line
                .strip_prefix("eval")
                .and_then(|tail| tail.trim_start().strip_prefix(':'))
                .is_some_and(|tail| !tail.starts_with('='));
            if in_eval {
                found = true;
                if let Some((_, recipe)) = line.split_once(';') {
                    recipes.push(recipe);
                }
            }
        } else if in_eval {
            recipes.push(trimmed);
        }
    }
    let effects: Vec<_> = recipes.into_iter().map(recipe_effect).collect();
    found
        && (opaque_context
            || !effects.contains(&RecipeEffect::EvalWrapper)
            || effects.contains(&RecipeEffect::Other))
}

#[derive(PartialEq)]
enum RecipeEffect {
    EvalWrapper,
    Harmless,
    Other,
}

fn recipe_effect(recipe: &str) -> RecipeEffect {
    let recipe = recipe.trim_start_matches([' ', '\t', '@', '-', '+']);
    if recipe.starts_with("#!")
        || ["&&", "||", ";", "|"]
            .iter()
            .any(|part| recipe.contains(part))
    {
        return RecipeEffect::Other;
    }
    let words = super::runner::shell_words(recipe);
    let Some(program) = words.first() else {
        return RecipeEffect::Harmless;
    };
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let name = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".cmd"))
        .unwrap_or(&name);
    if matches!(name, "epic" | "epic-harness")
        && words.get(1).is_some_and(|word| word == "eval")
        && !words
            .iter()
            .skip(2)
            .any(|word| matches!(word.as_str(), "--init" | "--scaffold" | "--help" | "-h"))
    {
        RecipeEffect::EvalWrapper
    } else if matches!(name, "echo" | ":" | "true") {
        RecipeEffect::Harmless
    } else {
        RecipeEffect::Other
    }
}

/// Resolve commands from explicit config only — no stack-based defaults.
///
/// eval delegates build/test/lint to `verify`. Only explicitly configured
/// dimension commands are returned; stack is informational.
pub fn resolve_commands(cfg: &EvalConfig) -> Result<ResolvedCommands, String> {
    let stack = if cfg.stack == "auto" {
        detect_stack(&std::env::current_dir().unwrap_or_default())
    } else {
        cfg.stack.clone()
    };

    let mutation_cmd = cfg
        .dimensions
        .correctness
        .extra
        .mutation_tool
        .as_deref()
        .map(|t| match t {
            "cargo-mutants" => "cargo mutants".to_string(),
            "mutmut" => "mutmut run".to_string(),
            "pitest" => "mvn org.pitest:pitest-maven:mutationCoverage".to_string(),
            other => other.to_string(),
        });

    Ok(ResolvedCommands {
        test_command: cfg.dimensions.correctness.command.clone(),
        lint_command: cfg.dimensions.quality.command.clone(),
        bench_command: cfg.dimensions.performance.command.clone(),
        mutation_command: mutation_cmd,
        stack,
    })
}
