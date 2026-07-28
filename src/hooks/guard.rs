use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::common::{self, CONFLICT_LOOKBACK, HookInput, PROFILE_GUARD, hint, should_run};
use crate::telemetry::{RuleKind, Telemetry};

struct BuiltinRule {
    pattern: &'static str,
    msg: &'static str,
}

const BLOCKED_RULES: &[BuiltinRule] = &[
    BuiltinRule {
        pattern: r"git\s+push\s+.*--force\s+(origin\s+)?(main|master)\b",
        msg: "Force push to main/master blocked",
    },
    BuiltinRule {
        pattern: r"rm\s+-rf\s+/([^a-zA-Z0-9_]|$)",
        msg: "rm -rf / blocked",
    },
    BuiltinRule {
        pattern: r"(?i)DROP\s+(DATABASE|TABLE)\s+.*prod",
        msg: "DROP on production DB blocked",
    },
];

/// Conventional Commits pattern: `type(scope): desc` or `type: desc`
/// Optional `!` before `:` for breaking changes.
/// Types: feat, fix, build, chore, ci, docs, style, refactor, perf, test
static CC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(feat|fix|build|chore|ci|docs|style|refactor|perf|test)(\([a-zA-Z0-9_/.,:-]+\))?!?:\s.+",
    )
    .unwrap()
});

/// Extract the commit message from a `git commit -m "..."` command.
/// Handles single quotes, double quotes, and HEREDOC `$(cat <<'EOF' ... EOF)` patterns.
fn extract_commit_message(cmd: &str) -> Option<String> {
    // Normalize CRLF → LF so HEREDOC parsing works on Windows or git CRLF output.
    let normalized;
    let cmd = if cmd.contains('\r') {
        normalized = cmd.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        cmd
    };

    // HEREDOC: find delimiter after <<, then find that delimiter on its own line
    static HEREDOC_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*-m\s+"\$\(cat\s+<<'?(\w+)'?"#).unwrap());
    if let Some(caps) = HEREDOC_START.captures(cmd) {
        let delim = caps[1].to_string();
        let match_end = caps.get(0).unwrap().end();
        // Body starts on the line after the HEREDOC declaration line.
        let after_match = &cmd[match_end..];
        if let Some(nl) = after_match.find('\n') {
            let body_start = match_end + nl + 1;
            if let Some(end_pos) = cmd[body_start..].find(&format!("\n{delim}")) {
                return Some(cmd[body_start..body_start + end_pos].trim().to_string());
            }
        }
        return None;
    }

    // Simple: git commit -m "msg" or git commit -m 'msg'
    // Split into two patterns to avoid mismatched quote matching
    // and to allow the other quote type inside the message.
    // Use non-greedy .*? so we capture the FIRST -m argument (the subject),
    // not the last one (which would be the body in `git commit -m "subj" -m "body"`).
    static SIMPLE_DQ: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*?-m\s+"([^"]+)""#).unwrap());
    static SIMPLE_SQ: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"git\s+commit\s+.*?-m\s+'([^']+)'"#).unwrap());
    if let Some(caps) = SIMPLE_DQ.captures(cmd) {
        return Some(caps[1].trim().to_string());
    }
    if let Some(caps) = SIMPLE_SQ.captures(cmd) {
        return Some(caps[1].trim().to_string());
    }

    None
}

/// Validate commit message against Conventional Commits.
/// Returns an error message if invalid, None if valid or not a commit command.
fn check_conventional_commit(cmd: &str) -> Option<String> {
    let msg = extract_commit_message(cmd)?;
    let first_line = msg.lines().next().unwrap_or("").trim();
    if first_line.is_empty() || CC_RE.is_match(first_line) {
        None
    } else {
        Some(format!(
            "Commit message does not follow Conventional Commits: \"{first_line}\"\n\
             Expected: type(scope): description  (types: feat|fix|build|chore|ci|docs|style|refactor|perf|test)\n\
             Rewrite the message as: type(scope): description"
        ))
    }
}

const WARNED_RULES: &[BuiltinRule] = &[
    BuiltinRule {
        pattern: r"git\s+push\s+.*--force",
        msg: "Force push — ensure this is intentional",
    },
    BuiltinRule {
        pattern: r"git\s+reset\s+--hard",
        msg: "Hard reset will discard local changes",
    },
    BuiltinRule {
        pattern: r"rm\s+-rf\s+",
        msg: "Recursive delete — double-check the path",
    },
];

static COMPILED_BLOCKED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    BLOCKED_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

static COMPILED_WARNED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    WARNED_RULES
        .iter()
        .filter_map(|r| Regex::new(r.pattern).ok().map(|rx| (rx, r.msg)))
        .collect()
});

fn check_blocked(cmd: &str) -> Option<&'static str> {
    for (rx, msg) in COMPILED_BLOCKED.iter() {
        if rx.is_match(cmd) {
            return Some(msg);
        }
    }
    None
}

fn check_warned(cmd: &str) -> Vec<&'static str> {
    COMPILED_WARNED
        .iter()
        .filter(|(rx, _)| rx.is_match(cmd))
        .map(|(_, msg)| *msg)
        .collect()
}

// ── Orchestration: Concurrent Write Conflict Detection ──────

/// Returns true when `EPIC_ORCHESTRATION=enabled` or when `HARNESS_DIR/orchestrator` exists.
fn is_orchestration_enabled() -> bool {
    if std::env::var("EPIC_ORCHESTRATION").as_deref() == Ok("enabled") {
        return true;
    }
    // Also enabled when the orchestrator directory exists
    // (used for testing without env var races)
    if orchestrator_dir().is_some() {
        return true;
    }
    false
}

/// True when this tool writes to files, on either host.
///
/// Codex edits arrive as `apply_patch`, so an `Edit`/`Write`-only check left
/// every Codex edit outside the orchestration pause and conflict checks.
fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name.to_lowercase().as_str(),
        "edit" | "write" | "apply_patch" | "multiedit" | "notebookedit"
    )
}

/// Resolve the orchestrator directory from an optional explicit base, then the
/// normal per-project harness base used by hooks.
fn resolve_orchestrator_dir(env_base: Option<PathBuf>, project_base: &Path) -> Option<PathBuf> {
    env_base
        .map(|base| base.join("orchestrator"))
        .filter(|path| path.is_dir())
        .or_else(|| {
            let path = project_base.join("orchestrator");
            path.is_dir().then_some(path)
        })
}

/// Resolve the active project's orchestrator state. `HARNESS_DIR` remains an
/// explicit override, but native hooks do not need it.
fn orchestrator_dir() -> Option<PathBuf> {
    resolve_orchestrator_dir(
        std::env::var("HARNESS_DIR").ok().map(PathBuf::from),
        &common::harness_dir(),
    )
}

/// Return agent IDs whose `status.json` reports `"running"`.
fn running_agent_ids(orch_dir: &Path) -> Vec<String> {
    let agents_dir = orch_dir.join("agents");
    if !agents_dir.is_dir() {
        return vec![];
    }
    let mut ids = vec![];
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let agent_id = match entry.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let status_path = entry.path().join("status.json");
            if let Ok(content) = std::fs::read_to_string(&status_path)
                && let Ok(doc) = serde_json::from_str::<serde_json::Value>(&content)
                && doc.get("status").and_then(|v| v.as_str()) == Some("running")
            {
                ids.push(agent_id);
            }
        }
    }
    ids
}

/// Read the last N lines of a JSONL file, parsed as generic JSON values.
fn read_jsonl_tail(path: &Path, n: usize) -> Vec<serde_json::Value> {
    use std::io::{Read, Seek, SeekFrom};

    const MAX_TAIL_BYTES: u64 = 64 * 1024;

    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return vec![],
    };
    let len = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return vec![],
    };
    let start = len.saturating_sub(MAX_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return vec![];
    }

    let mut bytes = Vec::with_capacity((len - start) as usize);
    if file.read_to_end(&mut bytes).is_err() {
        return vec![];
    }
    let bytes = if start > 0 {
        match bytes.iter().position(|byte| *byte == b'\n') {
            Some(index) => &bytes[index + 1..],
            None => return vec![],
        }
    } else {
        &bytes
    };
    let content = String::from_utf8_lossy(bytes);
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .rev()
        .take(n)
        .collect()
}

/// Check if `file_path` appears in the recent entries of `stream.jsonl`.
fn file_in_recent_entries(entries: &[serde_json::Value], file_path: &str) -> bool {
    entries.iter().any(|entry| {
        // Check tool_input.file_path
        if let Some(fp) = entry
            .get("tool_input")
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            && fp == file_path
        {
            return true;
        }
        // Also check action field (observe-style records)
        if let Some(action) = entry.get("action").and_then(|v| v.as_str())
            && action.contains(file_path)
        {
            return true;
        }
        false
    })
}

/// Detect concurrent write conflicts: returns agent IDs that recently modified
/// the same file path, excluding `exclude_agent_id`.
/// Accepts an explicit orchestrator directory for testability.
#[cfg(test)]
fn detect_concurrent_write_conflict(
    file_path: &str,
    exclude_agent_id: Option<&str>,
    orch_dir: &Path,
) -> Vec<String> {
    build_conflict_index(&[file_path.to_string()], exclude_agent_id, orch_dir)
        .remove(file_path)
        .unwrap_or_default()
}

/// Build one conflict index for all deduplicated targets in this invocation.
///
/// Each running agent stream is tail-read once. The caller can then emit all
/// target warnings without rescanning the same files.
fn build_conflict_index(
    file_paths: &[String],
    exclude_agent_id: Option<&str>,
    orch_dir: &Path,
) -> BTreeMap<String, Vec<String>> {
    let running = running_agent_ids(orch_dir);
    let targets: BTreeSet<&str> = file_paths
        .iter()
        .map(String::as_str)
        .filter(|path| !path.is_empty())
        .collect();
    let mut conflicts = BTreeMap::<String, Vec<String>>::new();

    for agent_id in &running {
        if let Some(exclude) = exclude_agent_id
            && agent_id == exclude
        {
            continue;
        }
        let stream_path = orch_dir.join("agents").join(agent_id).join("stream.jsonl");
        if !stream_path.is_file() {
            continue;
        }
        let recent = read_jsonl_tail(&stream_path, CONFLICT_LOOKBACK);
        for target in &targets {
            if file_in_recent_entries(&recent, target) {
                conflicts
                    .entry((*target).to_string())
                    .or_default()
                    .push(agent_id.clone());
            }
        }
    }
    conflicts
}

/// Current agent ID from `EPIC_AGENT_ID` env var.
fn current_agent_id() -> Option<String> {
    // The host names its own subagents; `EPIC_AGENT_ID` only covers agents the
    // harness spawned itself.
    crate::shared::host::agent_id().or_else(|| std::env::var("EPIC_AGENT_ID").ok())
}

/// Check if `control.json` has a "pause" directive targeting the current agent.
/// Missing state means no pause. Existing unreadable, malformed, or invalid
/// state returns an error so the caller can block with the real reason.
/// Accepts an explicit orchestrator directory for testability.
fn check_control_json_pause(current_agent: Option<&str>, orch_dir: &Path) -> Result<bool, String> {
    let control_path = orch_dir.join("control.json");
    let content = match std::fs::read_to_string(&control_path) {
        Ok(c) => c,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!("cannot read {}: {error}", control_path.display()));
        }
    };
    let doc: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("malformed {}: {error}", control_path.display()))?;
    if !doc.is_object() {
        return Err(format!(
            "{} must contain a JSON object",
            control_path.display()
        ));
    }

    // Format 1: ControlDirective style {"action": "pause", "target": "agent-x" or "all"}
    if let Some(action_value) = doc.get("action") {
        let Some(action) = action_value.as_str() else {
            return Err(format!(
                "{} action must be a string",
                control_path.display()
            ));
        };
        if action == "pause" {
            let target = match doc.get("target") {
                Some(value) => match value.as_str() {
                    Some(target) if !target.is_empty() => target,
                    _ => {
                        return Err(format!(
                            "{} pause target must be a nonempty string",
                            control_path.display()
                        ));
                    }
                },
                None => "all",
            };
            if target == "all" {
                return Ok(true);
            }
            if let Some(agent) = current_agent {
                return Ok(target == agent);
            }
        }
        return Ok(false);
    }

    // Format 2: Legacy {"pause": ["agent-1", "agent-2"]} or {"pause": "agent-1"} or {"pause": true}
    let pause_val = match doc.get("pause") {
        Some(v) => v,
        None => return Ok(false),
    };
    if pause_val.is_boolean() {
        return Ok(pause_val.as_bool().unwrap_or(false));
    }
    if pause_val.is_string() {
        if let Some(agent) = current_agent {
            return Ok(pause_val.as_str() == Some(agent));
        }
        return Ok(false);
    }
    if let Some(arr) = pause_val.as_array() {
        if arr.iter().any(|value| !value.is_string()) {
            return Err(format!(
                "{} pause array must contain only agent strings",
                control_path.display()
            ));
        }
        if let Some(agent) = current_agent {
            return Ok(arr.iter().any(|v| v.as_str() == Some(agent)));
        }
        return Ok(false);
    }
    Err(format!(
        "{} pause must be a boolean, string, or string array",
        control_path.display()
    ))
}

pub fn run(input: &HookInput) -> i32 {
    if !should_run(PROFILE_GUARD) {
        return 0;
    }

    // ── Orchestration checks (file-writing tools only) ────
    if is_orchestration_enabled() {
        let tool_name = input.tool_name.as_deref().unwrap_or("");

        if is_write_tool(tool_name) {
            let orch_dir = orchestrator_dir();

            // control.json pause check — blocks the tool call entirely
            let agent_id = current_agent_id();
            if let Some(ref orch) = orch_dir {
                match check_control_json_pause(agent_id.as_deref(), orch) {
                    Ok(true) => {
                        hint(
                            "guard",
                            &format!(
                                "BLOCKED: control.json pause directive active for agent {}",
                                agent_id.as_deref().unwrap_or("unknown")
                            ),
                        );
                        return 2;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        hint("guard", &format!("BLOCKED: invalid control state: {error}"));
                        return 2;
                    }
                }
            }

            // Concurrent write conflict detection — informational warning only.
            // An `apply_patch` envelope can touch several files, so every target
            // is checked, not just a single `file_path`.
            if let Some(ref orch) = orch_dir {
                let targets = super::polish::target_files(input);
                let conflicts = build_conflict_index(&targets, agent_id.as_deref(), orch);
                for (file_path, other_agents) in conflicts {
                    for other_id in other_agents {
                        hint(
                            "guard",
                            &format!(
                                "Concurrent write conflict: agent {} recently modified {}",
                                other_id, file_path
                            ),
                        );
                    }
                }
            }
        }
    }

    let cmd = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if cmd.is_empty() {
        return 0;
    }

    // Lazy: construct Telemetry (file I/O) only when a block or warn actually fires.
    let telemetry = std::cell::OnceCell::new();
    let get_telemetry = || telemetry.get_or_init(Telemetry::init);

    // Check built-in blocked rules first — safety-critical, must run before CC check
    // so a dangerous command appended after a CC-invalid message cannot bypass the block.
    if let Some(msg) = check_blocked(cmd) {
        hint("guard", &format!("BLOCKED: {msg}"));
        get_telemetry().track_hook_blocked(RuleKind::Builtin);
        return 2;
    }

    // Check conventional commit format
    if let Some(msg) = check_conventional_commit(cmd) {
        hint("guard", &format!("BLOCKED: {msg}"));
        get_telemetry().track_hook_blocked(RuleKind::ConventionalCommit);
        return 2;
    }

    // Check custom blocked rules
    let rules_file = common::guard_rules_file();
    if common::harness_exists()
        && rules_file.is_file()
        && let Ok(content) = std::fs::read_to_string(&rules_file)
    {
        let (custom_blocked, custom_warned) = common::parse_guard_rules(&content);
        for rule in &custom_blocked {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("BLOCKED: {}", rule.msg));
                get_telemetry().track_hook_blocked(RuleKind::Custom);
                return 2;
            }
        }
        // Evaluate builtin + custom warned together (same order as TS implementation)
        for msg in check_warned(cmd) {
            hint("guard", &format!("WARNING: {msg}"));
            get_telemetry().track_hook_warned(RuleKind::Builtin);
        }
        for rule in &custom_warned {
            if rule.pattern.is_match(cmd) {
                hint("guard", &format!("WARNING: {}", rule.msg));
                get_telemetry().track_hook_warned(RuleKind::Custom);
            }
        }
        return 0;
    }

    // No custom rules file — just check builtin warned rules
    for msg in check_warned(cmd) {
        hint("guard", &format!("WARNING: {msg}"));
        get_telemetry().track_hook_warned(RuleKind::Builtin);
    }

    0
}

// ── Guard rule file editor (for HarnessEdit::AddGuardRule) ──────

/// Max pattern length accepted by [`append_guard_rule`] / [`HarnessEdit::validate`].
/// Guards against pathological regex that would slow every guard run.
pub const GUARD_PATTERN_MAX_LEN: usize = 256;

/// Severity level for a custom guard rule.
///
/// `"block"` (or `"blocked"`) routes the rule into the `blocked:` section;
/// anything else routes into `warned:`.
fn normalize_level(level: &str) -> &'static str {
    match level.trim().to_lowercase().as_str() {
        "block" | "blocked" => "blocked",
        _ => "warned",
    }
}

/// In-memory representation of a `guard-rules.yaml` file, kept as plain strings
/// so it round-trips losslessly through the line-based [`parse_guard_rules`]
/// parser used by the guard hook (which expects `pattern: <re> | msg: <text>`).
///
/// We deliberately do NOT use `serde_yaml` for the file body here: the
/// incumbent parser reads entries shaped `pattern: x | msg: y`, which
/// serde_yaml would misinterpret (the `|` would fold into the pattern value).
/// serde_yaml is therefore restricted to validating inputs the caller passes
/// in; the on-disk format stays parser-compatible.
#[derive(Debug, Clone, Default)]
struct GuardRulesFile {
    blocked: Vec<(String, String)>,
    warned: Vec<(String, String)>,
}

impl GuardRulesFile {
    /// Parse a `guard-rules.yaml` body into typed entries, mirroring the
    /// incumbent [`parse_guard_rules`] line grammar so the same file can be
    /// read, mutated, and rewritten without losing entries.
    ///
    /// Unknown / malformed lines are silently skipped (parser-tolerant, same
    /// policy as the regex-compiling incumbent parser).
    fn parse(content: &str) -> Self {
        let mut out = GuardRulesFile::default();
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
            let Some(sec) = section else {
                continue;
            };
            if !trimmed.starts_with("- ") {
                continue;
            }
            let entry = &trimmed[2..];
            if let Some((pat_part, msg_part)) = entry.split_once(" | msg: ") {
                let pat = pat_part.trim_start_matches("pattern:").trim().to_string();
                let msg = msg_part.trim().to_string();
                if !pat.is_empty() {
                    match sec {
                        "blocked" => out.blocked.push((pat, msg)),
                        "warned" => out.warned.push((pat, msg)),
                        _ => {}
                    }
                }
            }
        }
        out
    }

    /// Serialize back to the exact `guard-rules.yaml` grammar the incumbent
    /// parser consumes. Empty sections are omitted so a fresh single-rule file
    /// stays minimal.
    fn render(&self) -> String {
        let mut out = String::new();
        let render_section = |out: &mut String, header: &str, entries: &[(String, String)]| {
            if entries.is_empty() {
                return;
            }
            out.push_str(header);
            out.push('\n');
            for (pat, msg) in entries {
                out.push_str(&format!("  - pattern: {pat} | msg: {msg}\n"));
            }
        };
        render_section(&mut out, "blocked:", &self.blocked);
        render_section(&mut out, "warned:", &self.warned);
        out
    }
}

/// Append a guard rule to a `guard-rules.yaml` file at `path`.
///
/// Read-modify-write: existing entries are preserved verbatim (parsed and
/// re-rendered through the round-trip-safe grammar), the new rule is appended
/// to the requested section, and the result is written atomically via a
/// same-directory temp file + rename so a crash mid-write cannot corrupt or
/// truncate the existing rules.
///
/// Returns `Ok(())` on success. Errors (unreadable file, non-UTF-8 content,
/// unwritable directory, rename failure) propagate so callers can map them to
/// [`EditOutcome::Skipped`].
pub fn append_guard_rule(
    path: &Path,
    level: &str,
    pattern: &str,
    msg: &str,
) -> std::io::Result<()> {
    // Read existing content (empty if missing) — never clobber.
    let existing = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };

    let mut rules = GuardRulesFile::parse(&existing);
    let entry = (pattern.to_string(), msg.to_string());
    match normalize_level(level) {
        "blocked" => rules.blocked.push(entry),
        _ => rules.warned.push(entry),
    }
    let rendered = rules.render();

    // Atomic write: write to a sibling temp file, fsync, then rename over the
    // target. Same directory guarantees the rename is atomic on POSIX.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".guard-rules.yaml.tmp.{}", std::process::id(),));
    {
        let mut file = std::fs::File::create(&tmp)?;
        use std::io::Write;
        file.write_all(rendered.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // ── Blocked commands ────────────────────────────
    #[test]
    fn blocks_force_push_main() {
        assert!(check_blocked("git push --force origin main").is_some());
    }

    #[test]
    fn blocks_force_push_master() {
        assert!(check_blocked("git push --force origin master").is_some());
    }

    #[test]
    fn blocks_rm_rf_root() {
        assert!(check_blocked("rm -rf /").is_some());
    }

    #[test]
    fn blocks_rm_rf_root_with_space() {
        assert!(check_blocked("rm -rf / --no-preserve-root").is_some());
    }

    #[test]
    fn blocks_drop_prod_database() {
        assert!(check_blocked("DROP DATABASE prod_db").is_some());
    }

    #[test]
    fn blocks_drop_prod_table() {
        assert!(check_blocked("DROP TABLE production_users").is_some());
    }

    // ── Allowed commands ────────────────────────────
    #[test]
    fn allows_normal_push() {
        assert!(check_blocked("git push origin main").is_none());
    }

    #[test]
    fn allows_force_push_feature() {
        assert!(check_blocked("git push --force origin feature/x").is_none());
    }

    #[test]
    fn allows_rm_rf_dir() {
        assert!(check_blocked("rm -rf /tmp/build").is_none());
    }

    #[test]
    fn blocks_rm_rf_double_slash() {
        assert!(check_blocked("rm -rf //").is_some());
    }

    #[test]
    fn allows_rm_rf_tmp() {
        assert!(check_blocked("rm -rf /tmp").is_none());
    }

    #[test]
    fn allows_rm_rf_var() {
        assert!(check_blocked("rm -rf /var/log").is_none());
    }

    #[test]
    fn allows_drop_dev_db() {
        assert!(check_blocked("DROP DATABASE dev_db").is_none());
    }

    #[test]
    fn allows_git_status() {
        assert!(check_blocked("git status").is_none());
    }

    #[test]
    fn allows_empty() {
        assert!(check_blocked("").is_none());
    }

    // ── Warned commands ─────────────────────────────
    #[test]
    fn warns_force_push_feature() {
        let w = check_warned("git push --force origin feature/x");
        assert!(!w.is_empty());
        assert!(w[0].contains("Force push"));
    }

    #[test]
    fn warns_hard_reset() {
        let w = check_warned("git reset --hard HEAD~3");
        assert!(!w.is_empty());
        assert!(w[0].contains("Hard reset"));
    }

    #[test]
    fn warns_rm_rf_dir() {
        let w = check_warned("rm -rf /tmp/build");
        assert!(!w.is_empty());
        assert!(w[0].contains("Recursive delete"));
    }

    #[test]
    fn no_warning_for_safe_commands() {
        assert!(check_warned("git status").is_empty());
        assert!(check_warned("ls -la").is_empty());
        assert!(check_warned("npm test").is_empty());
    }

    // ── Conventional Commits ─────────────────────────
    #[test]
    fn cc_valid_feat() {
        assert!(
            check_conventional_commit(r#"git commit -m "feat(auth): add login endpoint""#)
                .is_none()
        );
    }

    #[test]
    fn cc_valid_fix_no_scope() {
        assert!(
            check_conventional_commit(r#"git commit -m "fix: resolve null pointer""#).is_none()
        );
    }

    #[test]
    fn cc_valid_breaking() {
        assert!(
            check_conventional_commit(r#"git commit -m "refactor!: drop legacy API""#).is_none()
        );
    }

    #[test]
    fn cc_valid_heredoc() {
        let cmd = "git commit -m \"$(cat <<'EOF'\nfeat(mem): add search\nEOF\n)\"";
        assert!(check_conventional_commit(cmd).is_none());
    }

    #[test]
    fn cc_valid_heredoc_crlf() {
        // Windows CRLF line endings must not break HEREDOC parsing
        let cmd = "git commit -m \"$(cat <<'EOF'\r\nfeat(guard): fix crlf\r\nEOF\r\n)\"";
        assert!(check_conventional_commit(cmd).is_none());
    }

    #[test]
    fn cc_invalid_heredoc_crlf() {
        let cmd = "git commit -m \"$(cat <<'EOF'\r\nadded stuff\r\nEOF\r\n)\"";
        assert!(check_conventional_commit(cmd).is_some());
    }

    #[test]
    fn cc_invalid_no_type() {
        assert!(check_conventional_commit(r#"git commit -m "added login""#).is_some());
    }

    #[test]
    fn cc_invalid_uppercase() {
        assert!(check_conventional_commit(r#"git commit -m "Feat: add login""#).is_some());
    }

    #[test]
    fn cc_not_a_commit() {
        assert!(check_conventional_commit("git status").is_none());
    }

    #[test]
    fn cc_run_blocks_bad_message() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git commit -m \"added stuff\""})),
            ..Default::default()
        };
        assert_eq!(run(&input), 2);
    }

    #[test]
    fn cc_run_allows_good_message() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git commit -m \"feat: add stuff\""})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    #[test]
    fn cc_valid_single_quotes() {
        assert!(check_conventional_commit("git commit -m 'feat: add login'").is_none());
    }

    #[test]
    fn cc_message_with_apostrophe() {
        assert!(check_conventional_commit(r#"git commit -m "feat: it's done""#).is_none());
    }

    #[test]
    fn cc_valid_multi_scope() {
        assert!(
            check_conventional_commit(r#"git commit -m "fix(cli,index): prevent injection""#)
                .is_none()
        );
    }

    #[test]
    fn cc_valid_multi_m_body() {
        // Second -m is the body; subject should be validated, not the body line
        let cmd = r#"git commit -m "fix(mem): resolve injection" -m "- use rusqlite params""#;
        assert!(
            check_conventional_commit(cmd).is_none(),
            "subject is valid CC, body must be ignored"
        );
    }

    #[test]
    fn cc_subject_extracted_not_body() {
        let msg = extract_commit_message(
            r#"git commit -m "fix(mem): subject line" -m "- body line one""#,
        );
        assert_eq!(msg.as_deref(), Some("fix(mem): subject line"));
    }

    #[test]
    fn cc_mismatched_quotes_no_match() {
        assert!(extract_commit_message(r#"git commit -m "bad message'"#).is_none());
    }

    // ── run() integration ───────────────────────────
    #[test]
    fn run_empty_input_returns_0() {
        let input = HookInput::default();
        assert_eq!(run(&input), 0);
    }

    #[test]
    fn run_blocked_returns_2() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git push --force origin main"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 2);
    }

    #[test]
    fn run_safe_returns_0() {
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "git status"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    /// Verify that a safe command (no block/warn) goes through the entire
    /// run() without touching telemetry. We confirm this indirectly: the
    /// OnceCell must still be unset after run() returns 0 for a safe command
    /// with no warnings. The cell is local to run(), so we validate the
    /// observable behaviour: run() returns 0 and does not panic even when
    /// consent/install-id files are absent (because Telemetry::init() is
    /// never called for purely safe commands with no custom rules file).
    #[test]
    fn run_safe_no_warn_no_telemetry_init() {
        // "npm install" matches none of the builtin blocked/warned rules and
        // is not a git commit, so the OnceCell must never be initialised.
        let input = HookInput {
            tool_input: Some(serde_json::json!({"command": "npm install"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);
    }

    // ── Orchestration: concurrent write conflict detection ──

    fn targets(tool_input: serde_json::Value) -> Vec<String> {
        super::super::polish::target_files(&HookInput {
            tool_input: Some(tool_input),
            ..Default::default()
        })
    }

    #[test]
    fn extract_file_path_from_edit_input() {
        assert_eq!(
            targets(
                serde_json::json!({"file_path": "/src/main.rs", "old_string": "fn main()", "new_string": "fn main() {}"})
            ),
            vec!["/src/main.rs".to_string()]
        );
    }

    #[test]
    fn extract_file_path_from_write_input() {
        assert_eq!(
            targets(serde_json::json!({"file_path": "/src/lib.ts", "content": "export {}"})),
            vec!["/src/lib.ts".to_string()]
        );
    }

    #[test]
    fn extract_file_path_missing_returns_none() {
        assert!(targets(serde_json::json!({"command": "git status"})).is_empty());
    }

    #[test]
    fn extract_file_path_empty_returns_none() {
        assert!(targets(serde_json::json!({"file_path": "", "content": ""})).is_empty());
    }

    #[test]
    fn apply_patch_targets_every_file_in_the_envelope() {
        // Codex edits carry no file_path; conflict detection must still see
        // every file the patch touches.
        let patch =
            "*** Begin Patch\n*** Update File: src/a.rs\n*** Add File: src/b.rs\n*** End Patch";
        assert_eq!(
            targets(serde_json::json!({ "command": patch })),
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
        );
    }

    #[test]
    fn apply_patch_is_a_write_tool() {
        for tool in ["Edit", "write", "apply_patch", "MultiEdit"] {
            assert!(is_write_tool(tool), "{tool} must take the write path");
        }
        for tool in ["Bash", "Read", "Grep", ""] {
            assert!(!is_write_tool(tool), "{tool} must not take the write path");
        }
    }

    #[test]
    fn read_jsonl_tail_reads_last_n() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.jsonl");
        std::fs::write(
            &path,
            "{\"seq\":1}\n{\"seq\":2}\n{\"seq\":3}\n{\"seq\":4}\n{\"seq\":5}\n",
        )
        .unwrap();
        let entries = read_jsonl_tail(&path, 3);
        assert_eq!(entries.len(), 3);
        // rev order: 5, 4, 3
        assert_eq!(entries[0]["seq"], 5);
        assert_eq!(entries[1]["seq"], 4);
        assert_eq!(entries[2]["seq"], 3);
    }

    #[test]
    fn read_jsonl_tail_missing_file_empty() {
        let path = std::path::Path::new("/tmp/nonexistent_epic_test_file.jsonl");
        let entries = read_jsonl_tail(path, 3);
        assert!(entries.is_empty());
    }

    #[test]
    fn read_jsonl_tail_does_not_scan_past_the_bounded_tail_window() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.jsonl");
        let content = format!("{{\"seq\":1}}\n{}\n", "x".repeat(128 * 1024));
        std::fs::write(&path, content).unwrap();

        assert!(
            read_jsonl_tail(&path, 3).is_empty(),
            "records outside the bounded tail window must not be scanned"
        );
    }

    #[test]
    fn file_in_recent_entries_matches_tool_input_file_path() {
        let entries = vec![serde_json::json!({
            "tool_input": {"file_path": "/src/main.rs"}
        })];
        assert!(file_in_recent_entries(&entries, "/src/main.rs"));
        assert!(!file_in_recent_entries(&entries, "/src/other.rs"));
    }

    #[test]
    fn file_in_recent_entries_matches_action_field() {
        let entries = vec![serde_json::json!({
            "action": "Edit /src/lib.rs: replaced fn"
        })];
        assert!(file_in_recent_entries(&entries, "/src/lib.rs"));
        assert!(!file_in_recent_entries(&entries, "/src/main.rs"));
    }

    #[test]
    fn file_in_recent_entries_empty_no_match() {
        let entries: Vec<serde_json::Value> = vec![];
        assert!(!file_in_recent_entries(&entries, "/src/main.rs"));
    }

    #[test]
    fn detect_conflict_finds_other_agent() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        // agent-1: running, with stream containing file path
        let agent1_dir = agents_dir.join("agent-1");
        std::fs::create_dir_all(&agent1_dir).unwrap();
        std::fs::write(agent1_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent1_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/main.rs\"}}\n",
        )
        .unwrap();

        // agent-2: running, different file
        let agent2_dir = agents_dir.join("agent-2");
        std::fs::create_dir_all(&agent2_dir).unwrap();
        std::fs::write(agent2_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent2_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/other.rs\"}}\n",
        )
        .unwrap();

        // agent-1 modified /src/main.rs, we are agent-2
        let conflicts =
            detect_concurrent_write_conflict("/src/main.rs", Some("agent-2"), &orch_dir);
        assert!(conflicts.contains(&"agent-1".to_string()));
        assert!(!conflicts.contains(&"agent-2".to_string()));

        // agent-2 modified /src/other.rs, no conflict for /src/main.rs
        let no_conflicts =
            detect_concurrent_write_conflict("/src/main.rs", Some("agent-1"), &orch_dir);
        assert!(no_conflicts.is_empty());
    }

    #[test]
    fn detect_conflict_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        let conflicts = detect_concurrent_write_conflict("/src/main.rs", None, &orch_dir);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn detect_conflict_ignores_stopped_agents() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        let agent_dir = agents_dir.join("agent-stopped");
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("status.json"), "{\"status\":\"stopped\"}").unwrap();
        std::fs::write(
            agent_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/main.rs\"}}\n",
        )
        .unwrap();

        let conflicts = detect_concurrent_write_conflict("/src/main.rs", None, &orch_dir);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflict_index_deduplicates_targets_and_agents() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agent_dir = orch_dir.join("agents").join("agent-1");
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent_dir.join("stream.jsonl"),
            concat!(
                "{\"tool_input\":{\"file_path\":\"/src/a.rs\"}}\n",
                "{\"tool_input\":{\"file_path\":\"/src/b.rs\"}}\n"
            ),
        )
        .unwrap();

        let targets = vec![
            "/src/a.rs".to_string(),
            "/src/a.rs".to_string(),
            "/src/b.rs".to_string(),
        ];
        let index = build_conflict_index(&targets, None, &orch_dir);

        assert_eq!(index.len(), 2);
        assert_eq!(index["/src/a.rs"], vec!["agent-1".to_string()]);
        assert_eq!(index["/src/b.rs"], vec!["agent-1".to_string()]);
    }

    #[test]
    fn normal_project_harness_dir_enables_orchestration_without_env_override() {
        let dir = tempfile::tempdir().expect("tempdir");
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).expect("orchestrator dir");

        assert_eq!(
            resolve_orchestrator_dir(None, dir.path()),
            Some(orch_dir),
            "normal hooks must discover project orchestration state without HARNESS_DIR"
        );
    }

    // ── control.json pause enforcement ──────────────────────

    #[test]
    fn control_json_pause_boolean_true() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":true}").unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
    }

    #[test]
    fn control_json_pause_boolean_false() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":false}").unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
    }

    #[test]
    fn control_json_pause_string_match() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-1\"}").unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
        assert!(!check_control_json_pause(Some("agent-2"), &orch_dir).unwrap());
    }

    #[test]
    fn control_json_pause_array_match() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(
            orch_dir.join("control.json"),
            "{\"pause\":[\"agent-1\",\"agent-3\"]}",
        )
        .unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
        assert!(!check_control_json_pause(Some("agent-2"), &orch_dir).unwrap());
        assert!(check_control_json_pause(Some("agent-3"), &orch_dir).unwrap());
    }

    #[test]
    fn control_json_no_pause_field() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"resume\":true}").unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
    }

    #[test]
    fn control_json_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        // Dir exists but no control.json file
        std::fs::create_dir_all(&orch_dir).unwrap();

        assert!(!check_control_json_pause(Some("agent-1"), &orch_dir).unwrap());
    }

    #[test]
    fn malformed_control_json_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":").unwrap();

        assert!(check_control_json_pause(Some("agent-1"), &orch_dir).is_err());
    }

    // ── run() integration with orchestration ────────────────

    #[test]
    #[serial]
    fn run_orchestration_pause_blocks_edit() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-x\"}").unwrap();

        // SAFETY: env mutation scoped to this test; HARNESS_DIR only read by
        // orchestrator_dir() which resolves the temp path we created above.
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-x");
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(
                serde_json::json!({"file_path": "/src/main.rs", "old_string": "x", "new_string": "y"}),
            ),
            ..Default::default()
        };
        assert_eq!(run(&input), 2, "pause directive must block Edit tool call");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    #[serial]
    fn run_orchestration_pause_not_targeting_passes() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();
        std::fs::write(orch_dir.join("control.json"), "{\"pause\":\"agent-other\"}").unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-x");
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(
                serde_json::json!({"file_path": "/src/main.rs", "old_string": "x", "new_string": "y"}),
            ),
            ..Default::default()
        };
        assert_eq!(run(&input), 0, "non-targeted agent must not be blocked");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
    }

    #[test]
    #[serial]
    fn run_orchestration_not_enabled_skips_checks() {
        // No HARNESS_DIR set — orchestration checks are skipped entirely
        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        let input = HookInput {
            tool_name: Some("Edit".into()),
            tool_input: Some(serde_json::json!({"file_path": "/src/main.rs"})),
            ..Default::default()
        };
        // Should return 0 (no command field -> early return, no orchestration block)
        assert_eq!(run(&input), 0);
    }

    #[test]
    #[serial]
    fn run_orchestration_concurrent_warning_does_not_block() {
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");

        // agent-1 running, modified /src/shared.rs
        let agent1_dir = agents_dir.join("agent-1");
        std::fs::create_dir_all(&agent1_dir).unwrap();
        std::fs::write(agent1_dir.join("status.json"), "{\"status\":\"running\"}").unwrap();
        std::fs::write(
            agent1_dir.join("stream.jsonl"),
            "{\"tool_input\":{\"file_path\":\"/src/shared.rs\"}}\n",
        )
        .unwrap();

        // control.json with no pause
        std::fs::write(orch_dir.join("control.json"), "{}").unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
            std::env::set_var("EPIC_AGENT_ID", "agent-2");
        }

        let input = HookInput {
            tool_name: Some("Write".into()),
            tool_input: Some(serde_json::json!({"file_path": "/src/shared.rs", "content": "new"})),
            ..Default::default()
        };
        // Warning is informational — must NOT block (return 0)
        assert_eq!(run(&input), 0, "concurrent write warning must not block");

        unsafe {
            std::env::remove_var("HARNESS_DIR");
            std::env::remove_var("EPIC_AGENT_ID");
        }
    }

    #[test]
    #[serial]
    fn run_orchestration_bash_tool_skipped() {
        // Bash tool should not trigger orchestration checks
        let dir = tempfile::tempdir().unwrap();
        let orch_dir = dir.path().join("orchestrator");
        std::fs::create_dir_all(&orch_dir).unwrap();

        // SAFETY: env mutation scoped to this test
        unsafe {
            std::env::set_var("HARNESS_DIR", dir.path());
        }

        let input = HookInput {
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "git status"})),
            ..Default::default()
        };
        assert_eq!(run(&input), 0);

        unsafe {
            std::env::remove_var("HARNESS_DIR");
        }
    }

    // ── append_guard_rule file editor ──────────────────────

    #[test]
    fn append_guard_rule_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-rules.yaml");
        append_guard_rule(
            &path,
            "block",
            r"kubectl\s+delete",
            "kubectl delete blocked",
        )
        .unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("blocked:"));
        assert!(content.contains(r"pattern: kubectl\s+delete"));
        assert!(content.contains("msg: kubectl delete blocked"));
    }

    #[test]
    fn append_guard_rule_warn_level_routes_to_warned_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-rules.yaml");
        append_guard_rule(&path, "warn", r"docker\s+prune", "check first").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("warned:"));
        assert!(!content.contains("blocked:"));
    }

    #[test]
    fn append_guard_rule_preserves_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-rules.yaml");
        // Seed an existing file with one blocked + one warned rule.
        std::fs::write(
            &path,
            "blocked:\n  - pattern: kubectl\\s+delete | msg: kubectl delete blocked\n\
             warned:\n  - pattern: docker\\s+prune | msg: prune warning\n",
        )
        .unwrap();

        append_guard_rule(&path, "block", r"terraform\s+destroy", "no destroy").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // Original entries preserved.
        assert!(content.contains(r"kubectl\s+delete"));
        assert!(content.contains("kubectl delete blocked"));
        assert!(content.contains(r"docker\s+prune"));
        assert!(content.contains("prune warning"));
        // New entry appended.
        assert!(content.contains(r"terraform\s+destroy"));
        assert!(content.contains("no destroy"));
    }

    #[test]
    fn append_guard_rule_round_trips_through_incumbent_parser() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-rules.yaml");
        append_guard_rule(&path, "block", r"rm\s+-rf\s+/", "nope").unwrap();
        append_guard_rule(&path, "warn", r"git\s+reset", "careful").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // The file MUST be re-readable by the incumbent guard parser.
        let (blocked, warned) = crate::shared::classify::parse_guard_rules(&content);
        assert_eq!(blocked.len(), 1);
        assert_eq!(warned.len(), 1);
        assert_eq!(blocked[0].msg, "nope");
        assert_eq!(warned[0].msg, "careful");
        assert!(blocked[0].pattern.is_match("rm -rf /"));
    }

    #[test]
    fn append_guard_rule_atomic_no_tmp_residue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-rules.yaml");
        append_guard_rule(&path, "block", "foo", "bar").unwrap();
        // Temp file must be gone after successful atomic rename.
        let entries = std::fs::read_dir(dir.path()).unwrap();
        let count = entries.count();
        assert_eq!(count, 1, "only the target file should remain");
    }
}
