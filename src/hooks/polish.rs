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

    let session_file = obs_dir().join(format!("session_{}.jsonl", session_id()));
    append_jsonl(&session_file, &record);
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

fn check_ts(file_path: &str, wd: &Path) {
    if !wd.join("tsconfig.json").is_file() {
        return;
    }
    let output = Command::new("npx")
        .args(["tsc", "--noEmit", "--pretty", "false"])
        .current_dir(wd)
        .output();
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

/// Extract the files touched by an `apply_patch` envelope.
///
/// Codex's edit tool is `apply_patch`: it carries the whole patch in
/// `tool_input.command` and supplies no `file_path`, so a `file_path`-only
/// polish hook never fires on Codex at all. The envelope marks each target with
/// a `*** <verb> File:` header:
///
/// ```text
/// *** Begin Patch
/// *** Update File: src/main.rs
/// *** Add File: src/new.py
/// *** Delete File: src/old.go
/// *** End Patch
/// ```
///
/// `Delete File` is skipped — there is nothing left to format. A `*** Move to:`
/// header renames the previous target, so the destination replaces it.
pub fn patched_files(command: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for line in command.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("***") else {
            continue;
        };
        let rest = rest.trim();
        if let Some(p) = rest
            .strip_prefix("Update File:")
            .or_else(|| rest.strip_prefix("Add File:"))
        {
            let p = p.trim();
            if !p.is_empty() {
                files.push(p.to_string());
            }
        } else if let Some(p) = rest.strip_prefix("Move to:") {
            // Rename: format the destination, not the vanished source.
            let p = p.trim();
            if !p.is_empty() {
                files.pop();
                files.push(p.to_string());
            }
        }
    }
    files.dedup();
    files
}

/// Resolve which files a hook invocation touches.
///
/// Claude Code supplies `file_path` for Edit/Write; Codex supplies an
/// `apply_patch` envelope in `command`. Shared with `guard`, which needs the
/// same answer for concurrent-write conflict detection.
pub(crate) fn target_files(input: &HookInput) -> Vec<String> {
    let Some(ti) = input.tool_input.as_ref() else {
        return Vec::new();
    };

    if let Some(fp) = ti.get("file_path").and_then(|v| v.as_str())
        && !fp.is_empty()
    {
        return vec![fp.to_string()];
    }

    // Codex `apply_patch` — patch body lives in `command`.
    if let Some(cmd) = ti.get("command").and_then(|v| v.as_str()) {
        return patched_files(cmd);
    }

    Vec::new()
}

fn polish_file(file_path: &str, wd: &Path) {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "js" | "jsx" | "ts" | "tsx" => {
            format_js(file_path, wd);
            if ext == "ts" || ext == "tsx" {
                check_ts(file_path, wd);
            }
        }
        "py" => format_python(file_path, wd),
        "go" => format_go(file_path, wd),
        _ => {}
    }
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_POLISH) {
        return 0;
    }

    let targets = target_files(input);
    if targets.is_empty() {
        return 0;
    }

    let wd = cwd();
    for file_path in &targets {
        polish_file(file_path, &wd);
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

    // ── Codex apply_patch targeting ──────────────────

    #[test]
    fn patched_files_reads_update_and_add() {
        let patch = "\
*** Begin Patch
*** Update File: src/main.rs
@@ fn main
-old
+new
*** Add File: src/new.py
+print('hi')
*** End Patch";
        assert_eq!(patched_files(patch), vec!["src/main.rs", "src/new.py"]);
    }

    #[test]
    fn patched_files_skips_deletes() {
        let patch = "\
*** Begin Patch
*** Delete File: src/gone.go
*** End Patch";
        assert!(patched_files(patch).is_empty(), "nothing to format");
    }

    #[test]
    fn patched_files_follows_move_destination() {
        let patch = "\
*** Begin Patch
*** Update File: src/old.ts
*** Move to: src/new.ts
*** End Patch";
        assert_eq!(patched_files(patch), vec!["src/new.ts"]);
    }

    #[test]
    fn patched_files_ignores_non_patch_text() {
        assert!(patched_files("cargo test --all").is_empty());
    }

    /// The Codex path: `apply_patch` carries no `file_path`, so the old
    /// `file_path`-only lookup made polish a permanent no-op on Codex.
    #[test]
    fn target_files_handles_apply_patch_input() {
        let input = HookInput {
            tool_name: Some("apply_patch".into()),
            tool_input: Some(serde_json::json!({
                "command": "*** Begin Patch\n*** Update File: a.py\n*** End Patch"
            })),
            ..Default::default()
        };
        assert_eq!(target_files(&input), vec!["a.py"]);
    }

    #[test]
    fn target_files_still_handles_claude_file_path() {
        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(serde_json::json!({"file_path": "/src/main.rs"})),
            ..Default::default()
        };
        assert_eq!(target_files(&input), vec!["/src/main.rs"]);
    }

    #[test]
    fn target_files_empty_without_tool_input() {
        assert!(target_files(&HookInput::default()).is_empty());
    }
}
