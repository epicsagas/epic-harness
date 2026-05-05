use std::fs;
use std::io::{self, IsTerminal, Write as IoWrite};
use std::path::{Path, PathBuf};

// ── Canonical sources (single source of truth) ──────────────────────────────

static SKILL_COMMIT: &str = include_str!("../../skills/commit/SKILL.md");
static SKILL_CONTEXT: &str = include_str!("../../skills/context/SKILL.md");
static SKILL_DEBUG: &str = include_str!("../../skills/debug/SKILL.md");
static SKILL_DOCUMENT: &str = include_str!("../../skills/document/SKILL.md");
static SKILL_PERF: &str = include_str!("../../skills/perf/SKILL.md");
static SKILL_SECURE: &str = include_str!("../../skills/secure/SKILL.md");
static SKILL_SIMPLIFY: &str = include_str!("../../skills/simplify/SKILL.md");
static SKILL_TDD: &str = include_str!("../../skills/tdd/SKILL.md");
static SKILL_VERIFY: &str = include_str!("../../skills/verify/SKILL.md");
static SKILL_COUNCIL: &str = include_str!("../../skills/council/SKILL.md");
static SKILL_AGENT_INTROSPECTION: &str = include_str!("../../skills/agent-introspection/SKILL.md");
// _dispatch is Claude Code only, not installed to other tools

// ── Canonical commands (Claude Code plugin cache sync) ───────────────────────
static CMD_CHECK: &str  = include_str!("../../commands/check.md");
static CMD_EVOLVE: &str = include_str!("../../commands/evolve.md");
static CMD_GO: &str     = include_str!("../../commands/go.md");
static CMD_SHIP: &str   = include_str!("../../commands/ship.md");
static CMD_SPEC: &str   = include_str!("../../commands/spec.md");
static CMD_TEAM: &str   = include_str!("../../commands/team.md");

static SKILL_DISPATCH: &str = include_str!("../../skills/_dispatch/SKILL.md");

static AGENT_AUDITOR: &str = include_str!("../../agents/auditor.md");
static AGENT_BUILDER: &str = include_str!("../../agents/builder.md");
static AGENT_PLANNER: &str = include_str!("../../agents/planner.md");
static AGENT_REVIEWER: &str = include_str!("../../agents/reviewer.md");

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
];

static CANONICAL_AGENTS: &[(&str, &str)] = &[
    ("auditor", AGENT_AUDITOR),
    ("builder", AGENT_BUILDER),
    ("planner", AGENT_PLANNER),
    ("reviewer", AGENT_REVIEWER),
];

// ── Per-skill Memory Integration sections (appended for codex/gemini) ───────

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

// ── Per-tool agent addendums ─────────────────────────────────────────────────

static CODEX_AGENT_ADDENDUM_BUILDER: &str = "\n\n## Invoking as a Codex Sub-agent\n\nTo launch this agent for a task, pass the task description and context as the sub-agent prompt. Independent builder tasks can be launched in parallel using Codex's parallel task execution.\n";
static CODEX_AGENT_ADDENDUM_AUDITOR: &str = "\n\n## Invoking as a Codex Sub-agent\n\nLaunch this agent as a parallel Codex task alongside the Reviewer and Test runner during `/check`. Pass the list of changed files and the git diff as context.\n";
static CODEX_AGENT_ADDENDUM_PLANNER: &str = "\n\n## Invoking as a Codex Sub-agent\n\nInvoke this agent at the start of `/go` to produce the task breakdown. The output plan drives which builder sub-agents to launch and in what order.\n";
static CODEX_AGENT_ADDENDUM_REVIEWER: &str = "\n\n## Invoking as a Codex Sub-agent\n\nLaunch this agent as a parallel Codex task alongside the Auditor and Test runner during `/check`. Pass the list of changed files and the git diff as context.\n";

static GEMINI_AGENT_NOTE_BUILDER: &str = "\n> **Gemini CLI note**: Agents run sequentially, not in parallel. Complete this task fully before\n> the next agent or task begins.\n";
static GEMINI_AGENT_NOTE_AUDITOR: &str = "\n> **Gemini CLI note**: Agents run sequentially. This auditor runs after the reviewer completes.\n";
static GEMINI_AGENT_NOTE_REVIEWER: &str = "\n> **Gemini CLI note**: Agents run sequentially. This reviewer runs after the build task completes,\n> before the auditor.\n";

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

/// Transform a canonical agent for a specific tool.
pub(crate) fn transform_agent(tool: &str, name: &str, canonical: &str) -> String {
    match tool {
        "codex" => {
            let addendum = match name {
                "builder" => CODEX_AGENT_ADDENDUM_BUILDER,
                "auditor" => CODEX_AGENT_ADDENDUM_AUDITOR,
                "planner" => CODEX_AGENT_ADDENDUM_PLANNER,
                "reviewer" => CODEX_AGENT_ADDENDUM_REVIEWER,
                _ => "",
            };
            format!("{}{}", canonical.trim_end(), addendum)
        }
        "gemini" => {
            // Remap tools in frontmatter
            let mut result = canonical
                .replace(
                    "tools: [Read, Edit, Write, Bash, Grep, Glob]",
                    "tools: [read_file, replace, write_file, run_shell_command, grep_search, glob]",
                )
                .replace(
                    "tools: [Read, Grep, Glob, Bash]",
                    "tools: [read_file, grep_search, glob, run_shell_command]",
                )
                .replace(
                    "tools: [Read, Grep, Glob]",
                    "tools: [read_file, grep_search, glob]",
                );

            // Insert Gemini note after the first heading line
            let note = match name {
                "builder" => GEMINI_AGENT_NOTE_BUILDER,
                "auditor" => GEMINI_AGENT_NOTE_AUDITOR,
                "reviewer" => GEMINI_AGENT_NOTE_REVIEWER,
                _ => "",
            };
            if !note.is_empty() {
                // Insert after first "# " heading line
                if let Some(pos) = result.find("\n\n## ") {
                    result.insert_str(pos + 1, note);
                }
            }

            // Planner: rewrite parallelization references for sequential execution
            if name == "planner" {
                result = result.replace(
                    "description: \"Breaks down a goal into ordered, parallelizable tasks with dependencies.\"",
                    "description: \"Breaks down a goal into ordered, sequential tasks with dependencies.\"",
                );
                result = result.replace(
                    "5. **Parallelize**: Mark independent tasks that can run concurrently",
                    "5. **Sequence**: Order all tasks for sequential execution, grouping independent ones together",
                );
                result = result.replace(
                    "   - Parallel: yes\n",
                    "   - Could parallelize: yes (but will run sequentially)\n",
                );
                result = result.replace(
                    "   - Parallel: no\n",
                    "   - Could parallelize: no\n",
                );
                result = result.replace(
                    "   - Parallel: yes (with Task 1)\n",
                    "   - Could parallelize: yes (but will run sequentially, after Task 2)\n",
                );
                result = result.replace(
                    "### Execution Order\n- Batch 1 (parallel): Task 1, Task 3\n- Batch 2 (sequential): Task 2",
                    "### Execution Order (sequential)\n1. Task 1 \u{2192} Task 3 \u{2192} Task 2",
                );
                // Add Gemini note for planner (has unique placement)
                if let Some(pos) = result.find("\n\n## Process") {
                    result.insert_str(
                        pos + 1,
                        "\n> **Gemini CLI note**: Gemini CLI runs agents sequentially, not in parallel. Design plans with\n> clear sequential ordering. Mark which tasks could theoretically run in parallel as context for\n> the executor, but assume they will run one at a time.\n",
                    );
                }
            }

            result
        }
        "cursor" => {
            // Add model: inherit before the closing --- of frontmatter
            if let Some(start) = canonical.strip_prefix("---\n")
                && let Some(end_pos) = start.find("\n---\n")
            {
                let frontmatter = &start[..end_pos];
                let body = &start[end_pos + 4..]; // skip \n---\n
                return format!("---\n{}\nmodel: inherit\n---\n{}", frontmatter, body);
            }
            canonical.to_string()
        }
        "opencode" => {
            // Replace tools array with dict format
            let mut result = canonical
                .replace(
                    "tools: [Read, Edit, Write, Bash, Grep, Glob]",
                    "tools:\n  read: true\n  edit: true\n  write: true\n  bash: true",
                )
                .replace(
                    "tools: [Read, Grep, Glob, Bash]",
                    "tools:\n  write: false\n  edit: false",
                )
                .replace(
                    "tools: [Read, Grep, Glob]",
                    "tools:\n  write: false\n  edit: false\n  bash: false",
                );

            // Add Codex sub-agent addendum (opencode uses same text)
            let addendum = match name {
                "builder" => CODEX_AGENT_ADDENDUM_BUILDER,
                "auditor" => CODEX_AGENT_ADDENDUM_AUDITOR,
                "planner" => CODEX_AGENT_ADDENDUM_PLANNER,
                "reviewer" => CODEX_AGENT_ADDENDUM_REVIEWER,
                _ => "",
            };
            if !addendum.is_empty() {
                result = format!("{}{}", result.trim_end(), addendum);
            }
            result
        }
        _ => canonical.to_string(),
    }
}

/// Transform a canonical skill for a specific tool.
fn transform_skill(tool: &str, name: &str, canonical: &str) -> String {
    match tool {
        "codex" | "gemini" => {
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

            // Minor text normalizations that codex/gemini versions applied
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

            // Gemini: minor text tweak for tdd
            if tool == "gemini" && name == "tdd" {
                result = result.replace(
                    "- `/go` sub-agents: always",
                    "- `/go` tasks: always",
                );
            }
            if tool == "gemini" && name == "context" {
                result = result.replace(
                    "the `resume` hook will reload:",
                    "the `resume` hook (BeforeAgent) will reload:",
                );
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
            "commands/check.md",
            include_str!("../../integrations/gemini/commands/check.md")
        ),
        (
            "commands/evolve.md",
            include_str!("../../integrations/gemini/commands/evolve.md")
        ),
        (
            "commands/go.md",
            include_str!("../../integrations/gemini/commands/go.md")
        ),
        (
            "commands/ship.md",
            include_str!("../../integrations/gemini/commands/ship.md")
        ),
        (
            "commands/spec.md",
            include_str!("../../integrations/gemini/commands/spec.md")
        ),
        (
            "commands/team.md",
            include_str!("../../integrations/gemini/commands/team.md")
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

static CLAUDE_FILES: &[(&str, &str)] = integration_files!(
    "claude",
    [(
        ".claude/settings.json",
        include_str!("../../hooks/hooks.json")
    ),]
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
        // Claude Code: install hooks into ~/.claude/settings.json + MCP injection via inject_mcp_claude().
        "claude" => Some(ToolConfig {
            global_dir: PathBuf::from(&home),
            local_dir: cwd.clone(),
            root_files: &[],
            files: CLAUDE_FILES,
            note: Some("Installs hooks in ~/.claude/settings.json and registers harness-mem MCP server in ~/.claude.json."),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
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

/// Claude global settings hooks do not run in a plugin context, so
/// `${CLAUDE_PLUGIN_ROOT}` is unavailable there.
fn sanitize_claude_global_hooks(content: &str) -> String {
    let mut json: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return content.to_string(),
    };

    let Some(hooks) = json.get_mut("hooks").and_then(|v| v.as_object_mut()) else {
        return content.to_string();
    };

    let prefix = "EH=\"${CLAUDE_PLUGIN_ROOT}/hooks/bin/epic-harness\"; test -x \"$EH\" || ";
    for entries in hooks.values_mut() {
        let Some(entries_arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in entries_arr {
            let Some(cmd_hooks) = entry.get_mut("hooks").and_then(|v| v.as_array_mut()) else {
                continue;
            };
            for cmd_hook in cmd_hooks {
                let Some(cmd) = cmd_hook.get_mut("command").and_then(|v| v.as_str()) else {
                    continue;
                };
                let next = if cmd.contains("${CLAUDE_PLUGIN_ROOT}/hooks/setup.sh") {
                    // Keep SessionStart valid without plugin context.
                    ":"
                } else {
                    cmd.strip_prefix(prefix).unwrap_or(cmd)
                };
                cmd_hook["command"] = serde_json::Value::String(next.to_string());
            }
        }
    }

    serde_json::to_string_pretty(&json).unwrap_or_else(|_| content.to_string())
}

// ── MCP injection ─────────────────────────────────────────────────────────────

/// Syncs canonical commands/skills/agents into every discovered Claude Code
/// plugin cache directory (`~/.claude/plugins/cache/epicsagas/epic/*/`).
///
/// Called on `epic install claude` so the locally-installed binary always
/// keeps the cache in sync without waiting for an npm publish.
fn sync_plugin_cache(home: &str, dry_run: bool) {
    let cache_base = std::path::Path::new(home)
        .join(".claude/plugins/cache/epicsagas/epic");

    let entries = match fs::read_dir(&cache_base) {
        Ok(e) => e,
        Err(_) => {
            return; // Claude Code not installed — skip silently
        }
    };

    // symlink 탈출 방어: cache_base를 한 번 canonicalize (루프 밖)
    let cache_base_canon = match cache_base.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return; // Claude Code not installed — skip silently
        }
    };

    let files: &[(&str, &str)] = &[
        ("commands/check.md",          CMD_CHECK),
        ("commands/evolve.md",         CMD_EVOLVE),
        ("commands/go.md",             CMD_GO),
        ("commands/ship.md",           CMD_SHIP),
        ("commands/spec.md",           CMD_SPEC),
        ("commands/team.md",           CMD_TEAM),
        ("skills/_dispatch/SKILL.md",  SKILL_DISPATCH),
        ("skills/commit/SKILL.md",     SKILL_COMMIT),
        ("skills/context/SKILL.md",    SKILL_CONTEXT),
        ("skills/debug/SKILL.md",      SKILL_DEBUG),
        ("skills/document/SKILL.md",   SKILL_DOCUMENT),
        ("skills/perf/SKILL.md",       SKILL_PERF),
        ("skills/secure/SKILL.md",     SKILL_SECURE),
        ("skills/simplify/SKILL.md",   SKILL_SIMPLIFY),
        ("skills/tdd/SKILL.md",        SKILL_TDD),
        ("skills/verify/SKILL.md",     SKILL_VERIFY),
        ("skills/council/SKILL.md",    SKILL_COUNCIL),
        ("skills/agent-introspection/SKILL.md", SKILL_AGENT_INTROSPECTION),
        ("agents/auditor.md",          AGENT_AUDITOR),
        ("agents/builder.md",          AGENT_BUILDER),
        ("agents/planner.md",          AGENT_PLANNER),
        ("agents/reviewer.md",         AGENT_REVIEWER),
    ];

    let mut synced = 0u32;
    for entry in entries.flatten() {
        let version_dir = entry.path();
        if !version_dir.is_dir() {
            continue;
        }
        // symlink 탈출 방어: version_dir를 canonicalize 후 cache_base 내부인지 검증
        let version_dir = match version_dir.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !version_dir.starts_with(&cache_base_canon) {
            eprintln!(
                "[harness] skipping suspicious cache entry: {}",
                version_dir.display()
            );
            continue;
        }
        for (rel, content) in files {
            let dest = version_dir.join(rel);
            let status = write_or_sync(&dest, content, dry_run);
            if matches!(status, FileStatus::Updated | FileStatus::Added) {
                synced += 1;
            }
        }
    }

    if dry_run {
        eprintln!("[harness] dry-run: would sync plugin cache files");
    } else {
        eprintln!("[harness] plugin cache synced ({synced} files updated)");
    }
}

/// Injects `mcpServers.harness-mem` into `~/.claude.json`.
/// Claude Code uses this file (not ~/.claude/settings.json) for global app state including MCP.
fn inject_mcp_claude() {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("[harness] Could not determine home directory — skipping Claude MCP injection.");
        return;
    };
    let claude_json = std::path::Path::new(&home).join(".claude.json");

    let raw = if claude_json.exists() {
        fs::read_to_string(&claude_json).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    if json["mcpServers"]["harness-mem"].is_object() {
        eprintln!("[harness] mcpServers.harness-mem already registered in ~/.claude.json — skipping.");
        return;
    }

    // On Linux, current_exe() may return a path ending in " (deleted)" when the
    // binary has been replaced since startup.  Verify the path exists before
    // embedding it in ~/.claude.json; fall back to the binary name on the PATH.
    let binary = std::env::current_exe()
        .ok()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "epic".to_string());

    json["mcpServers"]["harness-mem"] = serde_json::json!({
        "command": binary,
        "args": ["mem", "mcp"]
    });

    let out = match serde_json::to_string_pretty(&json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[harness] Failed to serialize ~/.claude.json: {e}");
            return;
        }
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = claude_json
        .with_file_name(format!(".claude.{}.{}.json.tmp", std::process::id(), nonce));
    if fs::write(&tmp, &out).is_ok() && fs::rename(&tmp, &claude_json).is_ok() {
        eprintln!("[harness] Registered mcpServers.harness-mem in ~/.claude.json");
    } else {
        let _ = fs::remove_file(&tmp); // clean up tmp on failure
        eprintln!("[harness] Failed to write ~/.claude.json");
    }
}

/// Removes `mcpServers.harness-mem` from `~/.claude.json`.
/// Mirror of `inject_mcp_claude()` — called by `uninstall_tool` for the "claude" tool.
fn remove_mcp_claude(dry_run: bool) {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("[harness] Could not determine home directory — skipping Claude MCP removal.");
        return;
    };
    let claude_json = std::path::Path::new(&home).join(".claude.json");

    if !claude_json.exists() {
        eprintln!("[harness] ~/.claude.json not found — nothing to remove.");
        return;
    }

    let raw = match fs::read_to_string(&claude_json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[harness] Failed to read ~/.claude.json: {e}");
            return;
        }
    };

    let mut json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[harness] Failed to parse ~/.claude.json: {e}");
            return;
        }
    };

    if json["mcpServers"].get("harness-mem").is_none() {
        eprintln!("[harness] mcpServers.harness-mem not found in ~/.claude.json — nothing to remove.");
        return;
    }

    if dry_run {
        eprintln!("[harness] (dry-run) would remove mcpServers.harness-mem from ~/.claude.json");
        return;
    }

    if let Some(servers) = json["mcpServers"].as_object_mut() {
        servers.remove("harness-mem");
    }

    let out = match serde_json::to_string_pretty(&json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[harness] Failed to serialize ~/.claude.json: {e}");
            return;
        }
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = claude_json
        .with_file_name(format!(".claude.{}.{}.json.tmp", std::process::id(), nonce));
    if fs::write(&tmp, &out).is_ok() && fs::rename(&tmp, &claude_json).is_ok() {
        eprintln!("[harness] Removed mcpServers.harness-mem from ~/.claude.json");
    } else {
        let _ = fs::remove_file(&tmp); // clean up tmp on failure
        eprintln!("[harness] Failed to write ~/.claude.json");
    }
}

/// Injects `mcpServers.harness-mem` into the tool's settings JSON file.
/// Registers `epic-harness mem mcp` as the MCP server command (no Node.js required).
/// Silently skips if the settings file doesn't exist or already has the entry.
fn inject_mcp(tool: &str, target_dir: &Path) {
    // Claude Code stores MCP config in ~/.claude.json (global app state), not settings.json
    if tool == "claude" {
        inject_mcp_claude();
        return;
    }

    let settings_path = match tool {
        "codex"    => None, // Codex uses hooks.json, no mcpServers concept
        "gemini"   => Some(target_dir.join("settings.json")),
        "cursor"   => Some(target_dir.join("mcp.json")),
        "opencode" => Some(target_dir.join("opencode.json")),
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

    // Use the current running binary path for reliability; fall back to bare name.
    // Guard against Linux " (deleted)" paths when the binary was replaced at runtime.
    let binary = std::env::current_exe()
        .ok()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "epic".to_string());

    // opencode uses { "mcp": { "name": { type, command[] } } }
    // Others use { "mcpServers": { "name": { command, args[] } } }
    if tool == "opencode" {
        if json["mcp"]["harness-mem"].is_object() {
            eprintln!("[harness] mcp.harness-mem already registered in {tool} settings — skipping.");
            return;
        }
        json["mcp"]["harness-mem"] = serde_json::json!({
            "type": "local",
            "command": [binary, "mem", "mcp"]
        });
    } else {
        if json["mcpServers"]["harness-mem"].is_object() {
            eprintln!("[harness] mcpServers.harness-mem already registered in {tool} settings — skipping.");
            return;
        }
        json["mcpServers"]["harness-mem"] = serde_json::json!({
            "command": binary,
            "args": ["mem", "mcp"]
        });
    }

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
    } else {
        let _ = fs::remove_file(&tmp); // clean up tmp on failure
    }
}

// ── Interactive menu ──────────────────────────────────────────────────────────

const TOOLS: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
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
    let level = super::telemetry::prompt_consent_interactive();
    super::telemetry::write_consent(level);
    match level {
        super::telemetry::ConsentLevel::On => {
            eprintln!("[harness] Telemetry enabled. To opt out: epic-harness telemetry off");
        }
        super::telemetry::ConsentLevel::Off => {
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
            && n >= 1 && n <= TOOLS.len()
        {
            selected.push(TOOLS[n - 1].0.to_string());
        }
    }
    selected
}

// ── Canonical file generation ─────────────────────────────────────────────────

/// Generate transformed canonical skill and agent files for a tool.
/// Returns a Vec of (relative_path, content) pairs.
fn generate_canonical_files(tool: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();

    match tool {
        "codex" | "gemini" => {
            // Skills: transformed canonical + memory integration
            for (name, content) in CANONICAL_SKILLS {
                let transformed = transform_skill(tool, name, content);
                files.push((format!("skills/{}/SKILL.md", name), transformed));
            }
            // Agents: transformed canonical
            for (name, content) in CANONICAL_AGENTS {
                let transformed = transform_agent(tool, name, content);
                files.push((format!("agents/{}.md", name), transformed));
            }
        }
        "cursor" => {
            // Skills: concatenated into harness-skills.mdc
            files.push(("rules/harness-skills.mdc".to_string(), build_cursor_skills_mdc()));
            // Agents: transformed canonical
            for (name, content) in CANONICAL_AGENTS {
                let transformed = transform_agent(tool, name, content);
                files.push((format!("agents/{}.md", name), transformed));
            }
        }
        "opencode" => {
            // Agents only (no skills for opencode)
            for (name, content) in CANONICAL_AGENTS {
                let transformed = transform_agent(tool, name, content);
                files.push((format!("agents/{}.md", name), transformed));
            }
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
                "[harness] Unknown tool '{tool}'. Use one of: claude, codex, gemini, cursor, opencode, cline, aider"
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

    // Generate canonical files (transformed skills + agents)
    let canonical = generate_canonical_files(tool);
    let total_files = cfg.files.len() + canonical.len();
    let mut progress = Progress::new(tool, total_files, dry_run);

    for (rel, content) in cfg.files {
        let effective_content;
        let content = if tool == "claude" && *rel == ".claude/settings.json" {
            effective_content = sanitize_claude_global_hooks(content);
            effective_content.as_str()
        } else {
            content
        };

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

    // Write generated canonical files (skills + agents)
    for (rel, content) in &canonical {
        let dest = if !cfg.alt_prefix.is_empty() && rel.starts_with(cfg.alt_prefix) {
            if let Some(alt) = &alt_target {
                alt.join(rel)
            } else {
                target_dir.join(rel)
            }
        } else {
            target_dir.join(rel)
        };

        let status = write_or_sync(&dest, content, dry_run);
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

    // Sync plugin cache for Claude Code (keeps commands/skills/agents up-to-date
    // without requiring an npm publish for every change).
    if tool == "claude" {
        let home = std::env::var("HOME").unwrap_or_default();
        sync_plugin_cache(&home, dry_run);
    }

    // Seed the default epic org/team on first install (idempotent).
    // Restricted to `epic install claude` — other tools should not implicitly
    // create org state in ~/.harness/orgs/.
    if !dry_run && tool == "claude" {
        crate::hooks::team::store::install_default_team_if_needed("epic");
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
            println!("Available integrations: claude, codex, gemini, cursor, opencode, cline, aider");
            0
        }

        Some(tool) => {
            let code = install_tool(tool, local, dry_run);
            ensure_global_config(dry_run);
            code
        }
    }
}

/// Ensure `~/.harness/config.toml` exists. Creates it with the commented default
/// template if absent. Never overwrites an existing config.
fn ensure_global_config(dry_run: bool) {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    let harness_dir = std::path::Path::new(&home).join(".harness");
    let config_path = harness_dir.join("config.toml");

    if config_path.exists() {
        return;
    }
    if dry_run {
        eprintln!(
            "[harness] Would create {} with default configuration",
            config_path.display()
        );
        return;
    }
    let _ = std::fs::create_dir_all(&harness_dir);
    match std::fs::write(&config_path, super::config::default_config_template()) {
        Ok(_) => eprintln!("[harness] Created {} with default configuration", config_path.display()),
        Err(e) => eprintln!("[harness] Warning: could not create {}: {}", config_path.display(), e),
    }
}

// ── Uninstall ─────────────────────────────────────────────────────────────────

fn uninstall_tool(tool: &str, local: bool, dry_run: bool) -> i32 {
    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: claude, codex, gemini, cursor, opencode, cline, aider"
            );
            return 1;
        }
    };

    // Claude Code: no files to remove — only MCP entry in ~/.claude.json.
    if tool == "claude" {
        remove_mcp_claude(dry_run);
        return 0;
    }

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

    // Collect all files: static + canonical generated
    let canonical = generate_canonical_files(tool);
    let all_files: Vec<(&str, &str)> = cfg.files.to_vec();

    for (rel, _) in &all_files {
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

    // Also remove canonical generated files
    for (rel, _) in &canonical {
        let dest = if !cfg.alt_prefix.is_empty() && rel.starts_with(cfg.alt_prefix) {
            if let Some(alt) = &alt_target {
                alt.join(rel)
            } else {
                target_dir.join(rel)
            }
        } else {
            target_dir.join(rel)
        };

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
            println!("Available integrations: claude, codex, gemini, cursor, opencode, cline, aider");
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

    // ── transform_agent ──────────────────────────────────────────────────────

    #[test]
    fn test_transform_agent_codex_adds_addendum() {
        let result = transform_agent("codex", "builder", AGENT_BUILDER);
        assert!(result.contains("Invoking as a Codex Sub-agent"));
        assert!(result.contains("parallel task execution"));
    }

    #[test]
    fn test_transform_agent_gemini_remaps_tools() {
        let result = transform_agent("gemini", "builder", AGENT_BUILDER);
        assert!(result.contains("tools: [read_file, replace, write_file, run_shell_command, grep_search, glob]"));
        assert!(!result.contains("tools: [Read, Edit, Write, Bash, Grep, Glob]"));
    }

    #[test]
    fn test_transform_agent_gemini_adds_note() {
        let result = transform_agent("gemini", "builder", AGENT_BUILDER);
        assert!(result.contains("Gemini CLI note"));
    }

    #[test]
    fn test_transform_agent_cursor_adds_model_inherit() {
        let result = transform_agent("cursor", "builder", AGENT_BUILDER);
        assert!(result.contains("model: inherit"));
        // Verify frontmatter is valid
        assert!(result.starts_with("---\n"));
        assert!(result.contains("\nmodel: inherit\n---\n"));
    }

    #[test]
    fn test_transform_agent_opencode_yaml_tools() {
        let result = transform_agent("opencode", "builder", AGENT_BUILDER);
        assert!(result.contains("tools:\n  read: true\n  edit: true"));
        assert!(!result.contains("tools: [Read"));
    }

    #[test]
    fn test_transform_agent_opencode_readonly_tools() {
        let result = transform_agent("opencode", "auditor", AGENT_AUDITOR);
        assert!(result.contains("write: false"));
        assert!(result.contains("edit: false"));
    }

    #[test]
    fn test_transform_agent_gemini_planner_sequential() {
        let result = transform_agent("gemini", "planner", AGENT_PLANNER);
        assert!(result.contains("sequential"));
        assert!(result.contains("Could parallelize"));
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
        assert!(paths.contains(&"agents/builder.md"));
        assert_eq!(files.len(), 11 + 4); // 11 skills + 4 agents
    }

    #[test]
    fn test_generate_canonical_files_cursor() {
        let files = generate_canonical_files("cursor");
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"rules/harness-skills.mdc"));
        assert!(paths.contains(&"agents/builder.md"));
        // Cursor: 1 mdc + 4 agents
        assert_eq!(files.len(), 1 + 4);
    }

    #[test]
    fn test_generate_canonical_files_opencode() {
        let files = generate_canonical_files("opencode");
        assert_eq!(files.len(), 4); // agents only
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

    #[test]
    fn test_sanitize_claude_global_hooks_removes_plugin_root_refs() {
        let out = sanitize_claude_global_hooks(CLAUDE_FILES[0].1);
        assert!(!out.contains("${CLAUDE_PLUGIN_ROOT}"));
        let json: serde_json::Value = serde_json::from_str(&out).unwrap();
        let hooks = json["hooks"].as_object().unwrap();
        let mut found_path_fallback = false;
        for entries in hooks.values() {
            if let Some(entries_arr) = entries.as_array() {
                for entry in entries_arr {
                    if let Some(cmd_hooks) = entry["hooks"].as_array() {
                        for cmd_hook in cmd_hooks {
                            if cmd_hook["command"].as_str()
                                .is_some_and(|cmd| cmd.contains("command -v epic-harness"))
                            {
                                found_path_fallback = true;
                            }
                        }
                    }
                }
            }
        }
        assert!(found_path_fallback);
    }

    #[test]
    fn test_sanitize_claude_global_hooks_replaces_setup_hook_with_noop() {
        let out = sanitize_claude_global_hooks(CLAUDE_FILES[0].1);
        assert!(out.contains("\"command\": \":\""));
    }

    // ── sync_plugin_cache ─────────────────────────────────────────────────────

    #[test]
    fn test_sync_plugin_cache_no_panic_on_missing_dir() {
        // HOME을 존재하지 않는 임시 경로로 설정 → cache_base 없음 → silent return
        let fake_home = std::env::temp_dir()
            .join(format!("epic_test_no_cache_{}", rand_suffix()));
        // 디렉토리를 만들지 않아야 함 — cache_base read_dir 실패해야 함
        let home_str = fake_home.to_string_lossy().to_string();
        // dry_run=true 로 호출 — 패닉 없이 즉시 반환되어야 함
        sync_plugin_cache(&home_str, true);
        // 여기까지 도달하면 성공
    }

    #[test]
    fn test_sync_plugin_cache_no_panic_on_empty_cache_dir() {
        // cache_base 디렉토리는 존재하지만 version 서브디렉토리가 없는 경우
        let base_dir = tmp_dir();
        let cache_base = base_dir
            .join(".claude/plugins/cache/epicsagas/epic");
        fs::create_dir_all(&cache_base).unwrap();
        let home_str = base_dir.to_string_lossy().to_string();
        // 패닉 없이 종료되어야 함
        sync_plugin_cache(&home_str, true);
        let _ = fs::remove_dir_all(base_dir);
    }
}
