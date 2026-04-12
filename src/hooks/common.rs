use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// ── Types ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    /// Legacy structured output (kept for forward compat)
    pub tool_output: Option<ToolOutput>,
    /// Claude Code actual PostToolUse payload field (string or object)
    pub tool_result: Option<serde_json::Value>,
    pub conversation_summary: Option<String>,
    pub pending_tasks: Option<Vec<String>>,
    pub context_usage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    pub output: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScoreDimensions {
    pub tool_success: f64,
    pub output_quality: f64,
    pub execution_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsRecord {
    pub timestamp: String,
    pub tool: String,
    pub tool_category: String,
    pub action: Option<String>,
    pub result: Option<String>,
    pub score: Option<f64>,
    pub dimensions: Option<ScoreDimensions>,
    pub failure_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub timestamp: String,
    #[serde(rename = "type")]
    pub snap_type: String,
    pub summary: String,
    pub pending_tasks: Vec<String>,
    pub context_usage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStats {
    pub tool_category: String,
    pub total: u64,
    pub successes: u64,
    pub errors: u64,
    pub avg_score: f64,
    pub failure_categories: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub pattern_type: String,
    pub description: String,
    pub count: u64,
    pub involved_files: Vec<String>,
    pub suggested_remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionAnalysis {
    pub total_observations: u64,
    pub success_rate: f64,
    pub avg_score: f64,
    pub score_distribution: HashMap<String, u64>,
    pub per_tool_stats: HashMap<String, ToolStats>,
    pub per_error_stats: HashMap<String, u64>,
    pub per_ext_stats: HashMap<String, ExtStats>,
    pub failure_patterns: Vec<DetectedPattern>,
    pub dimension_averages: ScoreDimensions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtStats {
    pub total: u64,
    pub errors: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionScoreEntry {
    pub timestamp: String,
    pub success_rate: f64,
    pub avg_score: f64,
    pub observations: u64,
    pub dimension_averages: ScoreDimensions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAttribution {
    pub skill_name: String,
    pub sessions_active: u64,
    pub avg_score_with: f64,
    pub avg_score_without: f64,
    pub first_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub total_sessions: u64,
    pub avg_success_rate: f64,
    pub total_evolved_skills: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_session: Option<String>,
    pub score_history: Vec<SessionScoreEntry>,
    pub best_score: f64,
    pub best_session: String,
    pub trend: String,
    pub stagnation_count: u64,
    #[serde(default)]
    pub skill_attribution: HashMap<String, SkillAttribution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionRecord {
    pub timestamp: String,
    pub observations: u64,
    pub success_rate: f64,
    pub avg_score: f64,
    pub error_patterns: HashMap<String, u64>,
    pub failure_patterns: Vec<DetectedPattern>,
    pub skills_seeded: u64,
    pub skills_rolled_back: u64,
    pub total_evolved: u64,
    pub analysis_summary: String,
}

impl Default for ScoreDimensions {
    fn default() -> Self {
        Self {
            tool_success: 0.0,
            output_quality: 0.0,
            execution_cost: 0.0,
        }
    }
}

// ── Constants ────────────────────────────────────────

pub const SCORE_WEIGHTS: (f64, f64, f64) = (0.5, 0.3, 0.2); // success, quality, cost

pub const STAGNATION_LIMIT: u64 = 3;
pub const IMPROVEMENT_THRESHOLD: f64 = 0.05;
pub const MAX_EVOLVED_SKILLS: usize = 10;

pub const REPEATED_ERROR_MIN: u64 = 3;
pub const FTB_LOOKAHEAD: usize = 3;
pub const FTB_MIN_CYCLES: u64 = 2;
pub const DEBUG_LOOP_MIN: u64 = 5;
pub const THRASH_MIN_EDITS: u64 = 3;
pub const THRASH_MIN_ERRORS: u64 = 3;

pub const WEAK_TOOL_RATE: f64 = 0.6;
pub const WEAK_TOOL_MIN_OBS: u64 = 5;
pub const WEAK_EXT_RATE: f64 = 0.5;
pub const WEAK_EXT_MIN_OBS: u64 = 3;
pub const HIGH_FREQ_ERROR_MIN: u64 = 5;

// ── Paths ────────────────────────────────────────────

pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Returns a stable slug for the current project: `{sanitized-dirname}-{hash6}`.
/// - Name is sanitized to `[a-zA-Z0-9_-]` to be safe as a directory component.
/// - 6-char hex hash (24 bits) prevents collisions between same-named projects.
pub fn project_slug() -> String {
    static SLUG: LazyLock<String> = LazyLock::new(|| {
        let path = cwd();
        // Walk components to find the last meaningful segment (handles "/" edge case).
        let name = path
            .components()
            .filter_map(|c| {
                if let std::path::Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                }
            })
            .next_back()
            .unwrap_or("project")
            .to_string();
        // Sanitize: replace any char that isn't alphanumeric, hyphen, or underscore.
        let safe_name: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let full = path.to_string_lossy();
        let mut h: u32 = 0;
        for b in full.bytes() {
            h = h.wrapping_shl(5).wrapping_sub(h).wrapping_add(b as u32);
        }
        format!("{}-{:06x}", safe_name, h & 0x00ff_ffff)
    });
    SLUG.clone()
}

/// Per-project data lives in `~/.harness/projects/{slug}/` — outside the
/// project tree so it never pollutes git and survives project deletion.
pub fn harness_dir() -> PathBuf {
    static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
        dirs_home()
            .join(".harness")
            .join("projects")
            .join(project_slug())
    });
    DIR.clone()
}

/// Legacy project-local path used for migration detection only.
pub(crate) fn local_harness_dir() -> PathBuf {
    cwd().join(".harness")
}

pub fn obs_dir() -> PathBuf {
    harness_dir().join("obs")
}
pub fn sessions_dir() -> PathBuf {
    harness_dir().join("sessions")
}
pub fn memory_dir() -> PathBuf {
    harness_dir().join("memory")
}
pub fn evolved_dir() -> PathBuf {
    harness_dir().join("evolved")
}
pub fn evolved_backup_dir() -> PathBuf {
    harness_dir().join("evolved_backup")
}
pub fn team_dir() -> PathBuf {
    harness_dir().join("team")
}

pub fn metrics_file() -> PathBuf {
    harness_dir().join("metrics.json")
}
pub fn evolution_file() -> PathBuf {
    harness_dir().join("evolution.jsonl")
}

/// guard-rules.yaml stays in the project tree only if the user/team explicitly
/// created it there. Otherwise, we default to the per-project global directory
/// to keep the project tree clean.
pub fn guard_rules_file() -> PathBuf {
    let local = local_harness_dir().join("guard-rules.yaml");
    if local.is_file() {
        local
    } else {
        harness_dir().join("guard-rules.yaml")
    }
}

pub fn global_harness_dir() -> PathBuf {
    dirs_home().join(".harness").join("global")
}
pub fn global_patterns_file() -> PathBuf {
    global_harness_dir().join("patterns.jsonl")
}

/// Opt-in marker lives in the global dir (not per-project).
pub fn cross_project_file() -> PathBuf {
    global_harness_dir().join(".cross-project-enabled")
}

fn dirs_home() -> PathBuf {
    // Check HOME (Linux/macOS) then USERPROFILE (Windows)
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        return PathBuf::from(up);
    }
    // Windows fallback: HOMEDRIVE + HOMEPATH
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return PathBuf::from(format!("{}{}", drive, path));
    }

    // Fail loudly if home directory cannot be determined.
    // Falling back to /tmp is insecure as it's typically world-readable.
    panic!("[harness] FATAL: Home directory not detected. Please set HOME or USERPROFILE.");
}

// ── Failure Classification ──────────────────────────

struct FailureRule {
    pattern: &'static str,
    category: &'static str,
}

const FAILURE_RULES: &[FailureRule] = &[
    FailureRule {
        pattern: r"(?i)TypeError|type error",
        category: "type_error",
    },
    FailureRule {
        pattern: r"(?i)SyntaxError|Unexpected token|Parse error",
        category: "syntax_error",
    },
    FailureRule {
        pattern: r"(?i)FAIL(?:ED|ING)?[\s:]|test.*fail|AssertionError|assert\.\w+",
        category: "test_fail",
    },
    FailureRule {
        pattern: r"(?i)\blint\b.*(?:error|fail)|eslint.*error|biome.*error|oxlint.*error",
        category: "lint_fail",
    },
    FailureRule {
        pattern: r"(?i)build.*fail|tsc.*error|error TS\d+|compilation.*fail",
        category: "build_fail",
    },
    FailureRule {
        pattern: r"(?i)EACCES|permission denied",
        category: "permission_denied",
    },
    FailureRule {
        pattern: r"(?i)timeout|ETIMEDOUT|timed out",
        category: "timeout",
    },
    FailureRule {
        pattern: r"(?i)ENOENT|No such file or directory",
        category: "not_found",
    },
    FailureRule {
        pattern: r"(?m)(?:^|\n)\s*(?:Error|error|ERROR):|Traceback|at [\w.]+\s*\(|Unhandled|uncaught exception",
        category: "runtime_error",
    },
];

static COMPILED_RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    FAILURE_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.category)))
        .collect()
});

pub fn classify_failure(output: &str) -> Option<&'static str> {
    if output.is_empty() {
        return None;
    }
    let sample = &output[..output.len().min(2000)];
    for (rx, cat) in COMPILED_RULES.iter() {
        if rx.is_match(sample) {
            return Some(cat);
        }
    }
    None
}

pub fn classify_tool(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "bash" => "bash",
        "edit" => "edit",
        "write" => "write",
        "read" => "read",
        "glob" => "glob",
        "grep" => "grep",
        _ => "other",
    }
}

pub fn extract_file_ext(input: &serde_json::Value) -> Option<String> {
    let file_path = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !file_path.is_empty() {
        return Path::new(file_path)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()));
    }

    let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    static EXT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\.(ts|js|py|go|rs|java|c|cpp|rb|sh|json|yaml|yml|md|css|html|tsx|jsx)\b")
            .unwrap()
    });
    EXT_RE.find(cmd).map(|m| m.as_str().to_string())
}

// ── Helpers ──────────────────────────────────────────

pub fn harness_exists() -> bool {
    harness_dir().is_dir()
}

pub fn ensure_dir(path: &Path) {
    let _ = fs::create_dir_all(path);
}

pub fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple date calc (no chrono dep)
    let days = now / 86400;
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &d) in month_days.iter().enumerate() {
        if remaining < d as i64 {
            m = i;
            break;
        }
        remaining -= d as i64;
    }
    format!("{:04}{:02}{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Reuse today() logic for full ISO
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 0usize;
    for (i, &d) in month_days.iter().enumerate() {
        if remaining < d as i64 {
            mo = i;
            break;
        }
        remaining -= d as i64;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo + 1,
        remaining + 1,
        h,
        m,
        s
    )
}

pub fn hint(tag: &str, msg: &str) {
    eprintln!("[{tag}] {msg}");
}

pub fn raw(line: &str) {
    eprintln!("{line}");
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path, fallback: T) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(fallback)
}

pub fn read_jsonl(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn read_jsonl_typed<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn append_jsonl(path: &Path, record: &impl Serialize) {
    use std::io::Write;
    if let Ok(json) = serde_json::to_string(record)
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = writeln!(f, "{json}");
    }
}

pub fn list_dirs(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_files(dir: &Path, ext: &str) -> Vec<String> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.ends_with(ext))
                .collect()
        })
        .unwrap_or_default()
}

pub fn copy_dir(src: &Path, dest: &Path) {
    if !src.is_dir() {
        return;
    }
    ensure_dir(dest);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir(&src_path, &dest_path);
            } else {
                let _ = fs::copy(&src_path, &dest_path);
            }
        }
    }
}

pub struct CopyResult {
    pub ok: u64,
    pub errors: u64,
}

/// Like `copy_dir` but counts successes and errors instead of silently ignoring failures.
pub fn copy_dir_counted(src: &Path, dest: &Path) -> CopyResult {
    let mut result = CopyResult { ok: 0, errors: 0 };
    if !src.is_dir() {
        return result;
    }
    ensure_dir(dest);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                let sub = copy_dir_counted(&src_path, &dest_path);
                result.ok += sub.ok;
                result.errors += sub.errors;
            } else {
                match fs::copy(&src_path, &dest_path) {
                    Ok(_) => result.ok += 1,
                    Err(_) => result.errors += 1,
                }
            }
        }
    }
    result
}

pub fn rm_dir(dir: &Path) {
    if dir.is_dir() {
        let _ = fs::remove_dir_all(dir);
    }
}

pub fn default_metrics() -> Metrics {
    Metrics {
        total_sessions: 0,
        avg_success_rate: 0.0,
        total_evolved_skills: 0,
        last_session: None,
        score_history: vec![],
        best_score: 0.0,
        best_session: String::new(),
        trend: "stable".into(),
        stagnation_count: 0,
        skill_attribution: HashMap::new(),
        last_error_context: None,
    }
}

pub fn session_id() -> String {
    format!("{}_{}", today(), std::process::id())
}

pub fn compute_score(dims: &ScoreDimensions) -> f64 {
    let raw = SCORE_WEIGHTS.0 * dims.tool_success
        + SCORE_WEIGHTS.1 * dims.output_quality
        + SCORE_WEIGHTS.2 * dims.execution_cost;
    (raw * 1000.0).round() / 1000.0
}

pub fn hash_string(s: &str) -> String {
    let mut hash: u32 = 0;
    for b in s.bytes() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(b as u32);
    }
    format!("{:08x}", hash)
}

pub fn normalize_error(snippet: &str) -> String {
    static TS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\dZ]*").unwrap());
    static LC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":\d+:\d+").unwrap());
    static PATH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/[\w./-]+/").unwrap());
    static WS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

    let s = TS_RE.replace_all(snippet, "");
    let s = LC_RE.replace_all(&s, ":L:C");
    let s = PATH_RE.replace_all(&s, "/PATH/");
    let s = WS_RE.replace_all(&s, " ");
    let trimmed = s.trim();
    trimmed[..trimmed.len().min(200)].to_string()
}

/// Parse simple guard-rules.yaml
pub fn parse_guard_rules(content: &str) -> (Vec<GuardRule>, Vec<GuardRule>) {
    let mut blocked = vec![];
    let mut warned = vec![];
    let mut section: Option<&str> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "blocked:" {
            section = Some("blocked");
            continue;
        }
        if trimmed == "warned:" {
            section = Some("warned");
            continue;
        }
        let Some(sec) = section else { continue };
        if !trimmed.starts_with("- ") {
            continue;
        }

        let entry = &trimmed[2..];
        // Format: "pattern: <regex> | msg: <message>"
        if let Some((pat_part, msg_part)) = entry.split_once(" | msg: ") {
            let pat_str = pat_part.trim_start_matches("pattern:").trim();
            if let Ok(rx) = Regex::new(pat_str) {
                let rule = GuardRule {
                    pattern: rx,
                    msg: msg_part.trim().to_string(),
                };
                match sec {
                    "blocked" => blocked.push(rule),
                    "warned" => warned.push(rule),
                    _ => {}
                }
            }
        }
    }
    (blocked, warned)
}

pub struct GuardRule {
    pub pattern: Regex,
    pub msg: String,
}

pub fn extract_file(action: &str) -> Option<&str> {
    static FILE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(/[\w./-]+\.\w+)").unwrap());
    FILE_RE.find(action).map(|m| m.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_failure ────────────────────────────
    #[test]
    fn classify_type_error() {
        assert_eq!(
            classify_failure("TypeError: x is not a function"),
            Some("type_error")
        );
    }

    #[test]
    fn classify_syntax_error() {
        assert_eq!(
            classify_failure("SyntaxError: Unexpected token '}'"),
            Some("syntax_error")
        );
    }

    #[test]
    fn classify_test_fail() {
        assert_eq!(classify_failure("FAILED: test_login"), Some("test_fail"));
    }

    #[test]
    fn classify_lint_fail() {
        assert_eq!(
            classify_failure("eslint error: no-unused-vars"),
            Some("lint_fail")
        );
    }

    #[test]
    fn classify_build_fail() {
        assert_eq!(
            classify_failure("error TS2304: Cannot find name 'x'"),
            Some("build_fail")
        );
    }

    #[test]
    fn classify_permission_denied() {
        assert_eq!(
            classify_failure("EACCES: permission denied"),
            Some("permission_denied")
        );
    }

    #[test]
    fn classify_timeout() {
        assert_eq!(
            classify_failure("ETIMEDOUT: connection timed out"),
            Some("timeout")
        );
    }

    #[test]
    fn classify_not_found() {
        assert_eq!(
            classify_failure("ENOENT: No such file or directory"),
            Some("not_found")
        );
    }

    #[test]
    fn classify_runtime_error() {
        assert_eq!(
            classify_failure("Error: something went wrong"),
            Some("runtime_error")
        );
    }

    #[test]
    fn classify_empty_none() {
        assert_eq!(classify_failure(""), None);
    }

    #[test]
    fn classify_clean_output_none() {
        assert_eq!(classify_failure("npm install completed successfully"), None);
    }

    // ── classify_tool ───────────────────────────────
    #[test]
    fn tool_categories() {
        assert_eq!(classify_tool("Bash"), "bash");
        assert_eq!(classify_tool("Edit"), "edit");
        assert_eq!(classify_tool("Write"), "write");
        assert_eq!(classify_tool("Read"), "read");
        assert_eq!(classify_tool("Glob"), "glob");
        assert_eq!(classify_tool("Grep"), "grep");
        assert_eq!(classify_tool("Agent"), "other");
    }

    // ── extract_file_ext ────────────────────────────
    #[test]
    fn ext_from_file_path() {
        let input = serde_json::json!({"file_path": "/src/main.rs"});
        assert_eq!(extract_file_ext(&input), Some(".rs".into()));
    }

    #[test]
    fn ext_from_command() {
        let input = serde_json::json!({"command": "cat /src/index.ts"});
        assert_eq!(extract_file_ext(&input), Some(".ts".into()));
    }

    #[test]
    fn ext_none_for_no_ext() {
        let input = serde_json::json!({"command": "ls"});
        assert_eq!(extract_file_ext(&input), None);
    }

    // ── compute_score ───────────────────────────────
    #[test]
    fn score_perfect() {
        let dims = ScoreDimensions {
            tool_success: 1.0,
            output_quality: 1.0,
            execution_cost: 1.0,
        };
        assert_eq!(compute_score(&dims), 1.0);
    }

    #[test]
    fn score_zero() {
        let dims = ScoreDimensions {
            tool_success: 0.0,
            output_quality: 0.0,
            execution_cost: 0.0,
        };
        assert_eq!(compute_score(&dims), 0.0);
    }

    #[test]
    fn score_weighted() {
        let dims = ScoreDimensions {
            tool_success: 1.0,
            output_quality: 0.0,
            execution_cost: 0.0,
        };
        assert_eq!(compute_score(&dims), 0.5); // 0.5 * 1.0
    }

    // ── hash_string ─────────────────────────────────
    #[test]
    fn hash_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
    }

    #[test]
    fn hash_different_inputs() {
        assert_ne!(hash_string("hello"), hash_string("world"));
    }

    // ── normalize_error ─────────────────────────────
    #[test]
    fn normalize_strips_timestamps() {
        let input = "2024-01-15T10:30:00Z error happened";
        let output = normalize_error(input);
        assert!(!output.contains("2024-01-15"));
    }

    #[test]
    fn normalize_strips_line_numbers() {
        let input = "error at file.ts:42:10";
        let output = normalize_error(input);
        assert!(output.contains(":L:C"));
    }

    #[test]
    fn normalize_strips_paths() {
        let input = "error in /home/user/project/src/main.ts";
        let output = normalize_error(input);
        assert!(output.contains("/PATH/"));
    }

    #[test]
    fn normalize_truncates_long() {
        let long = "x".repeat(500);
        assert!(normalize_error(&long).len() <= 200);
    }

    // ── parse_guard_rules ───────────────────────────
    #[test]
    fn parse_guard_rules_basic() {
        let yaml = "\
blocked:
  - pattern: kubectl\\s+delete | msg: kubectl delete blocked
warned:
  - pattern: docker\\s+prune | msg: docker prune warning";
        let (blocked, warned) = parse_guard_rules(yaml);
        assert_eq!(blocked.len(), 1);
        assert_eq!(warned.len(), 1);
        assert_eq!(blocked[0].msg, "kubectl delete blocked");
        assert!(blocked[0].pattern.is_match("kubectl delete namespace"));
    }

    #[test]
    fn parse_guard_rules_empty() {
        let (blocked, warned) = parse_guard_rules("");
        assert!(blocked.is_empty());
        assert!(warned.is_empty());
    }

    #[test]
    fn parse_guard_rules_invalid_regex_skipped() {
        let yaml = "blocked:\n  - pattern: (unclosed | msg: bad regex";
        let (blocked, _) = parse_guard_rules(yaml);
        assert!(blocked.is_empty());
    }

    // ── extract_file ────────────────────────────────
    #[test]
    fn extract_file_from_path() {
        assert_eq!(extract_file("/src/main.rs"), Some("/src/main.rs"));
    }

    #[test]
    fn extract_file_from_command() {
        assert_eq!(
            extract_file("cat /project/src/index.ts"),
            Some("/project/src/index.ts")
        );
    }

    #[test]
    fn extract_file_none() {
        assert_eq!(extract_file("ls -la"), None);
    }

    // ── project_slug ────────────────────────────────
    #[test]
    fn project_slug_deterministic() {
        assert_eq!(project_slug(), project_slug());
    }

    #[test]
    fn project_slug_format() {
        let slug = project_slug();
        // "{name}-{6 hex chars}"
        let parts: Vec<&str> = slug.rsplitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 6);
        assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn project_slug_safe_chars_only() {
        // slug before the hash must not contain filesystem-unsafe characters
        let slug = project_slug();
        let name_part = slug.rsplit_once('-').map(|x| x.0).unwrap_or("");
        assert!(
            name_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn project_slug_non_empty() {
        assert!(!project_slug().is_empty());
    }

    // ── today / now_iso ─────────────────────────────
    #[test]
    fn today_format() {
        let t = today();
        assert_eq!(t.len(), 8); // YYYYMMDD
        assert!(t.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn now_iso_format() {
        let iso = now_iso();
        assert!(iso.contains('T'));
        assert!(iso.ends_with('Z'));
        assert!(iso.len() >= 20);
    }

    // ── session_id ──────────────────────────────────
    #[test]
    fn session_id_contains_today() {
        let id = session_id();
        assert!(id.starts_with(&today()));
    }

    #[test]
    fn session_id_contains_pid() {
        let id = session_id();
        let pid = std::process::id().to_string();
        assert!(id.contains(&pid));
    }

    // ── default_metrics ─────────────────────────────
    #[test]
    fn default_metrics_zeroed() {
        let m = default_metrics();
        assert_eq!(m.total_sessions, 0);
        assert_eq!(m.avg_success_rate, 0.0);
        assert_eq!(m.stagnation_count, 0);
        assert!(m.score_history.is_empty());
    }
}
