use std::fs;
use std::path::{Path, PathBuf};

// ── Embedded integration files ────────────────────────────────────────────────

macro_rules! integration_files {
    ($tool:literal, [ $( ($rel:literal, $content:expr) ),* $(,)? ]) => {
        &[ $( ($rel, $content) ),* ]
    };
}

static CODEX_FILES: &[(&str, &str)] = integration_files!("codex", [
    ("hooks.json",                      include_str!("../../integrations/codex/hooks.json")),
    ("commands/check.md",               include_str!("../../integrations/codex/commands/check.md")),
    ("commands/evolve.md",              include_str!("../../integrations/codex/commands/evolve.md")),
    ("commands/go.md",                  include_str!("../../integrations/codex/commands/go.md")),
    ("commands/ship.md",                include_str!("../../integrations/codex/commands/ship.md")),
    ("commands/spec.md",                include_str!("../../integrations/codex/commands/spec.md")),
    ("commands/team.md",                include_str!("../../integrations/codex/commands/team.md")),
    ("skills/context/SKILL.md",         include_str!("../../integrations/codex/skills/context/SKILL.md")),
    ("skills/document/SKILL.md",        include_str!("../../integrations/codex/skills/document/SKILL.md")),
    ("skills/perf/SKILL.md",            include_str!("../../integrations/codex/skills/perf/SKILL.md")),
    ("skills/secure/SKILL.md",          include_str!("../../integrations/codex/skills/secure/SKILL.md")),
    ("skills/simplify/SKILL.md",        include_str!("../../integrations/codex/skills/simplify/SKILL.md")),
    ("skills/tdd/SKILL.md",             include_str!("../../integrations/codex/skills/tdd/SKILL.md")),
    ("skills/verify/SKILL.md",          include_str!("../../integrations/codex/skills/verify/SKILL.md")),
    ("agents/auditor.md",               include_str!("../../integrations/codex/agents/auditor.md")),
    ("agents/builder.md",               include_str!("../../integrations/codex/agents/builder.md")),
    ("agents/planner.md",               include_str!("../../integrations/codex/agents/planner.md")),
    ("agents/reviewer.md",              include_str!("../../integrations/codex/agents/reviewer.md")),
]);

static GEMINI_FILES: &[(&str, &str)] = integration_files!("gemini", [
    ("settings.json",                   include_str!("../../integrations/gemini/settings.json")),
    ("GEMINI.md",                       include_str!("../../integrations/gemini/GEMINI.md")),
    ("commands/check.md",               include_str!("../../integrations/gemini/commands/check.md")),
    ("commands/evolve.md",              include_str!("../../integrations/gemini/commands/evolve.md")),
    ("commands/go.md",                  include_str!("../../integrations/gemini/commands/go.md")),
    ("commands/ship.md",                include_str!("../../integrations/gemini/commands/ship.md")),
    ("commands/spec.md",                include_str!("../../integrations/gemini/commands/spec.md")),
    ("commands/team.md",                include_str!("../../integrations/gemini/commands/team.md")),
    ("skills/context/SKILL.md",         include_str!("../../integrations/gemini/skills/context/SKILL.md")),
    ("skills/document/SKILL.md",        include_str!("../../integrations/gemini/skills/document/SKILL.md")),
    ("skills/perf/SKILL.md",            include_str!("../../integrations/gemini/skills/perf/SKILL.md")),
    ("skills/secure/SKILL.md",          include_str!("../../integrations/gemini/skills/secure/SKILL.md")),
    ("skills/simplify/SKILL.md",        include_str!("../../integrations/gemini/skills/simplify/SKILL.md")),
    ("skills/tdd/SKILL.md",             include_str!("../../integrations/gemini/skills/tdd/SKILL.md")),
    ("skills/verify/SKILL.md",          include_str!("../../integrations/gemini/skills/verify/SKILL.md")),
    ("agents/auditor.md",               include_str!("../../integrations/gemini/agents/auditor.md")),
    ("agents/builder.md",               include_str!("../../integrations/gemini/agents/builder.md")),
    ("agents/planner.md",               include_str!("../../integrations/gemini/agents/planner.md")),
    ("agents/reviewer.md",              include_str!("../../integrations/gemini/agents/reviewer.md")),
]);

static CURSOR_FILES: &[(&str, &str)] = integration_files!("cursor", [
    ("hooks.json",                      include_str!("../../integrations/cursor/hooks.json")),
    ("rules/harness-context.mdc",       include_str!("../../integrations/cursor/rules/harness-context.mdc")),
    ("rules/harness-skills.mdc",        include_str!("../../integrations/cursor/rules/harness-skills.mdc")),
    ("commands/check.md",               include_str!("../../integrations/cursor/commands/check.md")),
    ("commands/evolve.md",              include_str!("../../integrations/cursor/commands/evolve.md")),
    ("commands/go.md",                  include_str!("../../integrations/cursor/commands/go.md")),
    ("commands/ship.md",                include_str!("../../integrations/cursor/commands/ship.md")),
    ("commands/spec.md",                include_str!("../../integrations/cursor/commands/spec.md")),
    ("commands/team.md",                include_str!("../../integrations/cursor/commands/team.md")),
    ("agents/auditor.md",               include_str!("../../integrations/cursor/agents/auditor.md")),
    ("agents/builder.md",               include_str!("../../integrations/cursor/agents/builder.md")),
    ("agents/planner.md",               include_str!("../../integrations/cursor/agents/planner.md")),
    ("agents/reviewer.md",              include_str!("../../integrations/cursor/agents/reviewer.md")),
]);

static ANTIGRAVITY_FILES: &[(&str, &str)] = integration_files!("antigravity", [
    ("AGENTS.md",                       include_str!("../../integrations/antigravity/AGENTS.md")),
    ("skills/document.md",              include_str!("../../integrations/antigravity/skills/document.md")),
    ("skills/perf.md",                  include_str!("../../integrations/antigravity/skills/perf.md")),
    ("skills/secure.md",                include_str!("../../integrations/antigravity/skills/secure.md")),
    ("skills/simplify.md",              include_str!("../../integrations/antigravity/skills/simplify.md")),
    ("skills/tdd.md",                   include_str!("../../integrations/antigravity/skills/tdd.md")),
    ("skills/verify.md",                include_str!("../../integrations/antigravity/skills/verify.md")),
    ("workflows/check.md",              include_str!("../../integrations/antigravity/workflows/check.md")),
    ("workflows/evolve.md",             include_str!("../../integrations/antigravity/workflows/evolve.md")),
    ("workflows/go.md",                 include_str!("../../integrations/antigravity/workflows/go.md")),
    ("workflows/ship.md",               include_str!("../../integrations/antigravity/workflows/ship.md")),
    ("workflows/spec.md",               include_str!("../../integrations/antigravity/workflows/spec.md")),
    ("workflows/team.md",               include_str!("../../integrations/antigravity/workflows/team.md")),
    ("agents/auditor.md",               include_str!("../../integrations/antigravity/agents/auditor.md")),
    ("agents/builder.md",               include_str!("../../integrations/antigravity/agents/builder.md")),
    ("agents/planner.md",               include_str!("../../integrations/antigravity/agents/planner.md")),
    ("agents/reviewer.md",              include_str!("../../integrations/antigravity/agents/reviewer.md")),
]);

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
        }),
        "gemini" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".gemini"),
            local_dir: cwd.join(".gemini"),
            root_files: &["GEMINI.md"],
            files: GEMINI_FILES,
            note: Some("If GEMINI.md already exists, append the section manually."),
        }),
        "cursor" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".cursor"),
            local_dir: cwd.join(".cursor"),
            root_files: &[],
            files: CURSOR_FILES,
            note: Some("Requires Cursor 1.7+"),
        }),
        "antigravity" => Some(ToolConfig {
            global_dir: PathBuf::from(&home).join(".agents"),
            local_dir: cwd.join(".agents"),
            root_files: &["AGENTS.md"],
            files: ANTIGRAVITY_FILES,
            note: Some("Ring 0 hooks not available — using AGENTS.md + skills/workflows instead."),
        }),
        _ => None,
    }
}

// ── Install logic ─────────────────────────────────────────────────────────────

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

pub fn run(args: &[String]) -> i32 {
    // Parse: epic-harness install <tool> [--local] [--dry-run]
    let tool = match args.first() {
        Some(t) => t.as_str(),
        None => {
            eprintln!("Usage: epic-harness install <codex|gemini|cursor|antigravity> [--local] [--dry-run]");
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
            eprintln!("[harness] Unknown tool '{tool}'. Use one of: codex, gemini, cursor, antigravity");
            return 1;
        }
    };

    let target_dir = if local { &cfg.local_dir } else { &cfg.global_dir };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    println!("[harness] Installing {tool} → {}", target_dir.display());
    if let Some(note) = cfg.note {
        println!("[harness] Note: {note}");
    }

    for (rel, content) in cfg.files {
        // Root files (GEMINI.md, AGENTS.md) go into cwd, not the tool dir
        let dest = if cfg.root_files.contains(rel) {
            cwd.join(rel)
        } else {
            target_dir.join(rel)
        };
        write_if_missing(&dest, content, dry_run);
    }

    println!("[harness] Done.");
    0
}
