use std::path::Path;
use std::process::Command;

use super::common::*;

fn try_exec(cmd: &str, cwd: &Path) -> Option<String> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
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
        if try_exec(&format!("npx biome format --write \"{file_path}\""), wd).is_some() {
            let name = Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            hint("polish", &format!("Biome: {name}"));
            feedback_to_observe(file_path, "biome", true, None);
        }
    } else if has_prettier
        && try_exec(&format!("npx prettier --write \"{file_path}\""), wd).is_some()
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
    if let Some(out) = try_exec("npx tsc --noEmit --pretty false 2>&1 | head -10", wd) {
        if out.contains("error TS") {
            let snippet = &out[..out.len().min(500)];
            hint("polish", &format!("TS errors:\n{snippet}"));
            feedback_to_observe(file_path, "tsc", false, Some(snippet));
        } else {
            feedback_to_observe(file_path, "tsc", true, None);
        }
    }
}

fn format_python(file_path: &str, wd: &Path) {
    let formatted = try_exec(&format!("ruff format \"{file_path}\" 2>/dev/null"), wd).is_some()
        || try_exec(&format!("black \"{file_path}\" 2>/dev/null"), wd).is_some();
    if formatted {
        let name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        hint("polish", &format!("Formatted: {name}"));
        feedback_to_observe(file_path, "ruff/black", true, None);
    }
}

fn format_go(file_path: &str, wd: &Path) {
    if try_exec(&format!("gofmt -w \"{file_path}\""), wd).is_some() {
        let name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        hint("polish", &format!("gofmt: {name}"));
        feedback_to_observe(file_path, "gofmt", true, None);
    }
}

pub fn run(input: &HookInput) -> i32 {
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
