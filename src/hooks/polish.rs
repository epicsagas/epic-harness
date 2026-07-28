use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::common::*;
use crate::telemetry::{FormatterKind, Telemetry};

const MAX_FORMAT_TARGETS: usize = 64;
const MAX_FORMAT_TARGET_BYTES: usize = 4096;
const MAX_FORMATTER_OUTPUT_BYTES: usize = 1024 * 1024;
const FORMATTER_TIMEOUT: Duration = Duration::from_secs(10);
const POLISH_HOOK_TIMEOUT: Duration = Duration::from_secs(20);

fn read_bounded_output(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_FORMATTER_OUTPUT_BYTES as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(unix)]
fn terminate_process_group(process_id: u32) {
    let _ = unsafe { libc::kill(-(process_id as i32), libc::SIGKILL) };
}

#[cfg(not(unix))]
fn terminate_process_group(_process_id: u32) {}

fn terminate_child(child: &mut Child) {
    terminate_process_group(child.id());
    #[cfg(not(unix))]
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn run_command_with_timeout(
    prog: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> io::Result<Output> {
    let mut command = Command::new(prog);
    command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let (stdout_sender, stdout_receiver) = std::sync::mpsc::sync_channel(1);
    let (stderr_sender, stderr_receiver) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = stdout_sender.send(read_bounded_output(stdout));
    });
    thread::spawn(move || {
        let _ = stderr_sender.send(read_bounded_output(stderr));
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_child(&mut child);
                return Err(error);
            }
        }
        if started.elapsed() >= timeout {
            terminate_child(&mut child);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{prog} exceeded {} second timeout", timeout.as_secs()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };

    // The direct child can exit while a descendant still owns its inherited
    // pipes. Close that process group before waiting for capture completion.
    terminate_process_group(child.id());
    let receive = |receiver: std::sync::mpsc::Receiver<io::Result<Vec<u8>>>| {
        let remaining = timeout.saturating_sub(started.elapsed());
        receiver.recv_timeout(remaining).map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "{prog} output capture exceeded {} second timeout",
                    timeout.as_secs()
                ),
            )
        })?
    };
    let stdout = receive(stdout_receiver)?;
    let stderr = receive(stderr_receiver)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Execute a program with discrete arguments — no shell involved.
/// This is the safe replacement for `try_exec` when `file_path` is part of
/// the argument list, because no shell string interpolation occurs.
/// Returns `Some(stdout)` only when the process exits successfully (exit code 0).
#[cfg(test)]
fn try_exec_args(prog: &str, args: &[&str], cwd: &Path) -> Option<String> {
    let o = match run_command_with_timeout(prog, args, cwd, FORMATTER_TIMEOUT) {
        Ok(output) => output,
        Err(error) => {
            if error.kind() == io::ErrorKind::TimedOut {
                eprintln!("[polish] {error}");
            }
            return None;
        }
    };
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        None
    }
}

struct StagedTarget {
    original: PathBuf,
    staged: PathBuf,
    original_bytes: Vec<u8>,
    original_permissions: std::fs::Permissions,
}

impl Drop for StagedTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.staged);
    }
}

fn stage_target(original: &Path) -> io::Result<StagedTarget> {
    let metadata = std::fs::symlink_metadata(original)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("formatter target changed type: {}", original.display()),
        ));
    }
    let original_bytes = std::fs::read(original)?;
    let parent = original
        .parent()
        .ok_or_else(|| io::Error::other("formatter target has no parent"))?;
    let stem = original
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("target");
    let extension = original
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    for attempt in 0..32 {
        let staged = parent.join(format!(
            ".{stem}.{}.{}.polish.{extension}",
            std::process::id(),
            attempt
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        file.write_all(&original_bytes)?;
        file.sync_all()?;
        return Ok(StagedTarget {
            original: original.to_path_buf(),
            staged,
            original_bytes,
            original_permissions: metadata.permissions(),
        });
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate formatter staging file in {}",
            parent.display()
        ),
    ))
}

fn stage_targets(targets: &[PathBuf]) -> io::Result<Vec<StagedTarget>> {
    targets.iter().map(|target| stage_target(target)).collect()
}

#[cfg(not(windows))]
fn atomic_replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(windows)]
fn atomic_replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    crate::team::codex::atomic_replace_file(source, destination)
}

fn commit_staged_target(target: &StagedTarget) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(&target.original)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "formatter target changed during execution: {}",
                target.original.display()
            ),
        ));
    }
    // `validate_targets` hands us paths in `canonical_for_compare` form, so the
    // re-check has to use the same form. Plain `canonicalize` would compare
    // `\\?\C:\...` against `C:\...` on Windows and reject every commit.
    if crate::shared::paths::canonical_for_compare(&target.original)? != target.original {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "formatter target identity changed during execution: {}",
                target.original.display()
            ),
        ));
    }
    if std::fs::read(&target.original)? != target.original_bytes {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "formatter target was modified concurrently: {}",
                target.original.display()
            ),
        ));
    }
    std::fs::set_permissions(&target.staged, target.original_permissions.clone())?;
    atomic_replace_file(&target.staged, &target.original)
}

fn run_batch_command(
    program: &str,
    prefix_args: &[&str],
    targets: &[StagedTarget],
    cwd: &Path,
    deadline: Instant,
) -> io::Result<Output> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "polish hook deadline exceeded",
        ));
    }
    let target_args: Vec<String> = targets
        .iter()
        .map(|target| target.staged.to_string_lossy().into_owned())
        .collect();
    let mut args: Vec<&str> = prefix_args.to_vec();
    args.extend(target_args.iter().map(String::as_str));
    run_command_with_timeout(program, &args, cwd, remaining.min(FORMATTER_TIMEOUT))
}

fn installed_npx_args<'a>(package: &'a str, args: &'a [&'a str]) -> Vec<&'a str> {
    let mut installed_only = Vec::with_capacity(args.len() + 2);
    installed_only.extend(["--no", package]);
    installed_only.extend_from_slice(args);
    installed_only
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
        error_snippet: error_snippet.map(|s| truncate_utf8(s, 500).to_string()),
        file_ext: ext,
        sequence_id: None,
        tool_use_id: None,
    };

    let session_file = obs_dir().join(format!("session_{}.jsonl", session_id()));
    append_jsonl(&session_file, &record);
}

fn format_batch(
    targets: &[PathBuf],
    formatter: &str,
    program: &str,
    prefix_args: &[&str],
    wd: &Path,
    deadline: Instant,
) -> io::Result<bool> {
    if targets.is_empty() {
        return Ok(false);
    }
    let staged = stage_targets(targets)?;
    let output = match run_batch_command(program, prefix_args, &staged, wd, deadline) {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !output.status.success() {
        return Ok(false);
    }
    for target in &staged {
        commit_staged_target(target)?;
        feedback_to_observe(
            target.original.to_string_lossy().as_ref(),
            formatter,
            true,
            None,
        );
    }
    hint(
        "polish",
        &format!("{formatter}: formatted {} file(s)", staged.len()),
    );
    Ok(true)
}

fn check_ts(targets: &[PathBuf], wd: &Path, deadline: Instant) -> io::Result<()> {
    if targets.is_empty() || !wd.join("tsconfig.json").is_file() {
        return Ok(());
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "polish hook deadline exceeded before type-check",
        ));
    }
    let output = run_command_with_timeout(
        "npx",
        &["tsc", "--noEmit", "--pretty", "false"],
        wd,
        remaining.min(FORMATTER_TIMEOUT),
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    if !output.status.success() || combined.contains("error TS") {
        let snippet = truncate_utf8(&combined, 500);
        hint("polish", &format!("TS errors:\n{snippet}"));
        for target in targets {
            feedback_to_observe(
                target.to_string_lossy().as_ref(),
                "tsc",
                false,
                Some(snippet),
            );
        }
        Telemetry::init().track_polish_failed(FormatterKind::Tsc);
    } else {
        for target in targets {
            feedback_to_observe(target.to_string_lossy().as_ref(), "tsc", true, None);
        }
    }
    Ok(())
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
    let mut seen = HashSet::new();
    files
        .into_iter()
        .filter(|file| seen.insert(file.clone()))
        .collect()
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

fn validate_targets(raw_targets: Vec<String>, workspace: &Path) -> Result<Vec<PathBuf>, String> {
    if raw_targets.len() > MAX_FORMAT_TARGETS {
        return Err(format!(
            "{} formatter targets exceeds limit of {MAX_FORMAT_TARGETS}",
            raw_targets.len()
        ));
    }

    // `canonical_for_compare`, not `canonicalize`: on Windows the latter
    // returns `\\?\C:\...` while the host supplies `C:\...` in `file_path` (or
    // in an `apply_patch` envelope), and `strip_prefix` matches whole
    // components — so every absolute target looked like it was outside the
    // workspace and `polish` never formatted anything there.
    let workspace = canonical_for_compare(workspace)
        .map_err(|error| format!("cannot resolve workspace {}: {error}", workspace.display()))?;
    let mut seen = HashSet::new();
    let mut targets = Vec::new();

    for raw in raw_targets {
        if raw.len() > MAX_FORMAT_TARGET_BYTES {
            return Err(format!(
                "formatter target exceeds {MAX_FORMAT_TARGET_BYTES} byte limit"
            ));
        }
        if raw.starts_with('-') {
            return Err(format!("option-like formatter target rejected: {raw}"));
        }

        let path = Path::new(&raw);
        if path.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('-'))
        }) {
            return Err(format!("option-like formatter target rejected: {raw}"));
        }
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(format!("parent traversal formatter target rejected: {raw}"));
        }

        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace.join(path)
        };
        let relative = candidate
            .strip_prefix(&workspace)
            .map_err(|_| format!("formatter target is outside workspace: {raw}"))?;

        let mut cursor = workspace.clone();
        for component in relative.components() {
            cursor.push(component);
            let metadata = std::fs::symlink_metadata(&cursor)
                .map_err(|error| format!("invalid formatter target {raw}: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!("symlink formatter target rejected: {raw}"));
            }
        }

        let canonical = canonical_for_compare(&candidate)
            .map_err(|error| format!("cannot resolve formatter target {raw}: {error}"))?;
        if !canonical.starts_with(&workspace) {
            return Err(format!("formatter target escapes workspace: {raw}"));
        }
        if !canonical.is_file() {
            return Err(format!("formatter target is not a regular file: {raw}"));
        }
        if canonical.to_str().is_none() {
            return Err(format!("formatter target is not valid UTF-8: {raw}"));
        }
        if seen.insert(canonical.clone()) {
            targets.push(canonical);
        }
    }

    Ok(targets)
}

fn polish_targets(targets: &[PathBuf], wd: &Path) -> io::Result<()> {
    let deadline = Instant::now() + POLISH_HOOK_TIMEOUT;
    let by_extension = |extensions: &[&str]| {
        targets
            .iter()
            .filter(|target| {
                target
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extensions.contains(&extension))
            })
            .cloned()
            .collect::<Vec<_>>()
    };

    let javascript = by_extension(&["js", "jsx", "ts", "tsx"]);
    if wd.join("biome.json").is_file() || wd.join("biome.jsonc").is_file() {
        let _ = format_batch(
            &javascript,
            "biome",
            "npx",
            &["biome", "format", "--write"],
            wd,
            deadline,
        )?;
    } else if wd.join(".prettierrc").is_file() || wd.join(".prettierrc.json").is_file() {
        let args = installed_npx_args("prettier", &["--write"]);
        let _ = format_batch(&javascript, "prettier", "npx", &args, wd, deadline)?;
    }

    let python = by_extension(&["py"]);
    if !python.is_empty() && !format_batch(&python, "ruff", "ruff", &["format"], wd, deadline)? {
        let _ = format_batch(&python, "black", "black", &[], wd, deadline)?;
    }

    let go = by_extension(&["go"]);
    let _ = format_batch(&go, "gofmt", "gofmt", &["-w"], wd, deadline)?;

    let typescript = by_extension(&["ts", "tsx"]);
    check_ts(&typescript, wd, deadline)
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_POLISH) {
        return 0;
    }

    let raw_targets = target_files(input);
    if raw_targets.is_empty() {
        return 0;
    }

    let wd = cwd();
    let targets = match validate_targets(raw_targets, &wd) {
        Ok(targets) => targets,
        Err(error) => {
            eprintln!("[polish] {error}");
            return 1;
        }
    };
    if let Err(error) = polish_targets(&targets, &wd) {
        eprintln!("[polish] {error}");
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant};
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
    fn patched_files_removes_non_adjacent_duplicates() {
        let patch = "\
*** Begin Patch
*** Update File: src/a.py
*** Update File: src/b.py
*** Update File: src/a.py
*** End Patch";
        assert_eq!(patched_files(patch), vec!["src/a.py", "src/b.py"]);
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

    #[test]
    fn formatter_targets_are_canonical_regular_workspace_files() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.py"), "print('ok')").unwrap();

        let targets = validate_targets(vec!["src/main.py".into()], dir.path()).unwrap();
        assert_eq!(
            targets,
            vec![dir.path().join("src/main.py").canonicalize().unwrap()]
        );
    }

    #[test]
    fn formatter_targets_reject_escapes_options_and_non_files() {
        let workspace = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("outside.py");
        fs::write(&outside_file, "pass").unwrap();
        fs::create_dir(workspace.path().join("src")).unwrap();
        fs::write(workspace.path().join("src/-danger.py"), "pass").unwrap();

        for target in [
            outside_file.to_string_lossy().into_owned(),
            "../outside.py".into(),
            "-danger.py".into(),
            "src/-danger.py".into(),
            "src".into(),
        ] {
            assert!(
                validate_targets(vec![target.clone()], workspace.path()).is_err(),
                "{target:?} must be rejected"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn formatter_targets_reject_symlinks() {
        use std::os::unix::fs::symlink;

        let workspace = tempdir().unwrap();
        fs::write(workspace.path().join("real.py"), "pass").unwrap();
        symlink("real.py", workspace.path().join("linked.py")).unwrap();
        assert!(validate_targets(vec!["linked.py".into()], workspace.path()).is_err());
    }

    #[test]
    fn formatter_targets_enforce_count_and_length_caps() {
        let dir = tempdir().unwrap();
        let too_many = (0..=MAX_FORMAT_TARGETS)
            .map(|i| format!("file-{i}.py"))
            .collect();
        assert!(validate_targets(too_many, dir.path()).is_err());
        assert!(
            validate_targets(vec!["a".repeat(MAX_FORMAT_TARGET_BYTES + 1)], dir.path()).is_err()
        );
    }

    #[test]
    fn formatter_targets_deduplicate_by_canonical_path() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.py"), "pass").unwrap();
        let targets =
            validate_targets(vec!["file.py".into(), "./file.py".into()], dir.path()).unwrap();
        assert_eq!(targets.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn staged_formatter_never_follows_a_swapped_target_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.py");
        let external = dir.path().join("external.py");
        fs::write(&target, "original").unwrap();
        fs::write(&external, "external").unwrap();
        let staged = stage_target(&target.canonicalize().unwrap()).unwrap();
        fs::write(&staged.staged, "formatted").unwrap();
        fs::remove_file(&target).unwrap();
        symlink(&external, &target).unwrap();

        assert!(commit_staged_target(&staged).is_err());
        assert_eq!(fs::read_to_string(external).unwrap(), "external");
    }

    #[cfg(unix)]
    #[test]
    fn staged_formatter_preserves_target_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.py");
        fs::write(&target, "original").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o750)).unwrap();
        let staged = stage_target(&target.canonicalize().unwrap()).unwrap();
        fs::write(&staged.staged, "formatted").unwrap();

        commit_staged_target(&staged).unwrap();

        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o750);
    }

    #[cfg(unix)]
    #[test]
    fn formatter_batches_same_kind_targets_into_one_process() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let first = dir.path().join("first.py");
        let second = dir.path().join("second.py");
        let invocations = dir.path().join("invocations");
        let formatter = dir.path().join("formatter.sh");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        fs::write(
            &formatter,
            format!(
                "#!/bin/sh\nprintf 'one\\n' >> '{}'\nfor file in \"$@\"; do printf '\\nformatted' >> \"$file\"; done\n",
                invocations.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&formatter, fs::Permissions::from_mode(0o700)).unwrap();

        let targets = vec![
            first.canonicalize().unwrap(),
            second.canonicalize().unwrap(),
        ];
        assert!(
            format_batch(
                &targets,
                "test",
                formatter.to_str().unwrap(),
                &[],
                dir.path(),
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap()
        );

        assert_eq!(fs::read_to_string(invocations).unwrap().lines().count(), 1);
        assert!(fs::read_to_string(first).unwrap().contains("formatted"));
        assert!(fs::read_to_string(second).unwrap().contains("formatted"));
    }

    #[cfg(unix)]
    #[test]
    fn formatter_process_is_killed_at_timeout() {
        let dir = tempdir().unwrap();
        let start = Instant::now();
        let error =
            run_command_with_timeout("sleep", &["5"], dir.path(), Duration::from_millis(100))
                .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn formatter_output_reader_stops_at_the_capture_cap() {
        struct FailsAfterCap {
            remaining: usize,
        }
        impl Read for FailsAfterCap {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.remaining == 0 {
                    return Err(io::Error::other("reader was drained past cap"));
                }
                let read = self.remaining.min(buffer.len());
                buffer[..read].fill(b'x');
                self.remaining -= read;
                Ok(read)
            }
        }

        let output = read_bounded_output(FailsAfterCap {
            remaining: MAX_FORMATTER_OUTPUT_BYTES,
        })
        .unwrap();
        assert_eq!(output.len(), MAX_FORMATTER_OUTPUT_BYTES);
    }

    #[test]
    fn prettier_npx_arguments_refuse_implicit_installation() {
        assert_eq!(
            installed_npx_args("prettier", &["--write"]),
            vec!["--no", "prettier", "--write"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn formatter_timeout_covers_descendants_holding_output_pipes() {
        let dir = tempdir().unwrap();
        let started = Instant::now();
        run_command_with_timeout(
            "sh",
            &["-c", "(sleep 5) &"],
            dir.path(),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
