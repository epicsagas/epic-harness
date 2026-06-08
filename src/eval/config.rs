//! eval/config.rs — Eval configuration loading and stack detection

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalConfig {
    #[serde(default = "default_stack")]
    pub stack: String,
    #[serde(default)]
    pub dimensions: Dimensions,
    #[serde(default)]
    pub benchmarks: Vec<Benchmark>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dimensions {
    #[serde(default = "default_correctness")]
    pub correctness: DimensionConfig<CorrectnessExtra>,
    #[serde(default)]
    pub performance: DimensionConfig<PerformanceExtra>,
    #[serde(default = "default_quality")]
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
    #[serde(default = "default_baseline_dir")]
    pub baseline_dir: Option<String>,
    #[serde(default = "default_true")]
    pub fail_on_regression: bool,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Benchmark {
    pub name: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

// ── Resolved commands (after auto-detection) ────────────────────────

pub struct ResolvedCommands {
    pub test_command: Option<String>,
    pub lint_command: Option<String>,
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
fn default_baseline_dir() -> Option<String> {
    Some("eval/baselines".to_string())
}
fn default_threshold() -> f64 {
    0.05
}

impl Default for Dimensions {
    fn default() -> Self {
        Self {
            correctness: DimensionConfig {
                enabled: true,
                command: None,
                extra: CorrectnessExtra {
                    mutation_tool: None,
                    min_pass_rate: 1.0,
                },
            },
            performance: DimensionConfig {
                enabled: false,
                command: None,
                extra: PerformanceExtra {
                    max_regression_pct: 10.0,
                },
            },
            quality: default_quality(),
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

fn default_quality() -> DimensionConfig<QualityExtra> {
    DimensionConfig {
        enabled: true,
        command: None,
        extra: QualityExtra { llm_judge: true },
    }
}

fn default_correctness() -> DimensionConfig<CorrectnessExtra> {
    DimensionConfig {
        enabled: true,
        command: None,
        extra: CorrectnessExtra {
            mutation_tool: None,
            min_pass_rate: 1.0,
        },
    }
}

fn default_regression() -> DimensionConfig<RegressionExtra> {
    DimensionConfig {
        enabled: true,
        command: None,
        extra: RegressionExtra {
            baseline_dir: Some("eval/baselines".to_string()),
            fail_on_regression: true,
            threshold: 0.05,
        },
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
            baseline_dir: Some("eval/baselines".to_string()),
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

/// Scaffold a minimal eval.yaml with auto-detected stack.
pub fn scaffold(eval_dir: &Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(eval_dir).map_err(|e| format!("create dir: {e}"))?;

    let path = eval_dir.join("eval.yaml");
    if path.exists() {
        return Err(format!("eval.yaml already exists: {}", path.display()));
    }

    let stack = detect_stack();
    let cfg = EvalConfig {
        stack,
        ..EvalConfig::default()
    };

    let yaml = serde_yaml::to_string(&cfg).map_err(|e| format!("serialize: {e}"))?;

    // Add helpful comments
    let commented = format!(
        "# eval.yaml — auto-generated by `epic eval --init`\n\
         # Edit to customize evaluation dimensions.\n\n\
         {yaml}"
    );

    std::fs::write(&path, &commented).map_err(|e| format!("write {}: {e}", path.display()))?;

    // Also create baseline and results dirs
    let _ = std::fs::create_dir_all(eval_dir.join("baselines"));
    let _ = std::fs::create_dir_all(eval_dir.join("results"));

    Ok(path)
}

/// Detect project stack from marker files in CWD.
fn detect_stack() -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if cwd.join("package.json").exists() {
        "node".to_string()
    } else if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        "python".to_string()
    } else if cwd.join("go.mod").exists() {
        "go".to_string()
    } else if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        "java".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Resolve auto-detected commands from stack.
pub fn resolve_commands(cfg: &EvalConfig) -> Result<ResolvedCommands, String> {
    let stack = if cfg.stack == "auto" {
        detect_stack()
    } else {
        cfg.stack.clone()
    };

    let (test_cmd, lint_cmd, bench_cmd) = match stack.as_str() {
        "rust" => (
            Some("cargo test".to_string()),
            Some("cargo clippy -- -D warnings".to_string()),
            Some("cargo bench".to_string()),
        ),
        "node" => (
            Some("npm test".to_string()),
            Some("npm run lint".to_string()),
            Some("npm run bench".to_string()),
        ),
        "python" => (
            Some("pytest".to_string()),
            Some("ruff check .".to_string()),
            Some("pytest --benchmark-only".to_string()),
        ),
        "go" => (
            Some("go test ./...".to_string()),
            Some("golangci-lint run".to_string()),
            Some("go test -bench=.".to_string()),
        ),
        "java" => (
            Some("mvn test".to_string()),
            Some("mvn checkstyle:check".to_string()),
            None,
        ),
        _ => (None, None, None),
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
        test_command: cfg.dimensions.correctness.command.clone().or(test_cmd),
        lint_command: cfg.dimensions.quality.command.clone().or(lint_cmd),
        bench_command: cfg.dimensions.performance.command.clone().or(bench_cmd),
        mutation_command: mutation_cmd,
        stack,
    })
}
