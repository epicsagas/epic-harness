use std::fs;
use std::io::{self, ErrorKind, IsTerminal, Write as IoWrite};
use std::path::{Path, PathBuf};

// ── Canonical sources (single source of truth) ──────────────────────────────

static SKILL_COMMIT: &str = include_str!("../registry/skills/commit/SKILL.md");
static SKILL_CONTEXT: &str = include_str!("../registry/skills/context/SKILL.md");
static SKILL_DEBUG: &str = include_str!("../registry/skills/debug/SKILL.md");
static SKILL_DOCUMENT: &str = include_str!("../registry/skills/document/SKILL.md");
static SKILL_PERF: &str = include_str!("../registry/skills/perf/SKILL.md");
static SKILL_SECURE: &str = include_str!("../registry/skills/secure/SKILL.md");
static SKILL_SIMPLIFY: &str = include_str!("../registry/skills/simplify/SKILL.md");
static SKILL_TDD: &str = include_str!("../registry/skills/tdd/SKILL.md");
static SKILL_VERIFY: &str = include_str!("../registry/skills/verify/SKILL.md");
static SKILL_COUNCIL: &str = include_str!("../registry/skills/council/SKILL.md");
static SKILL_AGENT_INTROSPECTION: &str =
    include_str!("../registry/skills/agent-introspection/SKILL.md");
static SKILL_REFLECT: &str = include_str!("../registry/skills/reflect/SKILL.md");
static SKILL_DISCOVER: &str = include_str!("../registry/skills/discover/SKILL.md");
static SKILL_ORCHESTRATE: &str = include_str!("../registry/skills/orchestrate/SKILL.md");
static SKILL_SPEC: &str = include_str!("../registry/skills/spec/SKILL.md");
static SKILL_GO: &str = include_str!("../registry/skills/go/SKILL.md");
static SKILL_AUDIT: &str = include_str!("../registry/skills/audit/SKILL.md");
static SKILL_SHIP: &str = include_str!("../registry/skills/ship/SKILL.md");
static SKILL_ORBIT: &str = include_str!("../registry/skills/orbit/SKILL.md");
static SKILL_EVOLVE: &str = include_str!("../registry/skills/evolve/SKILL.md");
static SKILL_TEAM: &str = include_str!("../registry/skills/team/SKILL.md");
static CANONICAL_SKILLS: &[(&str, &str)] = &[
    ("commit", SKILL_COMMIT),
    ("context", SKILL_CONTEXT),
    ("debug", SKILL_DEBUG),
    ("document", SKILL_DOCUMENT),
    ("perf", SKILL_PERF),
    ("secure", SKILL_SECURE),
    ("simplify", SKILL_SIMPLIFY),
    ("tdd", SKILL_TDD),
    ("verify", SKILL_VERIFY),
    ("council", SKILL_COUNCIL),
    ("agent-introspection", SKILL_AGENT_INTROSPECTION),
    ("reflect", SKILL_REFLECT),
    ("discover", SKILL_DISCOVER),
    ("orchestrate", SKILL_ORCHESTRATE),
    ("spec", SKILL_SPEC),
    ("go", SKILL_GO),
    ("audit", SKILL_AUDIT),
    ("ship", SKILL_SHIP),
    ("orbit", SKILL_ORBIT),
    ("evolve", SKILL_EVOLVE),
    ("team", SKILL_TEAM),
];

// ── Per-skill Memory Integration sections (appended for codex) ──────────────

static MEM_SECTION_COMMIT: &str = "";
static MEM_SECTION_CONTEXT: &str = r#"
**CRITICAL**: Run `HARNESS_DIR=$(epic path)` first. NEVER use `.harness/` in the project directory.
"#;
static MEM_SECTION_DEBUG: &str = "";
static MEM_SECTION_DOCUMENT: &str = r#"
## Memory Integration

Check existing memory before writing docs to avoid duplication:
```
epic-harness mem search "<module or function name>"
# or via MCP: mem_search(query="<module or function name>")
```
"#;
static MEM_SECTION_PERF: &str = r#"
## Memory Integration

**Before review**: Check known performance patterns.
```
epic-harness mem search "performance" --limit 5
# or via MCP: mem_search(query="performance")
```

**After review** (if a perf pattern or bottleneck found):
```
epic-harness mem add --title "<pattern>" --type pattern --tags "performance" --body "<finding and fix>"
# or via MCP: mem_add(title="...", type="pattern", tags=["performance"], body="...")
```
"#;
static MEM_SECTION_SECURE: &str = r#"
## Memory Integration

**Before review**: Check known security patterns.
```
epic-harness mem search "security" --limit 5
# or via MCP: mem_search(query="security")
```

**After review** (if a security decision was made):
```
epic-harness mem add --title "<decision>" --type decision --tags "security" --body "<rationale>"
# or via MCP: mem_add(title="...", type="decision", tags=["security"], body="...")
```
"#;
static MEM_SECTION_SIMPLIFY: &str = r#"
## Memory Integration

If a significant architectural insight emerged from simplification:
```
epic-harness mem add --title "<insight>" --type concept --tags "architecture,refactor" --body "<what was simplified and why>"
# or via MCP: mem_add(title="...", type="concept", tags=["architecture"], body="...")
```
"#;
static MEM_SECTION_TDD: &str = r#"
## Memory Integration

**Session start**: Load relevant patterns before implementing.
```
epic-harness mem search "<feature keyword>"
# or via MCP: mem_search(query="<feature keyword>")
```

**After refactor** (if a notable pattern emerged):
```
epic-harness mem add --title "<pattern name>" --type pattern --tags "<stack>,tdd" --body "<what was learned>"
# or via MCP: mem_add(title="...", type="pattern", body="...", tags=[...])
```
"#;
static MEM_SECTION_VERIFY: &str = r#"
## Memory Integration

**Before verifying**: Load project context.
```
epic-harness mem context --project <current-project>
# or via MCP: mem_context(project="<current-project>")
```

**If bugs/regressions found**: Record as error node.
```
epic-harness mem add --title "<bug description>" --type error --tags "<component>" --body "<root cause and fix>"
# or via MCP: mem_add(title="...", type="error", body="...", tags=[...])
```
"#;

fn mem_section_for_skill(name: &str) -> &'static str {
    match name {
        "commit" => MEM_SECTION_COMMIT,
        "context" => MEM_SECTION_CONTEXT,
        "debug" => MEM_SECTION_DEBUG,
        "document" => MEM_SECTION_DOCUMENT,
        "perf" => MEM_SECTION_PERF,
        "secure" => MEM_SECTION_SECURE,
        "simplify" => MEM_SECTION_SIMPLIFY,
        "tdd" => MEM_SECTION_TDD,
        "verify" => MEM_SECTION_VERIFY,
        _ => "",
    }
}

// ── Transform functions ─────────────────────────────────────────────────────

/// Strip YAML frontmatter from a markdown document, returning just the body.
fn strip_frontmatter(md: &str) -> &str {
    if let Some(rest) = md.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let after = end + 4; // skip past \n---
        // Skip the trailing newline after closing ---
        if after < rest.len() && rest.as_bytes()[after] == b'\n' {
            return &rest[after + 1..];
        }
        if after < rest.len() {
            return &rest[after..];
        }
        return "";
    }
    md
}

/// Transform a canonical skill for a specific tool.
fn transform_skill(tool: &str, name: &str, canonical: &str) -> String {
    match tool {
        "codex" => {
            let mut result = canonical.to_string();

            // For context skill: apply inline mem-integration edits
            if name == "context" {
                result = result.replace(
                    "- Key decisions made \u{2192} note in conversation",
                    "- Key decisions made \u{2192} **save to memory before compacting**:\n  ```\n  epic-harness mem add --title \"<decision>\" --type decision --tags \"<project>\" --body \"<context and rationale>\"\n  # or via MCP: mem_add(title=\"...\", type=\"decision\", body=\"...\")\n  ```",
                );
                result = result.replace(
                    "- Project memory from `$HARNESS_DIR/memory/`",
                    "- Project memory from `~/.harness/memory.db` via `resume` hook",
                );
                // Add reload context after the evolved skills line
                result = result.replace(
                    "- Evolved skills from `$HARNESS_DIR/evolved/`",
                    "- Evolved skills from `$HARNESS_DIR/evolved/`\n- Reload project context manually if needed:\n  ```\n  epic-harness mem context --project <current-project>\n  # or via MCP: mem_context(project=\"<current-project>\")\n  ```",
                );
                // Add mem evidence item
                result = result.replace(
                    "- [ ] Snapshot written to `$HARNESS_DIR/sessions/` (show file name)",
                    "- [ ] Key decisions saved to memory (show mem add output or MCP call)\n- [ ] Snapshot written to `$HARNESS_DIR/sessions/` (show file name)",
                );
            }

            // Minor text normalizations
            if name == "tdd" || name == "verify" {
                result = result.replace("subagents", "sub-agents");
                result = result.replace("subagent", "sub-agent");
            }

            // Insert CRITICAL HARNESS_DIR line for context
            if name == "context" {
                let critical_line = "\n**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.\n";
                // Insert after frontmatter closing ---
                if let Some(pos) = result[3..].find("\n---\n") {
                    let insert_at = 3 + pos + 5;
                    result.insert_str(insert_at, critical_line);
                }
            }

            // Append Memory Integration section
            let mem_section = mem_section_for_skill(name);
            if !mem_section.is_empty() && name != "context" {
                // context's mem section is handled inline above
                result = format!("{}{}", result.trim_end(), mem_section);
            }

            result
        }
        _ => canonical.to_string(),
    }
}

/// Build the cursor harness-skills.mdc from canonical skills.
fn build_cursor_skills_mdc() -> String {
    let mut out = String::from(
        "---\ndescription: \"epic-harness quality skills \u{2014} TDD, security, verify, simplify, perf. Apply when implementing features, touching auth/DB/API code, or before marking tasks done.\"\nalwaysApply: false\n---\n# epic-harness Quality Skills\n\nCore skill rules applied automatically throughout every session.\n",
    );
    for (name, content) in CANONICAL_SKILLS {
        // Skip debug and context for cursor (they are operational, not quality skills)
        if *name == "debug" || *name == "context" {
            continue;
        }
        let body = strip_frontmatter(content);
        out.push_str(&format!("\n---\n\n{}\n", body.trim()));
    }
    out
}

// ── Embedded integration files ────────────────────────────────────────────────

macro_rules! integration_files {
    ($tool:literal, [ $( ($rel:literal, $content:expr) ),* $(,)? ]) => {
        &[ $( ($rel, $content) ),* ]
    };
}

static HARNESS_MD: &str = include_str!("../integrations/common/HARNESS.md");

static CURSOR_FILES: &[(&str, &str)] = integration_files!(
    "cursor",
    [
        (
            "hooks.json",
            include_str!("../integrations/cursor/hooks.json")
        ),
        (
            "rules/harness-context.mdc",
            include_str!("../integrations/cursor/rules/harness-context.mdc")
        ),
    ]
);

static OPENCODE_FILES: &[(&str, &str)] = integration_files!(
    "opencode",
    [(
        "plugins/epic-harness.js",
        include_str!("../integrations/opencode/plugins/epic-harness.js")
    ),]
);

static CLINE_FILES: &[(&str, &str)] = integration_files!(
    "cline",
    [
        (
            "hooks/PreToolUse",
            include_str!("../integrations/cline/hooks/PreToolUse")
        ),
        (
            "hooks/PostToolUse",
            include_str!("../integrations/cline/hooks/PostToolUse")
        ),
        (
            "hooks/TaskStart",
            include_str!("../integrations/cline/hooks/TaskStart")
        ),
        (
            "hooks/TaskResume",
            include_str!("../integrations/cline/hooks/TaskResume")
        ),
        (
            "hooks/TaskCancel",
            include_str!("../integrations/cline/hooks/TaskCancel")
        ),
        (
            "rules/epic-harness.md",
            include_str!("../integrations/cline/rules/epic-harness.md")
        ),
    ]
);

static AIDER_FILES: &[(&str, &str)] = integration_files!(
    "aider",
    [
        (
            ".aider.conf.yml",
            include_str!("../integrations/aider/.aider.conf.yml")
        ),
        (
            ".aider/CONVENTIONS.md",
            include_str!("../integrations/aider/.aider/CONVENTIONS.md")
        ),
    ]
);

// ── Tool config ───────────────────────────────────────────────────────────────

struct ToolConfig {
    /// Destination directory (global default)
    global_dir: PathBuf,
    /// Destination directory override for --local
    local_dir: PathBuf,
    /// Files that live at project root (e.g. AGENTS.md)
    root_files: &'static [&'static str],
    /// Files embedded in the binary
    files: &'static [(&'static str, &'static str)],
    /// Extra note shown after install
    note: Option<&'static str>,
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
        "cursor" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".cursor"),
            local_dir: cwd.join(".cursor"),
            root_files: &[],
            files: CURSOR_FILES,
            note: Some("Requires Cursor 1.7+"),
            preserve_files: &[],
            executable_files: &[],
        }),
        "opencode" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".config").join("opencode"),
            local_dir: cwd.join(".opencode"),
            root_files: &[],
            files: OPENCODE_FILES,
            note: Some("Place plugins/epic-harness.js in your OpenCode plugin directory."),
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
            let filled = (self.current * 20).checked_div(self.total).unwrap_or(20);
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

/// Root-only files (e.g. AGENTS.md): never overwrite — user may have edited or merged.
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
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            eprintln!(
                "\n[harness] WARN: permission denied writing {} — \
                 grant Full Disk Access to your terminal in System Settings > Privacy & Security, \
                 or skip global install.",
                dest.display()
            );
            FileStatus::Unchanged
        }
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
        && match fs::read_to_string(dest) {
            Ok(existing) => existing == content,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                // File exists but we can't read it (macOS TCC). Assume unchanged —
                // a terminal with Full Disk Access likely installed it already.
                return FileStatus::Unchanged;
            }
            Err(_) => false,
        };

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
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            eprintln!(
                "\n[harness] WARN: permission denied writing {} — \
                 grant Full Disk Access to your terminal in System Settings > Privacy & Security, \
                 or skip global install.",
                dest.display()
            );
            FileStatus::Unchanged
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

/// Prints MCP setup guidance instead of auto-injecting config (skill-driven mode).
fn inject_mcp(_tool: &str, _target_dir: &Path) {
    eprintln!("[harness] MCP server not auto-configured (skill-driven mode).");
    eprintln!("   To use MCP, see registry/mcp.json for manual setup.");
}

// ── Interactive menu ──────────────────────────────────────────────────────────

const TOOLS: &[(&str, &str)] = &[
    ("cursor", "Cursor IDE"),
    ("opencode", "OpenCode"),
    ("cline", "Cline (VS Code)"),
    ("aider", "Aider"),
];

fn interactive_menu() -> Vec<String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        match crate::install_wizard::interactive_select_tools(TOOLS) {
            // telemetry consent already handled inside interactive_select_tools
            Ok(selected) => selected,
            Err(e) => {
                eprintln!("[harness] Interactive UI failed ({e}); falling back to text prompt.");
                let selected = interactive_menu_fallback();
                prompt_and_save_telemetry_consent();
                selected
            }
        }
    } else {
        let selected = interactive_menu_fallback();
        prompt_and_save_telemetry_consent();
        selected
    }
}

fn prompt_and_save_telemetry_consent() {
    let level = crate::telemetry::prompt_consent_interactive();
    crate::telemetry::write_consent(level);
    match level {
        crate::telemetry::ConsentLevel::On => {
            eprintln!("[harness] Telemetry enabled. To opt out: epic-harness telemetry off");
        }
        crate::telemetry::ConsentLevel::Off => {
            eprintln!("[harness] Telemetry disabled. To enable: epic-harness telemetry on");
        }
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
            && n >= 1
            && n <= TOOLS.len()
        {
            selected.push(TOOLS[n - 1].0.to_string());
        }
    }
    selected
}

// ── Canonical file generation ─────────────────────────────────────────────────

/// Remove legacy command/agent files from a previous installation.
/// Called during install to clean up files that were absorbed into skills.
/// Returns the number of removed files for migration reporting.
fn cleanup_legacy_files(target_dir: &Path) -> u32 {
    let legacy_commands = [
        "discover",
        "spec",
        "go",
        "audit",
        "ship",
        "intervene",
        "status",
        "orbit",
        "evolve",
        "team",
    ];
    let legacy_agents = ["builder", "reviewer", "auditor", "planner"];
    let mut removed = 0u32;
    for name in &legacy_commands {
        let path = target_dir.join(format!("commands/{}.md", name));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            removed += 1;
        }
    }
    for name in &legacy_agents {
        let path = target_dir.join(format!("agents/{}.md", name));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            removed += 1;
        }
    }
    removed
}

// ── Canonical file generation helpers ─────────────────────────────────────────

/// Convert a Codex prompt .md into a Codex skill SKILL.md format.
/// Extracts the description from YAML frontmatter and wraps the body.
/// Generate transformed canonical skill files for a tool.
/// Returns a Vec of (relative_path, content) pairs.
fn generate_canonical_files(tool: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();

    match tool {
        "codex" => {
            // Skills: transformed canonical + memory integration
            for (name, content) in CANONICAL_SKILLS {
                let transformed = transform_skill(tool, name, content);
                files.push((format!("skills/{}/SKILL.md", name), transformed));
            }
        }
        "cursor" => {
            // Skills: individual SKILL.md in .cursor/skills/ + consolidated rules
            for (name, content) in CANONICAL_SKILLS {
                let transformed = transform_skill(tool, name, content);
                files.push((format!("skills/{}/SKILL.md", name), transformed));
            }
            // Also keep the consolidated rules file for auto-trigger quality skills
            files.push((
                "rules/harness-skills.mdc".to_string(),
                build_cursor_skills_mdc(),
            ));
        }
        "opencode" => {
            // No canonical files for opencode (only static command files)
        }
        // cline, aider: no canonical files to generate
        _ => {}
    }

    files
}

// ── Install a single tool ─────────────────────────────────────────────────────

fn install_tool(tool: &str, local: bool, dry_run: bool) -> i32 {
    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: cursor, opencode, cline, aider"
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

    // Generate canonical skill files
    let canonical = generate_canonical_files(tool);
    let total_files = cfg.files.len() + canonical.len();
    let mut progress = Progress::new(tool, total_files, dry_run);

    for (rel, content) in cfg.files {
        let dest = if cfg.root_files.contains(rel) {
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

    // Write generated canonical skill files
    for (rel, content) in &canonical {
        let dest = target_dir.join(rel);
        let status = write_or_sync(&dest, content, dry_run);
        progress.tick(rel, status);
    }

    progress.finish();

    // MCP setup guidance (skill-driven mode — no auto-injection).
    if !dry_run {
        inject_mcp(tool, target_dir);
    }

    // Clean up legacy command/agent files from previous installations.
    if !dry_run {
        let removed = cleanup_legacy_files(target_dir);
        if removed > 0 {
            eprintln!(
                "[harness] Removed {removed} legacy file(s) (commands/agents absorbed into skills in v0.4)"
            );
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
            ensure_global_config(dry_run);
            exit
        }

        Some("--list" | "list") => {
            println!("Available integrations: claude, codex, cursor, opencode, cline, aider");
            0
        }

        Some(tool) => {
            let code = install_tool(tool, local, dry_run);
            ensure_global_config(dry_run);
            code
        }
    }
}

/// Ensure `~/.harness/config.toml` and `~/.harness/HARNESS.md` exist.
/// config.toml is write-once (never overwrites user edits).
/// HARNESS.md uses write_or_sync — updated on binary upgrade, never removed on tool uninstall.
fn ensure_global_config(dry_run: bool) {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    let harness_dir = std::path::Path::new(&home).join(".harness");

    if !dry_run {
        let _ = std::fs::create_dir_all(&harness_dir);
    }

    // config.toml: write-once
    let config_path = harness_dir.join("config.toml");
    if !config_path.exists() {
        if dry_run {
            eprintln!(
                "[harness] Would create {} with default configuration",
                config_path.display()
            );
        } else {
            match std::fs::write(&config_path, crate::config::default_config_template()) {
                Ok(_) => eprintln!(
                    "[harness] Created {} with default configuration",
                    config_path.display()
                ),
                Err(e) => eprintln!(
                    "[harness] Warning: could not create {}: {}",
                    config_path.display(),
                    e
                ),
            }
        }
    }

    // HARNESS.md: write_or_sync — stays current with binary upgrades
    let harness_md_path = harness_dir.join("HARNESS.md");
    let status = write_or_sync(&harness_md_path, HARNESS_MD, dry_run);
    if matches!(status, FileStatus::Added | FileStatus::Updated) {
        eprintln!("[harness] Updated {}", harness_md_path.display());
    }
}

// ── Uninstall ─────────────────────────────────────────────────────────────────

fn uninstall_tool(tool: &str, local: bool, dry_run: bool) -> i32 {
    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: cursor, opencode, cline, aider"
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

    let mut removed = 0usize;
    let mut skipped = 0usize;

    // Collect all files: static + canonical generated
    let canonical = generate_canonical_files(tool);
    let all_files: Vec<(&str, &str)> = cfg.files.to_vec();

    for (rel, _) in &all_files {
        let dest = if cfg.root_files.contains(rel) {
            cwd.join(rel)
        } else {
            target_dir.join(rel)
        };

        // Never auto-delete root files — user may have edited them.
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

    // Also remove canonical generated files
    for (rel, _) in &canonical {
        let dest = target_dir.join(rel);

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
        let mut dirs_to_try: Vec<PathBuf> = cfg
            .files
            .iter()
            .filter_map(|(rel, _)| {
                let dest = target_dir.join(rel);
                dest.parent().map(|p| p.to_path_buf())
            })
            .collect();
        for (rel, _) in &canonical {
            let dest = target_dir.join(rel);
            if let Some(p) = dest.parent() {
                dirs_to_try.push(p.to_path_buf());
            }
        }
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
            println!("Available integrations: claude, codex, cursor, opencode, cline, aider");
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
        let dir = std::env::temp_dir().join(format!(
            "epic_test_{}_{}",
            std::process::id(),
            rand_suffix()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Cheap non-crypto suffix so parallel tests don't collide.
    fn rand_suffix() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ (COUNTER.fetch_add(1, Ordering::Relaxed) << 32)
    }

    // ── strip_frontmatter ─────────────────────────────────────────────────────

    #[test]
    fn strip_frontmatter_no_leading_newline() {
        let md = "---\nname: test\n---\n# Content\nBody";
        let result = strip_frontmatter(md);
        assert_eq!(result, "# Content\nBody");
        assert!(!result.starts_with('\n'));
    }

    #[test]
    fn strip_frontmatter_no_frontmatter() {
        let md = "# Just content\nBody";
        assert_eq!(strip_frontmatter(md), md);
    }

    #[test]
    fn strip_frontmatter_unclosed() {
        let md = "---\nname: test\n# No closing";
        assert_eq!(strip_frontmatter(md), md);
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

    // ── strip_frontmatter ──────────────────────────────────────────────────

    #[test]
    fn test_strip_frontmatter_removes_yaml() {
        let md = "---\nname: test\n---\n\n# Title\n\nBody";
        assert_eq!(strip_frontmatter(md), "\n# Title\n\nBody");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let md = "# Title\n\nBody";
        assert_eq!(strip_frontmatter(md), "# Title\n\nBody");
    }

    // ── transform_skill ──────────────────────────────────────────────────────

    #[test]
    fn test_transform_skill_codex_appends_mem_section() {
        let result = transform_skill("codex", "tdd", SKILL_TDD);
        assert!(result.contains("## Memory Integration"));
        assert!(result.contains("mem_search"));
    }

    #[test]
    fn test_transform_skill_codex_no_mem_for_debug() {
        let result = transform_skill("codex", "debug", SKILL_DEBUG);
        // Debug already has mem references in canonical; no extra section
        assert!(!result.contains("## Memory Integration\n\n**Session start"));
    }

    #[test]
    fn test_transform_skill_identity_for_cline() {
        // Cline and aider don't transform skills
        assert_eq!(transform_skill("cline", "tdd", SKILL_TDD), SKILL_TDD);
    }

    // ── build_cursor_skills_mdc ──────────────────────────────────────────────

    #[test]
    fn test_build_cursor_skills_mdc_has_frontmatter() {
        let mdc = build_cursor_skills_mdc();
        assert!(mdc.starts_with("---\n"));
        assert!(mdc.contains("alwaysApply: false"));
    }

    #[test]
    fn test_build_cursor_skills_mdc_contains_skills() {
        let mdc = build_cursor_skills_mdc();
        assert!(mdc.contains("TDD"));
        assert!(mdc.contains("Secure"));
        assert!(mdc.contains("Verify"));
        assert!(mdc.contains("Simplify"));
        assert!(mdc.contains("Perf"));
        assert!(mdc.contains("Commit"));
    }

    #[test]
    fn test_build_cursor_skills_mdc_excludes_debug_context() {
        let mdc = build_cursor_skills_mdc();
        // debug and context are operational skills, not included in cursor mdc
        assert!(!mdc.contains("# Debug"));
        assert!(!mdc.contains("# Context"));
    }

    // ── generate_canonical_files ──────────────────────────────────────────────

    #[test]
    fn test_generate_canonical_files_codex() {
        let files = generate_canonical_files("codex");
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"skills/tdd/SKILL.md"));
        // Consolidated skills (absorbed from commands)
        assert!(paths.contains(&"skills/spec/SKILL.md"));
        assert!(paths.contains(&"skills/go/SKILL.md"));
        assert!(paths.contains(&"skills/audit/SKILL.md"));
        assert!(paths.contains(&"skills/ship/SKILL.md"));
        // Remaining command-skills
        assert!(paths.contains(&"skills/orbit/SKILL.md"));
        assert!(paths.contains(&"skills/evolve/SKILL.md"));
        assert!(paths.contains(&"skills/team/SKILL.md"));
        // No agents directory
        assert!(!paths.iter().any(|p| p.starts_with("agents/")));
        // 18 canonical skills + 3 command-skills = 21
        assert_eq!(files.len(), 18 + 3);
    }

    #[test]
    fn test_generate_canonical_files_cursor() {
        let files = generate_canonical_files("cursor");
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"rules/harness-skills.mdc"));
        // Cursor: individual skills + consolidated mdc
        assert!(files.len() >= 22);
        assert!(
            paths
                .iter()
                .any(|p| p.contains("skills/") && p.contains("SKILL.md"))
        );
    }

    #[test]
    fn test_generate_canonical_files_opencode() {
        let files = generate_canonical_files("opencode");
        assert!(files.is_empty()); // no canonical files for opencode
    }

    #[test]
    fn test_generate_canonical_files_cline_empty() {
        let files = generate_canonical_files("cline");
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_canonical_files_claude_empty() {
        let files = generate_canonical_files("claude");
        assert!(files.is_empty());
    }

    #[test]
    fn test_generate_canonical_files_aider_empty() {
        let files = generate_canonical_files("aider");
        assert!(files.is_empty());
    }

    #[test]
    fn test_write_or_sync_merges_settings_json() {
        let dir = tmp_dir();
        let dest = dir.join("settings.json");
        // Existing file has a user key that must survive the merge.
        fs::write(&dest, r#"{"theme":"dark","hooksConfig":{"old":true}}"#).unwrap();
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

    #[test]
    fn test_write_or_sync_merges_claude_hooks() {
        let dir = tmp_dir();
        let dest = dir.join("settings.json");
        fs::write(
            &dest,
            r#"{"theme":"dark","hooks":{"SessionStart":[{"matcher":"*","hooks":[]}]}}"#,
        )
        .unwrap();
        let new_content = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[]}]}}"#;
        let status = write_or_sync(&dest, new_content, false);
        assert!(matches!(status, FileStatus::Updated));
        let written = fs::read_to_string(&dest).unwrap();
        let v: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(v["theme"], "dark");
        assert!(v["hooks"]["PreToolUse"].is_array());
        assert!(v["hooks"]["SessionStart"].is_null());
        let _ = fs::remove_dir_all(dir);
    }
}
