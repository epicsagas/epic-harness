use std::fs;
use std::io::{self, IsTerminal, Write as IoWrite};
use std::path::{Path, PathBuf};

// ── Embedded integration files ────────────────────────────────────────────────

macro_rules! integration_files {
    ($tool:literal, [ $( ($rel:literal, $content:expr) ),* $(,)? ]) => {
        &[ $( ($rel, $content) ),* ]
    };
}

static CODEX_FILES: &[(&str, &str)] = integration_files!(
    "codex",
    [
        (
            "hooks.json",
            include_str!("../../integrations/codex/hooks.json")
        ),
        // config.toml: enables codex_hooks (off by default without this).
        (
            "config.toml",
            include_str!("../../integrations/codex/config.toml")
        ),
        // Prompts are the Codex slash-command mechanism (~/.codex/prompts/).
        // Note: Codex marks prompts as deprecated in favour of skills, but they
        // still provide named /prompts:check etc. shortcuts in the CLI/IDE UI.
        (
            "prompts/check.md",
            include_str!("../../integrations/codex/prompts/check.md")
        ),
        (
            "prompts/evolve.md",
            include_str!("../../integrations/codex/prompts/evolve.md")
        ),
        (
            "prompts/go.md",
            include_str!("../../integrations/codex/prompts/go.md")
        ),
        (
            "prompts/ship.md",
            include_str!("../../integrations/codex/prompts/ship.md")
        ),
        (
            "prompts/spec.md",
            include_str!("../../integrations/codex/prompts/spec.md")
        ),
        (
            "prompts/team.md",
            include_str!("../../integrations/codex/prompts/team.md")
        ),
        (
            "skills/context/SKILL.md",
            include_str!("../../integrations/codex/skills/context/SKILL.md")
        ),
        (
            "skills/document/SKILL.md",
            include_str!("../../integrations/codex/skills/document/SKILL.md")
        ),
        (
            "skills/perf/SKILL.md",
            include_str!("../../integrations/codex/skills/perf/SKILL.md")
        ),
        (
            "skills/secure/SKILL.md",
            include_str!("../../integrations/codex/skills/secure/SKILL.md")
        ),
        (
            "skills/simplify/SKILL.md",
            include_str!("../../integrations/codex/skills/simplify/SKILL.md")
        ),
        (
            "skills/tdd/SKILL.md",
            include_str!("../../integrations/codex/skills/tdd/SKILL.md")
        ),
        (
            "skills/verify/SKILL.md",
            include_str!("../../integrations/codex/skills/verify/SKILL.md")
        ),
        (
            "agents/auditor.md",
            include_str!("../../integrations/codex/agents/auditor.md")
        ),
        (
            "agents/builder.md",
            include_str!("../../integrations/codex/agents/builder.md")
        ),
        (
            "agents/planner.md",
            include_str!("../../integrations/codex/agents/planner.md")
        ),
        (
            "agents/reviewer.md",
            include_str!("../../integrations/codex/agents/reviewer.md")
        ),
    ]
);

static GEMINI_FILES: &[(&str, &str)] = integration_files!(
    "gemini",
    [
        (
            "settings.json",
            include_str!("../../integrations/gemini/settings.json")
        ),
        (
            "GEMINI.md",
            include_str!("../../integrations/gemini/GEMINI.md")
        ),
        (
            "commands/check.toml",
            include_str!("../../integrations/gemini/commands/check.toml")
        ),
        (
            "commands/evolve.toml",
            include_str!("../../integrations/gemini/commands/evolve.toml")
        ),
        (
            "commands/go.toml",
            include_str!("../../integrations/gemini/commands/go.toml")
        ),
        (
            "commands/ship.toml",
            include_str!("../../integrations/gemini/commands/ship.toml")
        ),
        (
            "commands/spec.toml",
            include_str!("../../integrations/gemini/commands/spec.toml")
        ),
        (
            "commands/team.toml",
            include_str!("../../integrations/gemini/commands/team.toml")
        ),
        (
            "skills/context/SKILL.md",
            include_str!("../../integrations/gemini/skills/context/SKILL.md")
        ),
        (
            "skills/document/SKILL.md",
            include_str!("../../integrations/gemini/skills/document/SKILL.md")
        ),
        (
            "skills/perf/SKILL.md",
            include_str!("../../integrations/gemini/skills/perf/SKILL.md")
        ),
        (
            "skills/secure/SKILL.md",
            include_str!("../../integrations/gemini/skills/secure/SKILL.md")
        ),
        (
            "skills/simplify/SKILL.md",
            include_str!("../../integrations/gemini/skills/simplify/SKILL.md")
        ),
        (
            "skills/tdd/SKILL.md",
            include_str!("../../integrations/gemini/skills/tdd/SKILL.md")
        ),
        (
            "skills/verify/SKILL.md",
            include_str!("../../integrations/gemini/skills/verify/SKILL.md")
        ),
        (
            "agents/auditor.md",
            include_str!("../../integrations/gemini/agents/auditor.md")
        ),
        (
            "agents/builder.md",
            include_str!("../../integrations/gemini/agents/builder.md")
        ),
        (
            "agents/planner.md",
            include_str!("../../integrations/gemini/agents/planner.md")
        ),
        (
            "agents/reviewer.md",
            include_str!("../../integrations/gemini/agents/reviewer.md")
        ),
    ]
);

static CURSOR_FILES: &[(&str, &str)] = integration_files!(
    "cursor",
    [
        (
            "hooks.json",
            include_str!("../../integrations/cursor/hooks.json")
        ),
        (
            "rules/harness-context.mdc",
            include_str!("../../integrations/cursor/rules/harness-context.mdc")
        ),
        (
            "rules/harness-skills.mdc",
            include_str!("../../integrations/cursor/rules/harness-skills.mdc")
        ),
        (
            "commands/check.md",
            include_str!("../../integrations/cursor/commands/check.md")
        ),
        (
            "commands/evolve.md",
            include_str!("../../integrations/cursor/commands/evolve.md")
        ),
        (
            "commands/go.md",
            include_str!("../../integrations/cursor/commands/go.md")
        ),
        (
            "commands/ship.md",
            include_str!("../../integrations/cursor/commands/ship.md")
        ),
        (
            "commands/spec.md",
            include_str!("../../integrations/cursor/commands/spec.md")
        ),
        (
            "commands/team.md",
            include_str!("../../integrations/cursor/commands/team.md")
        ),
        (
            "agents/auditor.md",
            include_str!("../../integrations/cursor/agents/auditor.md")
        ),
        (
            "agents/builder.md",
            include_str!("../../integrations/cursor/agents/builder.md")
        ),
        (
            "agents/planner.md",
            include_str!("../../integrations/cursor/agents/planner.md")
        ),
        (
            "agents/reviewer.md",
            include_str!("../../integrations/cursor/agents/reviewer.md")
        ),
    ]
);

static OPENCODE_FILES: &[(&str, &str)] = integration_files!(
    "opencode",
    [
        (
            "commands/check.md",
            include_str!("../../integrations/opencode/commands/check.md")
        ),
        (
            "commands/evolve.md",
            include_str!("../../integrations/opencode/commands/evolve.md")
        ),
        (
            "commands/go.md",
            include_str!("../../integrations/opencode/commands/go.md")
        ),
        (
            "commands/ship.md",
            include_str!("../../integrations/opencode/commands/ship.md")
        ),
        (
            "commands/spec.md",
            include_str!("../../integrations/opencode/commands/spec.md")
        ),
        (
            "commands/team.md",
            include_str!("../../integrations/opencode/commands/team.md")
        ),
        (
            "agents/builder.md",
            include_str!("../../integrations/opencode/agents/builder.md")
        ),
        (
            "agents/reviewer.md",
            include_str!("../../integrations/opencode/agents/reviewer.md")
        ),
        (
            "agents/auditor.md",
            include_str!("../../integrations/opencode/agents/auditor.md")
        ),
        (
            "agents/planner.md",
            include_str!("../../integrations/opencode/agents/planner.md")
        ),
        (
            "plugins/epic-harness.js",
            include_str!("../../integrations/opencode/plugins/epic-harness.js")
        ),
    ]
);

static CLINE_FILES: &[(&str, &str)] = integration_files!(
    "cline",
    [
        (
            "hooks/PreToolUse",
            include_str!("../../integrations/cline/hooks/PreToolUse")
        ),
        (
            "hooks/PostToolUse",
            include_str!("../../integrations/cline/hooks/PostToolUse")
        ),
        (
            "hooks/TaskStart",
            include_str!("../../integrations/cline/hooks/TaskStart")
        ),
        (
            "hooks/TaskResume",
            include_str!("../../integrations/cline/hooks/TaskResume")
        ),
        (
            "hooks/TaskCancel",
            include_str!("../../integrations/cline/hooks/TaskCancel")
        ),
        (
            "rules/epic-harness.md",
            include_str!("../../integrations/cline/rules/epic-harness.md")
        ),
    ]
);

static AIDER_FILES: &[(&str, &str)] = integration_files!(
    "aider",
    [
        (
            ".aider.conf.yml",
            include_str!("../../integrations/aider/.aider.conf.yml")
        ),
        (
            ".aider/CONVENTIONS.md",
            include_str!("../../integrations/aider/.aider/CONVENTIONS.md")
        ),
    ]
);

// ── Tool config ───────────────────────────────────────────────────────────────

struct ToolConfig {
    /// Destination directory (global default)
    global_dir: PathBuf,
    /// Destination directory override for --local
    local_dir: PathBuf,
    /// Files that live at project root, not inside the tool dir (e.g. GEMINI.md, AGENTS.md)
    root_files: &'static [&'static str],
    /// Files embedded in the binary
    files: &'static [(&'static str, &'static str)],
    /// Extra note shown after install
    note: Option<&'static str>,
    /// Files whose relative path starts with this prefix are written to `alt_dir` instead of
    /// `global_dir`. Used to route `skills/` to `~/.agents/skills/` for Codex and Gemini.
    alt_dir: Option<PathBuf>,
    alt_prefix: &'static str,
    /// Files that should never be overwritten if they already exist (e.g. config.toml).
    /// Unlike root_files these live inside the tool dir, not in cwd.
    preserve_files: &'static [&'static str],
    /// Files that must be made executable after writing (chmod +x on Unix).
    executable_files: &'static [&'static str],
}

fn tool_config(tool: &str) -> Option<ToolConfig> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match tool {
        "codex" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".codex"),
            local_dir: cwd.join(".codex"),
            root_files: &[],
            files: CODEX_FILES,
            note: None,
            // Codex discovers skills from ~/.agents/skills/, not ~/.codex/skills/.
            // See: https://developers.openai.com/codex/skills
            alt_dir: Some(PathBuf::from(&home).join(".agents")),
            alt_prefix: "skills/",
            // config.toml may contain user-customised settings — never overwrite.
            preserve_files: &["config.toml"],
            executable_files: &[],
        }),
        "gemini" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".gemini"),
            local_dir: cwd.join(".gemini"),
            root_files: &["GEMINI.md"],
            files: GEMINI_FILES,
            note: Some("If GEMINI.md already exists, append the section manually."),
            // Gemini CLI loads skills from ~/.gemini/skills/ — install directly there.
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
            executable_files: &[],
        }),
        "cursor" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".cursor"),
            local_dir: cwd.join(".cursor"),
            root_files: &[],
            files: CURSOR_FILES,
            note: Some("Requires Cursor 1.7+"),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
            executable_files: &[],
        }),
        "opencode" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".config").join("opencode"),
            local_dir: cwd.join(".opencode"),
            root_files: &[],
            files: OPENCODE_FILES,
            note: Some("Place plugins/epic-harness.js in your OpenCode plugin directory."),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
            executable_files: &[],
        }),
        "cline" => Some(ToolConfig {
            global_dir: PathBuf::from(&home)
                .join("Documents")
                .join("Cline")
                .join("Rules"),
            local_dir: cwd.join(".clinerules"),
            root_files: &[],
            files: CLINE_FILES,
            note: Some(
                "Hook scripts have been made executable. \
                 For global hooks, also copy hooks/ to ~/Documents/Cline/Rules/Hooks/.",
            ),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
            executable_files: &[
                "hooks/PreToolUse",
                "hooks/PostToolUse",
                "hooks/TaskStart",
                "hooks/TaskResume",
                "hooks/TaskCancel",
            ],
        }),
        // Aider has no hook system. We install:
        //  - ~/.aider.conf.yml  (auto-loads conventions; preserved if already exists)
        //  - ~/.aider/CONVENTIONS.md  (coding rules injected into every session)
        // global_dir = $HOME so both paths resolve correctly.
        "aider" => Some(ToolConfig {
            global_dir: PathBuf::from(&home),
            local_dir: cwd.clone(),
            root_files: &[],
            files: AIDER_FILES,
            note: Some("No hook system available. Conventions are loaded via .aider.conf.yml."),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[".aider.conf.yml"],
            executable_files: &[],
        }),
        _ => None,
    }
}

// ── Progress bar ──────────────────────────────────────────────────────────────

struct Progress {
    tool: String,
    total: usize,
    current: usize,
    added: usize,
    updated: usize,
    unchanged: usize,
    dry_run: bool,
    tty: bool,
}

impl Progress {
    fn new(tool: &str, total: usize, dry_run: bool) -> Self {
        Self {
            tool: tool.to_string(),
            total,
            current: 0,
            added: 0,
            updated: 0,
            unchanged: 0,
            dry_run,
            tty: io::stderr().is_terminal(),
        }
    }

    fn tick(&mut self, filename: &str, status: FileStatus) {
        self.current += 1;
        match status {
            FileStatus::Added => self.added += 1,
            FileStatus::Updated => self.updated += 1,
            FileStatus::Unchanged => self.unchanged += 1,
        }

        if self.tty {
            let filled = if self.total > 0 {
                (self.current * 20) / self.total
            } else {
                20
            };
            let bar: String = std::iter::repeat_n('=', filled.saturating_sub(1))
                .chain(if filled > 0 && filled < 20 {
                    std::iter::once('>')
                } else {
                    std::iter::once('=')
                })
                .chain(std::iter::repeat_n(' ', 20 - filled))
                .collect();

            let name = if filename.len() > 26 {
                &filename[filename.len() - 26..]
            } else {
                filename
            };

            let tag = if self.dry_run { "dry-run" } else { &self.tool };
            eprint!(
                "\r  {:<8} [{}] {:>2}/{:<2}  {:<26}",
                tag, bar, self.current, self.total, name
            );
            let _ = io::stderr().flush();
        } else {
            // Non-TTY (CI / piped): compact one-line summary per tool, not per file
        }
    }

    fn finish(&self) {
        let dry = if self.dry_run { " (dry-run)" } else { "" };
        if self.tty {
            eprint!("\r{}\r", " ".repeat(60)); // clear bar line
            eprintln!(
                "  {:<8} ✓ {} files{}  ({} added, {} updated, {} unchanged)",
                self.tool, self.total, dry, self.added, self.updated, self.unchanged
            );
        } else {
            eprintln!(
                "[harness] {}: {} files{}  ({} added, {} updated, {} unchanged)",
                self.tool, self.total, dry, self.added, self.updated, self.unchanged
            );
        }
    }
}

#[derive(Clone, Copy)]
enum FileStatus {
    Added,
    Updated,
    Unchanged,
}

// ── Install logic ─────────────────────────────────────────────────────────────

/// Root-only files (GEMINI.md, AGENTS.md): never overwrite — user may have edited or merged.
/// Returns the FileStatus so the caller can update progress.
fn write_if_missing(dest: &Path, content: &str, dry_run: bool) -> FileStatus {
    if dest.exists() {
        return FileStatus::Unchanged;
    }
    if dry_run {
        return FileStatus::Added;
    }
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(dest, content) {
        Ok(_) => FileStatus::Added,
        Err(e) => {
            eprintln!("\n[harness] ERROR writing {}: {e}", dest.display());
            FileStatus::Unchanged
        }
    }
}

/// Hooks, commands, agents, rules, skills, etc.: write if missing or content differs from embedded.
fn write_or_sync(dest: &Path, content: &str, dry_run: bool) -> FileStatus {
    let existed = dest.exists();
    let is_settings_json = dest.file_name().is_some_and(|n| n == "settings.json");

    if is_settings_json && existed {
        // For settings.json, merge instead of overwriting to preserve theme, auth, etc.
        let existing_content = fs::read_to_string(dest).unwrap_or_default();

        let mut existing_json: serde_json::Value =
            serde_json::from_str(&existing_content).unwrap_or(serde_json::json!({}));
        let new_json: serde_json::Value =
            serde_json::from_str(content).unwrap_or(serde_json::json!({}));

        if let Some(v) = new_json.get("hooksConfig") {
            existing_json["hooksConfig"] = v.clone();
        }
        if let Some(v) = new_json.get("hooks") {
            existing_json["hooks"] = v.clone();
        }

        let merged =
            serde_json::to_string_pretty(&existing_json).unwrap_or_else(|_| content.to_string());

        if existing_content == merged {
            return FileStatus::Unchanged;
        }
        if dry_run {
            return FileStatus::Updated;
        }
        match fs::write(dest, merged) {
            Ok(_) => return FileStatus::Updated,
            Err(e) => eprintln!("\n[harness] ERROR merging {}: {e}", dest.display()),
        }
        return FileStatus::Unchanged;
    }

    let unchanged = existed
        && fs::read_to_string(dest)
            .map(|existing| existing == content)
            .unwrap_or(false);

    if unchanged {
        return FileStatus::Unchanged;
    }
    if dry_run {
        return if existed {
            FileStatus::Updated
        } else {
            FileStatus::Added
        };
    }
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(dest, content) {
        Ok(_) => {
            if existed {
                FileStatus::Updated
            } else {
                FileStatus::Added
            }
        }
        Err(e) => {
            eprintln!("\n[harness] ERROR writing {}: {e}", dest.display());
            FileStatus::Unchanged
        }
    }
}

/// Make a file executable on Unix (no-op on other platforms).
fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path; // no-op on Windows
    }
}

// ── MCP injection ─────────────────────────────────────────────────────────────

static MEM_MCP_CJS: &str = include_str!("../../hooks/scripts/mem-mcp.cjs");

fn harness_bin_dir() -> PathBuf {
    let root = std::env::var("HARNESS_ROOT")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(root).join(".harness").join("bin")
}

/// Returns path to mem-mcp.cjs, extracting the embedded copy to ~/.harness/bin/ if needed.
fn find_or_extract_mcp_cjs() -> Option<PathBuf> {
    // 1. Cargo dev build: target/debug -> target -> repo root
    if let Ok(exe) = std::env::current_exe()
        && let Some(c) = exe.parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|repo| repo.join("hooks").join("scripts").join("mem-mcp.cjs"))
            .filter(|c| c.exists())
    {
        return Some(c);
    }

    // 2. ~/.harness/bin/mem-mcp.cjs — already extracted
    let dest = harness_bin_dir().join("mem-mcp.cjs");
    if dest.exists() {
        return Some(dest);
    }

    // 3. Extract embedded copy
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    fs::write(&dest, MEM_MCP_CJS).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
    }
    Some(dest)
}

/// Injects `mcpServers.harness-mem` into the tool's settings JSON file.
/// Silently skips if the settings file doesn't exist or already has the entry.
fn inject_mcp(tool: &str, target_dir: &Path) {
    let mcp_cjs = match find_or_extract_mcp_cjs() {
        Some(p) => p,
        None => {
            eprintln!(
                "[harness] Note: failed to extract mem-mcp.cjs — skipping MCP registration.\n\
                 [harness] Run 'epic-harness mem mcp-install --path <path/to/mem-mcp.cjs>' manually."
            );
            return;
        }
    };

    let settings_path = match tool {
        "codex"    => None, // Codex uses hooks.json, no mcpServers concept
        "gemini"   => Some(target_dir.join("settings.json")),
        "cursor"   => Some(target_dir.join("mcp.json")),
        "opencode" => Some(target_dir.join("config.json")),
        "cline"    => None, // Cline MCP is configured per-workspace, not via global install
        "aider"    => None, // No MCP support
        _          => None,
    };

    let settings_path = match settings_path {
        Some(p) => p,
        None => return, // tool doesn't support MCP via a settings file
    };

    let raw = if settings_path.exists() {
        fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    // Already registered — don't overwrite
    if json["mcpServers"]["harness-mem"].is_object() {
        eprintln!("[harness] mcpServers.harness-mem already registered in {tool} settings — skipping.");
        return;
    }

    json["mcpServers"]["harness-mem"] = serde_json::json!({
        "command": "node",
        "args": [mcp_cjs.to_string_lossy()]
    });

    let out = serde_json::to_string_pretty(&json).unwrap_or_else(|_| raw.clone());

    if let Some(parent) = settings_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = settings_path.with_extension("tmp");
    if fs::write(&tmp, &out).is_ok() && fs::rename(&tmp, &settings_path).is_ok() {
        eprintln!(
            "[harness] Registered mcpServers.harness-mem in {}",
            settings_path.display()
        );
    }
}

// ── Interactive menu ──────────────────────────────────────────────────────────

const TOOLS: &[(&str, &str)] = &[
    ("codex", "OpenAI Codex CLI"),
    ("gemini", "Google Gemini CLI"),
    ("cursor", "Cursor IDE"),
    ("opencode", "OpenCode"),
    ("cline", "Cline (VS Code)"),
    ("aider", "Aider"),
];

fn interactive_menu() -> Vec<String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        match super::install_wizard::interactive_select_tools(TOOLS) {
            Ok(selected) => selected,
            Err(e) => {
                eprintln!("[harness] Interactive UI failed ({e}); falling back to text prompt.");
                interactive_menu_fallback()
            }
        }
    } else {
        interactive_menu_fallback()
    }
}

/// Non-TTY (CI, pipes): comma-separated indices or `a` / `all` for everything.
fn interactive_menu_fallback() -> Vec<String> {
    eprintln!();
    eprintln!("epic-harness — Select integrations to install");
    eprintln!("──────────────────────────────────────────────");
    for (i, (name, desc)) in TOOLS.iter().enumerate() {
        eprintln!("  [{}] {:<12} {}", i + 1, name, desc);
    }
    eprintln!("  [a] All of the above");
    eprintln!();
    eprint!("Selection (e.g. 1,3 or a): ");
    let _ = io::stderr().flush();

    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return vec![];
    }
    let line = line.trim().to_lowercase();

    if line == "a" || line == "all" {
        return TOOLS.iter().map(|(name, _)| name.to_string()).collect();
    }

    let mut selected = Vec::new();
    for token in line.split(',') {
        let token = token.trim();
        if let Ok(n) = token.parse::<usize>()
            && n >= 1 && n <= TOOLS.len()
        {
            selected.push(TOOLS[n - 1].0.to_string());
        }
    }
    selected
}

// ── Install a single tool ─────────────────────────────────────────────────────

fn install_tool(tool: &str, local: bool, dry_run: bool) -> i32 {
    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: codex, gemini, cursor, opencode, cline, aider"
            );
            return 1;
        }
    };

    let target_dir = if local {
        &cfg.local_dir
    } else {
        &cfg.global_dir
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(note) = cfg.note {
        eprintln!("[harness] Note: {note}");
    }

    // Resolve alt_dir for --local installs
    let alt_target: Option<PathBuf> = cfg.alt_dir.as_ref().map(|global_alt| {
        if local {
            cwd.join(
                global_alt
                    .file_name()
                    .unwrap_or(std::ffi::OsStr::new("agents")),
            )
        } else {
            global_alt.clone()
        }
    });

    let mut progress = Progress::new(tool, cfg.files.len(), dry_run);

    for (rel, content) in cfg.files {
        let dest = if !cfg.alt_prefix.is_empty() && rel.starts_with(cfg.alt_prefix) {
            if let Some(alt) = &alt_target {
                alt.join(rel)
            } else if cfg.root_files.contains(rel) {
                cwd.join(rel)
            } else {
                target_dir.join(rel)
            }
        } else if cfg.root_files.contains(rel) {
            cwd.join(rel)
        } else {
            target_dir.join(rel)
        };

        let status = if cfg.root_files.contains(rel) || cfg.preserve_files.contains(rel) {
            write_if_missing(&dest, content, dry_run)
        } else {
            write_or_sync(&dest, content, dry_run)
        };

        // chmod +x for executable files (e.g. Cline hook scripts)
        if !dry_run && cfg.executable_files.contains(rel) {
            make_executable(&dest);
        }

        progress.tick(rel, status);
    }

    progress.finish();

    // Inject harness-mem MCP server entry into the tool's settings file.
    if !dry_run {
        inject_mcp(tool, target_dir);
    } else {
        eprintln!("[harness] dry-run: would inject mcpServers.harness-mem into {tool} settings");
    }

    // Codex-specific: warn if config.toml exists but codex_hooks is not enabled.
    if tool == "codex" {
        let config_path = target_dir.join("config.toml");
        if config_path.exists() {
            let ok = fs::read_to_string(&config_path)
                .map(|s| s.contains("codex_hooks"))
                .unwrap_or(false);
            if !ok {
                eprintln!();
                eprintln!("[harness] WARNING: ~/.codex/config.toml exists but does not enable hooks.");
                eprintln!("[harness] Hooks are OFF by default. Add these lines to enable them:");
                eprintln!();
                eprintln!("    [features]");
                eprintln!("    codex_hooks = true");
                eprintln!();
                eprintln!("[harness] Then restart Codex for the change to take effect.");
            }
        }
    }

    0
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn run(args: &[String]) -> i32 {
    // Parse: epic-harness install [<tool>] [--local] [--dry-run]
    let local = args.iter().any(|a| a == "--local");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    // First positional arg that isn't a flag
    let tool_arg = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str());

    match tool_arg {
        None => {
            // Interactive menu
            let selected = interactive_menu();
            if selected.is_empty() {
                eprintln!("[harness] No integrations selected.");
                return 0;
            }
            let mut exit = 0;
            for tool in &selected {
                eprintln!("[harness] Installing {tool}...");
                let code = install_tool(tool, local, dry_run);
                if code != 0 {
                    exit = code;
                }
            }
            exit
        }

        Some("--list" | "list") => {
            println!("Available integrations: codex, gemini, cursor, opencode, cline, aider");
            0
        }

        Some(tool) => install_tool(tool, local, dry_run),
    }
}

// ── Uninstall ─────────────────────────────────────────────────────────────────

fn uninstall_tool(tool: &str, local: bool, dry_run: bool) -> i32 {
    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: codex, gemini, cursor, opencode, cline, aider"
            );
            return 1;
        }
    };

    let target_dir = if local { &cfg.local_dir } else { &cfg.global_dir };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let alt_target: Option<PathBuf> = cfg.alt_dir.as_ref().map(|global_alt| {
        if local {
            cwd.join(global_alt.file_name().unwrap_or(std::ffi::OsStr::new("agents")))
        } else {
            global_alt.clone()
        }
    });

    let mut removed = 0usize;
    let mut skipped = 0usize;

    for (rel, _) in cfg.files {
        // Resolve destination path (mirrors install logic)
        let dest = if !cfg.alt_prefix.is_empty() && rel.starts_with(cfg.alt_prefix) {
            if let Some(alt) = &alt_target {
                alt.join(rel)
            } else {
                target_dir.join(rel)
            }
        } else if cfg.root_files.contains(rel) {
            cwd.join(rel)
        } else {
            target_dir.join(rel)
        };

        // Never auto-delete root files (e.g. GEMINI.md) — user may have edited them.
        if cfg.root_files.contains(rel) {
            eprintln!("  skip  {}", dest.display());
            skipped += 1;
            continue;
        }

        if dest.exists() {
            if !dry_run {
                if let Err(e) = fs::remove_file(&dest) {
                    eprintln!("\n[harness] ERROR removing {}: {e}", dest.display());
                } else {
                    removed += 1;
                }
            } else {
                removed += 1;
            }
        }
    }

    // Prune empty directories left behind
    if !dry_run {
        let dirs_to_try: Vec<PathBuf> = cfg
            .files
            .iter()
            .filter_map(|(rel, _)| {
                let dest = target_dir.join(rel);
                dest.parent().map(|p| p.to_path_buf())
            })
            .collect();
        for dir in dirs_to_try {
            let _ = fs::remove_dir(&dir); // silently ignore non-empty
        }
        let _ = fs::remove_dir(target_dir);
    }

    let dry = if dry_run { " (dry-run)" } else { "" };
    eprintln!(
        "  {:<8} ✓ removed {removed} files{dry}  ({skipped} root files skipped — delete manually if needed)",
        tool
    );
    0
}

pub fn run_uninstall(args: &[String]) -> i32 {
    let local = args.iter().any(|a| a == "--local");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let tool_arg = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str());

    match tool_arg {
        None => {
            let selected = interactive_menu();
            if selected.is_empty() {
                eprintln!("[harness] No integrations selected.");
                return 0;
            }
            let mut exit = 0;
            for tool in &selected {
                eprintln!("[harness] Uninstalling {tool}...");
                let code = uninstall_tool(tool, local, dry_run);
                if code != 0 {
                    exit = code;
                }
            }
            exit
        }
        Some("--list" | "list") => {
            println!("Available integrations: codex, gemini, cursor, opencode, cline, aider");
            0
        }
        Some(tool) => uninstall_tool(tool, local, dry_run),
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("epic_test_{}_{}", std::process::id(), rand_suffix()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Cheap non-crypto suffix so parallel tests don't collide.
    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0)
    }

    // ── write_if_missing ──────────────────────────────────────────────────────

    #[test]
    fn test_write_if_missing_creates_new_file() {
        let dir = tmp_dir();
        let dest = dir.join("new.md");
        let status = write_if_missing(&dest, "hello", false);
        assert!(matches!(status, FileStatus::Added));
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "hello");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_if_missing_skips_existing() {
        let dir = tmp_dir();
        let dest = dir.join("existing.md");
        fs::write(&dest, "original").unwrap();
        let status = write_if_missing(&dest, "new content", false);
        assert!(matches!(status, FileStatus::Unchanged));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original");
        let _ = fs::remove_dir_all(dir);
    }

    // ── write_or_sync ─────────────────────────────────────────────────────────

    #[test]
    fn test_write_or_sync_creates_new() {
        let dir = tmp_dir();
        let dest = dir.join("brand_new.txt");
        let status = write_or_sync(&dest, "content", false);
        assert!(matches!(status, FileStatus::Added));
        assert!(dest.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_or_sync_updates_changed() {
        let dir = tmp_dir();
        let dest = dir.join("changed.txt");
        fs::write(&dest, "old").unwrap();
        let status = write_or_sync(&dest, "new", false);
        assert!(matches!(status, FileStatus::Updated));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_or_sync_unchanged_same_content() {
        let dir = tmp_dir();
        let dest = dir.join("same.txt");
        fs::write(&dest, "identical").unwrap();
        let status = write_or_sync(&dest, "identical", false);
        assert!(matches!(status, FileStatus::Unchanged));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_or_sync_dry_run_no_write() {
        let dir = tmp_dir();
        let dest = dir.join("dry.txt");
        // File does not exist; dry_run should return Added but not create file.
        let status = write_or_sync(&dest, "content", true);
        assert!(matches!(status, FileStatus::Added));
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_or_sync_merges_settings_json() {
        let dir = tmp_dir();
        let dest = dir.join("settings.json");
        // Existing file has a user key that must survive the merge.
        fs::write(
            &dest,
            r#"{"theme":"dark","hooksConfig":{"old":true}}"#,
        )
        .unwrap();
        let new_content = r#"{"hooksConfig":{"new":true}}"#;
        let status = write_or_sync(&dest, new_content, false);
        assert!(matches!(status, FileStatus::Updated));
        let written = fs::read_to_string(&dest).unwrap();
        let v: serde_json::Value = serde_json::from_str(&written).unwrap();
        // Existing key preserved.
        assert_eq!(v["theme"], "dark");
        // hooksConfig updated to new value.
        assert_eq!(v["hooksConfig"]["new"], true);
        // Old hooksConfig key gone (replaced, not merged within hooksConfig).
        assert!(v["hooksConfig"]["old"].is_null());
        let _ = fs::remove_dir_all(dir);
    }
}
