use std::path::Path;
use std::process::Command;

use super::common::*;
use crate::telemetry::{FormatterKind, Telemetry};

/// Execute a program with discrete arguments — no shell involved.
/// This is the safe replacement for `try_exec` when `file_path` is part of
/// the argument list, because no shell string interpolation occurs.
/// Returns `Some(stdout)` only when the process exits successfully (exit code 0).
fn try_exec_args(prog: &str, args: &[&str], cwd: &Path) -> Option<String> {
    let o = Command::new(prog)
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        None
    }
}

fn feedback_to_observe(
    file_path: &str,
    formatter: &str,
    success: bool,
    error_snippet: Option<&str>,
) {
    if !harness_exists() {
        return;
    }
    ensure_dir(&obs_dir());

    let dims = ScoreDimensions {
        tool_success: if success { 1.0 } else { 0.0 },
        output_quality: if success { 1.0 } else { 0.3 },
        execution_cost: 1.0,
    };

    let ext = Path::new(file_path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()));

    let basename = Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let record = ObsRecord {
        timestamp: now_iso(),
        tool: "polish".into(),
        tool_category: "other".into(),
        action: Some(format!("{formatter}:{basename}")),
        result: Some(if success { "success" } else { "error" }.into()),
        score: Some(compute_score(&dims)),
        dimensions: Some(dims),
        pipeline_id: detect_active_orbit_id(),
        failure_category: if success {
            None
        } else {
            Some(
                if formatter == "tsc" {
                    "build_fail"
                } else {
                    "lint_fail"
                }
                .into(),
            )
        },
        error_snippet: error_snippet.map(|s| s[..s.len().min(500)].to_string()),
        file_ext: ext,
        sequence_id: None,
    };

    // Storage policy: SQLite primary, JSONL fallback — same as observe. reflect
    // reads SQLite, so JSONL-only writes here never reached pattern detection
    // (the "Polish → Observe Feedback" path was dead until this).
    let sid = session_id();
    let stored = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        crate::store::observations::insert_observation_pool(
            &pool,
            &record,
            &sid,
            &crate::shared::paths::project_slug(),
        )
        .await
    });
    if let Err(e) = stored {
        eprintln!("[polish] SQLite write failed, falling back to JSONL: {e}");
        let session_file = obs_dir().join(format!("session_{sid}.jsonl"));
        append_jsonl(&session_file, &record);
    }
}

fn format_js(file_path: &str, wd: &Path) {
    let has_biome = wd.join("biome.json").is_file() || wd.join("biome.jsonc").is_file();
    let has_prettier = wd.join(".prettierrc").is_file() || wd.join(".prettierrc.json").is_file();

    if has_biome {
        if try_exec_args("npx", &["biome", "format", "--write", file_path], wd).is_some() {
            let name = Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            hint("polish", &format!("Biome: {name}"));
            feedback_to_observe(file_path, "biome", true, None);
        }
    } else if has_prettier
        && try_exec_args("npx", &["prettier", "--write", file_path], wd).is_some()
    {
        let name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        hint("polish", &format!("Prettier: {name}"));
        feedback_to_observe(file_path, "prettier", true, None);
    }
}

/// Minimum seconds between full-project tsc runs. Models fire rapid successive
/// edits; a blocking full typecheck per edit adds seconds of latency without
/// new information. The tsbuildinfo file doubles as incremental cache and
/// debounce timestamp.
const TSC_DEBOUNCE_SECS: u64 = 5;

/// Per-workspace tsbuildinfo path (under the user's home when available —
/// shared /tmp would leak project paths and invite symlink planting —
/// keyed by workspace path hash, never written into the user's project).
fn tsc_tsbuildinfo(wd: &Path) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    let base = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(".harness").join("tsc");
    std::fs::create_dir_all(&dir).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    wd.hash(&mut hasher);
    Some(dir.join(format!("{:016x}.tsbuildinfo", hasher.finish())))
}

fn check_ts(file_path: &str, wd: &Path) {
    if !wd.join("tsconfig.json").is_file() {
        return;
    }
    let Some(tsbuildinfo) = tsc_tsbuildinfo(wd) else {
        return;
    };
    let Some(info_str) = tsbuildinfo.to_str() else {
        return;
    };
    if let Ok(modified) = std::fs::metadata(&tsbuildinfo).and_then(|m| m.modified())
        && let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified)
        && elapsed.as_secs() < TSC_DEBOUNCE_SECS
    {
        return; // a typecheck ran moments ago against this workspace
    }

    // Prefer the workspace-local tsc — resolving it via npx costs 1-3s per run.
    let local_name = if cfg!(windows) { "tsc.cmd" } else { "tsc" };
    let local_tsc = wd.join("node_modules").join(".bin").join(local_name);
    let (prog, base_args): (String, Vec<&str>) = if local_tsc.exists() {
        (local_tsc.to_string_lossy().into_owned(), vec![])
    } else {
        ("npx".into(), vec!["tsc"])
    };

    let mut args = base_args;
    args.extend([
        "--noEmit",
        "--pretty",
        "false",
        "--incremental",
        "--tsBuildInfoFile",
        info_str,
    ]);
    let output = Command::new(&prog).args(&args).current_dir(wd).output();
    if let Ok(o) = output {
        let stdout = String::from_utf8_lossy(&o.stdout);
        let stderr = String::from_utf8_lossy(&o.stderr);
        let combined = format!("{stdout}{stderr}");
        if !o.status.success() || combined.contains("error TS") {
            let snippet = &combined[..combined.len().min(500)];
            hint("polish", &format!("TS errors:\n{snippet}"));
            feedback_to_observe(file_path, "tsc", false, Some(snippet));
            Telemetry::init().track_polish_failed(FormatterKind::Tsc);
        } else {
            feedback_to_observe(file_path, "tsc", true, None);
        }
    }
}

fn format_python(file_path: &str, wd: &Path) {
    let formatter = if try_exec_args("ruff", &["format", file_path], wd).is_some() {
        Some("ruff")
    } else if try_exec_args("black", &[file_path], wd).is_some() {
        Some("black")
    } else {
        None
    };
    if let Some(name) = formatter {
        let fname = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        hint("polish", &format!("Formatted: {fname}"));
        feedback_to_observe(file_path, name, true, None);
    }
}

fn format_go(file_path: &str, wd: &Path) {
    if try_exec_args("gofmt", &["-w", file_path], wd).is_some() {
        let name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        hint("polish", &format!("gofmt: {name}"));
        feedback_to_observe(file_path, "gofmt", true, None);
    }
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_POLISH) {
        return 0;
    }

    let file_path = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("file_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if file_path.is_empty() {
        return 0;
    }

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let wd = cwd();

    match ext {
        "js" | "jsx" | "ts" | "tsx" => {
            format_js(file_path, &wd);
            if ext == "ts" || ext == "tsx" {
                check_ts(file_path, &wd);
            }
        }
        "py" => format_python(file_path, &wd),
        "go" => format_go(file_path, &wd),
        _ => {}
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Verify that `try_exec_args` does NOT interpret shell metacharacters in
    /// the path argument.  A path containing `; touch INJECTED` would create
    /// a sentinel file if passed through `sh -c`; it must not happen here.
    #[test]
    fn try_exec_args_no_shell_injection() {
        let dir = tempdir().unwrap();
        let sentinel = dir.path().join("INJECTED");

        // Malicious file_path that escapes a double-quoted shell argument and
        // runs `touch <sentinel>`.
        let malicious = format!("foo\"; touch {} ; echo \"", sentinel.to_string_lossy());

        // "echo" always succeeds; we only care that the shell never ran.
        let _ = try_exec_args("echo", &[&malicious], dir.path());

        assert!(
            !sentinel.exists(),
            "Shell injection detected: sentinel file was created — \
             file_path was interpreted by a shell"
        );
    }

    /// Confirm that the argument is delivered verbatim to the child process,
    /// including spaces and single-quotes that would confuse a shell parser.
    #[test]
    fn try_exec_args_passes_path_as_literal_arg() {
        let dir = tempdir().unwrap();

        let path_with_spaces = "file with spaces and 'quotes'.js";

        // `printf '%s\n'` echoes each argument back unchanged.
        let out = try_exec_args("printf", &["%s\n", path_with_spaces], dir.path());

        assert_eq!(
            out.as_deref().map(str::trim),
            Some(path_with_spaces),
            "Argument was not passed verbatim to the child process"
        );
    }

    #[test]
    fn try_exec_args_returns_none_on_nonzero_exit() {
        // `false` command always exits with code 1
        let dir = tempdir().unwrap();
        let result = try_exec_args("false", &[], dir.path());
        assert!(result.is_none(), "non-zero exit must return None");
    }

    #[test]
    fn try_exec_args_returns_some_on_success() {
        let dir = tempdir().unwrap();
        let result = try_exec_args("true", &[], dir.path());
        assert!(result.is_some(), "zero exit must return Some");
    }

    #[test]
    fn tsc_tsbuildinfo_deterministic_per_workspace() {
        let dir = tempdir().unwrap();
        let a = tsc_tsbuildinfo(&dir.path().join("ws-a")).unwrap();
        let b = tsc_tsbuildinfo(&dir.path().join("ws-b")).unwrap();
        assert_eq!(a, tsc_tsbuildinfo(&dir.path().join("ws-a")).unwrap());
        assert_ne!(a, b);
        assert!(a.to_string_lossy().ends_with(".tsbuildinfo"));
    }
}
