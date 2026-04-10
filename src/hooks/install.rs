use std::fs;
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

static ANTIGRAVITY_FILES: &[(&str, &str)] = integration_files!(
    "antigravity",
    [
        (
            "AGENTS.md",
            include_str!("../../integrations/antigravity/AGENTS.md")
        ),
        (
            "skills/document.md",
            include_str!("../../integrations/antigravity/skills/document.md")
        ),
        (
            "skills/perf.md",
            include_str!("../../integrations/antigravity/skills/perf.md")
        ),
        (
            "skills/secure.md",
            include_str!("../../integrations/antigravity/skills/secure.md")
        ),
        (
            "skills/simplify.md",
            include_str!("../../integrations/antigravity/skills/simplify.md")
        ),
        (
            "skills/tdd.md",
            include_str!("../../integrations/antigravity/skills/tdd.md")
        ),
        (
            "skills/verify.md",
            include_str!("../../integrations/antigravity/skills/verify.md")
        ),
        (
            "workflows/check.md",
            include_str!("../../integrations/antigravity/workflows/check.md")
        ),
        (
            "workflows/evolve.md",
            include_str!("../../integrations/antigravity/workflows/evolve.md")
        ),
        (
            "workflows/go.md",
            include_str!("../../integrations/antigravity/workflows/go.md")
        ),
        (
            "workflows/ship.md",
            include_str!("../../integrations/antigravity/workflows/ship.md")
        ),
        (
            "workflows/spec.md",
            include_str!("../../integrations/antigravity/workflows/spec.md")
        ),
        (
            "workflows/team.md",
            include_str!("../../integrations/antigravity/workflows/team.md")
        ),
        (
            "agents/auditor.md",
            include_str!("../../integrations/antigravity/agents/auditor.md")
        ),
        (
            "agents/builder.md",
            include_str!("../../integrations/antigravity/agents/builder.md")
        ),
        (
            "agents/planner.md",
            include_str!("../../integrations/antigravity/agents/planner.md")
        ),
        (
            "agents/reviewer.md",
            include_str!("../../integrations/antigravity/agents/reviewer.md")
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
    /// Alternative destination for files whose relative path starts with `alt_prefix`.
    /// Used by Codex to route `skills/` to `~/.agents/skills/` per the official spec.
    alt_dir: Option<PathBuf>,
    alt_prefix: &'static str,
    /// Files that should never be overwritten if they already exist (e.g. config.toml).
    /// Unlike root_files these live inside the tool dir, not in cwd.
    preserve_files: &'static [&'static str],
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
        }),
        "gemini" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".gemini"),
            local_dir: cwd.join(".gemini"),
            root_files: &["GEMINI.md"],
            files: GEMINI_FILES,
            note: Some("If GEMINI.md already exists, append the section manually."),
            // Gemini CLI also discovers agents/ and skills/ from standard .agents/ location.
            // Using alt_dir ensures shared resources are not duplicated or conflicting.
            alt_dir: Some(PathBuf::from(&home).join(".agents")),
            alt_prefix: "skills/",
            preserve_files: &[],
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
        }),
        "antigravity" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".agents"),
            local_dir: cwd.join(".agents"),
            root_files: &["AGENTS.md"],
            files: ANTIGRAVITY_FILES,
            note: Some("Ring 0 hooks not available — using AGENTS.md + skills/workflows instead."),
            alt_dir: None,
            alt_prefix: "",
            preserve_files: &[],
        }),
        _ => None,
    }
}

// ── Install logic ─────────────────────────────────────────────────────────────

/// Root-only files (GEMINI.md, AGENTS.md): never overwrite — user may have edited or merged.
fn write_if_missing(dest: &Path, content: &str, dry_run: bool) {
    if dest.exists() {
        println!("[harness] ~ {} (exists, skipping)", dest.display());
        return;
    }
    if dry_run {
        println!("[dry-run] write {}", dest.display());
        return;
    }
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(dest, content) {
        Ok(_) => println!("[harness] + {}", dest.display()),
        Err(e) => eprintln!("[harness] ERROR writing {}: {e}", dest.display()),
    }
}

/// Hooks, commands, agents, rules, skills, etc.: write if missing or content differs from embedded.
fn write_or_sync(dest: &Path, content: &str, dry_run: bool) {
    let existed = dest.exists();
    let is_settings_json = dest.file_name().map_or(false, |n| n == "settings.json");

    if is_settings_json && existed {
        // For settings.json, merge instead of overwriting to preserve theme, auth, etc.
        let existing_content = match fs::read_to_string(dest) {
            Ok(s) => s,
            Err(_) => "".to_string(),
        };

        let mut existing_json: serde_json::Value = match serde_json::from_str(&existing_content) {
            Ok(j) => j,
            Err(_) => serde_json::json!({}),
        };

        let new_json: serde_json::Value = match serde_json::from_str(content) {
            Ok(j) => j,
            Err(_) => serde_json::json!({}),
        };

        // Merge hooksConfig and hooks into the existing JSON
        if let Some(new_hooks_config) = new_json.get("hooksConfig") {
            existing_json["hooksConfig"] = new_hooks_config.clone();
        }
        if let Some(new_hooks) = new_json.get("hooks") {
            existing_json["hooks"] = new_hooks.clone();
        }

        let merged_content = serde_json::to_string_pretty(&existing_json).unwrap_or_else(|_| content.to_string());
        
        if existing_content == merged_content {
            println!("[harness] = {} (merged, unchanged)", dest.display());
            return;
        }

        if dry_run {
            println!("[dry-run] merge {}", dest.display());
            return;
        }

        match fs::write(dest, merged_content) {
            Ok(_) => println!("[harness] # {} (merged/updated)", dest.display()),
            Err(e) => eprintln!("[harness] ERROR merging {}: {e}", dest.display()),
        }
        return;
    }

    let unchanged = existed
        && fs::read_to_string(dest)
            .map(|existing| existing == content)
            .unwrap_or(false);
    if unchanged {
        println!("[harness] = {} (unchanged)", dest.display());
        return;
    }
    if dry_run {
        println!("[dry-run] write {}", dest.display());
        return;
    }
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(dest, content) {
        Ok(_) => {
            if existed {
                println!("[harness] # {} (updated)", dest.display());
            } else {
                println!("[harness] + {}", dest.display());
            }
        }
        Err(e) => eprintln!("[harness] ERROR writing {}: {e}", dest.display()),
    }
}

pub fn run(args: &[String]) -> i32 {
    // Parse: epic-harness install <tool> [--local] [--dry-run]
    let tool = match args.first() {
        Some(t) => t.as_str(),
        None => {
            eprintln!(
                "Usage: epic-harness install <codex|gemini|cursor|antigravity> [--local] [--dry-run]"
            );
            eprintln!(
                "       Embedded integration files are synced (missing or outdated files are written)."
            );
            eprintln!("       Root files (GEMINI.md, AGENTS.md) are only created if absent.");
            eprintln!("       epic-harness install --list");
            return 1;
        }
    };

    if tool == "--list" || tool == "list" {
        println!("Available integrations: codex, gemini, cursor, antigravity");
        return 0;
    }

    let local = args.iter().any(|a| a == "--local");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let cfg = match tool_config(tool) {
        Some(c) => c,
        None => {
            eprintln!(
                "[harness] Unknown tool '{tool}'. Use one of: codex, gemini, cursor, antigravity"
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

    println!("[harness] Installing {tool} → {}", target_dir.display());
    if let Some(note) = cfg.note {
        println!("[harness] Note: {note}");
    }

    // For --local installs, alt_dir also becomes local (sibling of target_dir's parent).
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

    for (rel, content) in cfg.files {
        // Root files (GEMINI.md, AGENTS.md) go into cwd, not the tool dir.
        let dest = if cfg.root_files.contains(rel) {
            cwd.join(rel)
        } else {
            target_dir.join(rel)
        };

        if cfg.root_files.contains(rel) || cfg.preserve_files.contains(rel) {
            write_if_missing(&dest, content, dry_run);
        } else {
            write_or_sync(&dest, content, dry_run);
        }

        // Gemini-specific: also sync agents/ and skills/ to ~/.agents/ for Antigravity compatibility.
        if tool == "gemini" && (rel.starts_with("agents/") || rel.starts_with("skills/")) {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let alt_dest = if local {
                cwd.join(".agents").join(rel)
            } else {
                Path::new(&home).join(".agents").join(rel)
            };
            if alt_dest != dest {
                write_or_sync(&alt_dest, content, dry_run);
            }
        }
    }

    // Codex-specific: warn if config.toml exists but codex_hooks is not enabled.
    if tool == "codex" {
        let config_path = target_dir.join("config.toml");
        if config_path.exists() {
            let ok = fs::read_to_string(&config_path)
                .map(|s| s.contains("codex_hooks"))
                .unwrap_or(false);
            if !ok {
                println!();
                println!("[harness] WARNING: ~/.codex/config.toml exists but does not enable hooks.");
                println!("[harness] Hooks are OFF by default. Add these lines to enable them:");
                println!();
                println!("    [features]");
                println!("    codex_hooks = true");
                println!();
                println!("[harness] Then restart Codex for the change to take effect.");
            }
        }
    }

    println!("[harness] Done.");
    0
}
