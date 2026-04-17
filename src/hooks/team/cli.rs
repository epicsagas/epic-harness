//! cli.rs — epic team CLI subcommand dispatch

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use super::store::{
    append_playbook, build_agent_file, build_playbook_section, default_agents_for_type,
    default_org, home_dir, inject_team_context, list_agents, list_history, list_orgs, list_teams,
    load_agent, load_mission, load_playbook, load_team_config, read_org_from_agent_file,
    save_agent, save_mission, save_team_config, team_agents_dir, team_exists, team_store_dir,
    TeamConfig,
};

const SUBCOMMANDS: &[(&str, &str)] = &[
    ("list",    "List all teams in an org"),
    ("show",    "Show team details"),
    ("status",  "Show teams linked to the current project"),
    ("sync",    "Sync team agents to .claude/agents/ (--global for ~/.claude/agents/)"),
    ("link",    "Link a team to the current project"),
    ("unlink",  "Remove team agents from current project"),
    ("delete",  "Remove team from current project (--global to disband from org)"),
    ("history", "List agent history backups"),
    ("help",    "Show this help message"),
];

pub fn dispatch(args: &[String]) -> i32 {
    let (_, flags) = parse_flags(args);
    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    let sub = match args.first().map(|s| s.as_str()) {
        Some(s) if !s.starts_with("--") => s,
        _ => return cmd_default(&org, args),
    };

    match sub {
        "list"    => cmd_list(&args[1..]),
        "show"    => cmd_show(&args[1..]),
        "status"  => cmd_status(&args[1..]),
        "sync"    => cmd_sync(&args[1..]),
        "link"    => cmd_link(&args[1..]),
        "unlink"  => cmd_unlink(&args[1..]),
        "delete"  => cmd_delete(&args[1..]),
        "history" => cmd_history(&args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        _ => {
            eprintln!("error: unknown subcommand '{sub}'");
            eprintln!("\nRun 'epic team help' for available subcommands.");
            1
        }
    }
}

fn print_help() {
    println!("epic team — Manage org-level agent teams\n");
    println!("USAGE:");
    println!("  epic team <SUBCOMMAND> [OPTIONS]\n");
    println!("SUBCOMMANDS:");
    for (name, desc) in SUBCOMMANDS {
        println!("  {name:<12} {desc}");
    }
    println!("\nRun 'epic team <SUBCOMMAND> --help' for subcommand-specific options.");
    println!("Run 'epic team' (no args) for interactive team design.");
}

// ── Helpers ───────────────────────────────────────────

fn parse_flags(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let mut positional = vec![];
    let mut flags: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            positional.extend_from_slice(&args[i + 1..]);
            break;
        }
        if let Some(kv) = arg.strip_prefix("--") {
            // --key=value form
            if let Some((k, v)) = kv.split_once('=') {
                flags.insert(k.to_string(), v.to_string());
                i += 1;
            } else {
                // --key [value] — only consume next token as value if it's not a flag
                let next_is_flag = args.get(i + 1).map(|n| n.starts_with("--")).unwrap_or(true);
                if next_is_flag {
                    flags.insert(kv.to_string(), String::new());
                    i += 1;
                } else {
                    flags.insert(kv.to_string(), args[i + 1].clone());
                    i += 2;
                }
            }
        } else {
            positional.push(arg.clone());
            i += 1;
        }
    }
    (positional, flags)
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    line.trim().to_string()
}

fn confirm(msg: &str, default: bool) -> bool {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    let input = prompt(&format!("{} {} ", msg, hint));
    if input.is_empty() {
        return default;
    }
    matches!(input.to_lowercase().as_str(), "y" | "yes")
}

// ── Project scanning ──────────────────────────────────

struct ProjectContext {
    name: String,
    stacks: Vec<String>,
}

fn scan_project() -> ProjectContext {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    let mut stacks = vec![];

    if cwd.join("Cargo.toml").exists() {
        stacks.push("rust".to_string());
    }
    if cwd.join("package.json").exists() {
        stacks.push("node".to_string());
    }
    if cwd.join("pyproject.toml").exists()
        || cwd.join("setup.py").exists()
        || cwd.join("requirements.txt").exists()
    {
        stacks.push("python".to_string());
    }
    if cwd.join("go.mod").exists() {
        stacks.push("go".to_string());
    }
    if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        stacks.push("java".to_string());
    }

    ProjectContext { name, stacks }
}

fn recommend_team_type(_ctx: &ProjectContext) -> &'static str {
    "stream"
}

// ── Sync helper ───────────────────────────────────────

fn sync_to_project(org: &str, team: &str) -> io::Result<u32> {
    sync_to_dest(org, team, false)
}

fn validate_team_name(team: &str) -> io::Result<()> {
    let valid = !team.is_empty()
        && team.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
        && team.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid team name '{}': only [a-zA-Z0-9_-] allowed, must start with alphanumeric", team),
        ))
    }
}

fn validate_org_name(org: &str) -> io::Result<()> {
    let valid = !org.is_empty()
        && org.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
        && org.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid org name '{}': only [a-zA-Z0-9_-] allowed, must start with alphanumeric", org),
        ))
    }
}

/// Returns the agents dir for a tool if that tool appears to be installed globally.
fn installed_tool_agents_dir(tool: &str) -> Option<PathBuf> {
    let home = home_dir();
    let parent = match tool {
        "codex"    => home.join(".codex"),
        "gemini"   => home.join(".gemini"),
        "cursor"   => home.join(".cursor"),
        "opencode" => home.join(".config").join("opencode"),
        _ => return None,
    };
    if parent.exists() {
        Some(parent.join("agents"))
    } else {
        None
    }
}

fn sync_to_dest(org: &str, team: &str, global: bool) -> io::Result<u32> {
    validate_team_name(team)?;

    let config = load_team_config(org, team)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Team '{}' not found in org '{}'", team, org)))?;

    let mission = load_mission(org, team).unwrap_or_default();

    let dest = if global {
        let base = home_dir().join(".claude").join("agents");
        // Create and canonicalize base BEFORE creating team subdir (TOCTOU defense)
        fs::create_dir_all(&base)
            .map_err(|e| io::Error::new(e.kind(), format!("failed to create {}: {}", base.display(), e)))?;
        let canon_base = base.canonicalize().map_err(io::Error::other)?;
        let candidate = canon_base.join(team);
        // If team subdir already exists, verify it's not a symlink escaping base
        if candidate.exists() {
            let canon_candidate = candidate.canonicalize().map_err(io::Error::other)?;
            if !canon_candidate.starts_with(&canon_base) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("resolved path '{}' is outside ~/.claude/agents — aborting", canon_candidate.display()),
                ));
            }
        }
        fs::create_dir_all(&candidate)
            .map_err(|e| io::Error::new(e.kind(), format!("failed to create {}: {}", candidate.display(), e)))?;
        candidate
    } else {
        let cwd = std::env::current_dir().map_err(io::Error::other)?;
        let d = cwd.join(".claude").join("agents").join(team);
        fs::create_dir_all(&d)
            .map_err(|e| io::Error::new(e.kind(), format!("failed to create {}: {}", d.display(), e)))?;
        d
    };

    let agents = list_agents(org, team);
    let mut count = 0u32;

    for agent_name in &agents {
        if let Some(content) = load_agent(org, team, agent_name) {
            let injected = inject_team_context(&content, org, team, &config.team_type, &mission);
            let dest_path = dest.join(format!("{}.md", agent_name));
            fs::write(&dest_path, injected)?;
            count += 1;
        }
    }

    // Also sync to other installed tools with tool-specific transforms
    let other_tools = ["codex", "gemini", "cursor", "opencode"];
    for tool in &other_tools {
        if let Some(agents_dir) = installed_tool_agents_dir(tool) {
            let tool_team_dir = agents_dir.join(team);
            if let Err(e) = fs::create_dir_all(&tool_team_dir) {
                eprintln!("[harness] warn: could not create {}: {}", tool_team_dir.display(), e);
                continue;
            }
            for agent_name in &agents {
                if let Some(content) = load_agent(org, team, agent_name) {
                    let injected = inject_team_context(&content, org, team, &config.team_type, &mission);
                    let transformed = crate::hooks::install::transform_agent(tool, agent_name, &injected);
                    let dest_path = tool_team_dir.join(format!("{}.md", agent_name));
                    if let Err(e) = fs::write(&dest_path, &transformed) {
                        eprintln!("[harness] warn: could not write {}: {}", dest_path.display(), e);
                    }
                }
            }
        }
    }

    Ok(count)
}

fn today_str() -> String {
    let iso = crate::hooks::common::now_iso();
    iso[..10].to_string()
}

// ── cmd_default: interactive design flow ──────────────

fn cmd_default(org: &str, args: &[String]) -> i32 {
    let (_, flags) = parse_flags(args);
    let yes = flags.contains_key("yes");

    let ctx = scan_project();

    let suggested_name: String = ctx
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    let valid_types = ["stream", "platform", "enabling", "subsystem"];
    let suggested_type = recommend_team_type(&ctx);

    if yes {
        // ── Non-interactive path ──────────────────────────
        let team_name = flags.get("name").cloned().unwrap_or_else(|| suggested_name.clone());
        if let Err(e) = validate_team_name(&team_name) {
            eprintln!("error: {}", e);
            return 1;
        }

        let team_type = flags.get("type").cloned().unwrap_or_else(|| suggested_type.to_string());
        if !valid_types.contains(&team_type.as_str()) {
            eprintln!("error: invalid --type '{}'. Must be one of: stream, platform, enabling, subsystem", team_type);
            return 1;
        }

        let mission = match flags.get("mission").cloned() {
            Some(m) if !m.is_empty() && m.len() <= 200 => m,
            Some(m) if m.len() > 200 => {
                eprintln!("error: --mission too long (max 200 chars, got {})", m.len());
                return 1;
            }
            _ => {
                eprintln!("error: --mission is required with --yes");
                eprintln!("Usage: epic team --yes --name <name> --type <type> --mission \"<mission>\"");
                return 1;
            }
        };

        println!("Org: {}", org);
        println!("Project: {}", ctx.name);
        println!("Team: {} ({}) — {}", team_name, team_type, mission);

        let proposed_agents = default_agents_for_type(&team_type);
        println!("Agents: {}", proposed_agents.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "));

        return cmd_default_write(org, &ctx, &team_name, &team_type, &mission, true);
    }

    // ── Interactive path ──────────────────────────────
    println!("Org: {}  (use --org <name> to target a different org)", org);
    println!();

    println!("Project: {}", ctx.name);
    if ctx.stacks.is_empty() {
        println!("Stack: (none detected)");
    } else {
        println!("Stack: {}", ctx.stacks.join(", "));
    }
    println!();

    let existing_teams = list_teams(org);
    if !existing_teams.is_empty() {
        println!("Existing teams in '{}': {}", org, existing_teams.join(", "));
        println!();
    }

    // Team name
    let team_name_input = prompt(&format!("Team name [{}]: ", suggested_name));
    let team_name = if team_name_input.is_empty() { suggested_name.clone() } else { team_name_input };
    if let Err(e) = validate_team_name(&team_name) {
        eprintln!("error: {}", e);
        return 1;
    }

    // Team type
    let team_type = loop {
        let input = prompt(&format!(
            "Team type (stream/platform/enabling/subsystem) [{}]: ",
            suggested_type
        ));
        let t = if input.is_empty() { suggested_type.to_string() } else { input };
        if valid_types.contains(&t.as_str()) { break t; }
        println!("Invalid type '{}'. Must be one of: stream, platform, enabling, subsystem", t);
    };

    // Mission
    let mission = loop {
        let input = prompt("Mission (one-line domain ownership): ");
        if !input.is_empty() { break input; }
        println!("Mission cannot be empty.");
    };

    println!();

    let proposed_agents = default_agents_for_type(&team_type);
    println!("Proposed agents:");
    for (name, desc) in &proposed_agents {
        println!("  - {}: {}", name, desc);
    }
    println!("  (based on {} template)", team_type);
    println!();

    if !confirm("Proceed?", true) {
        println!("Aborted.");
        return 0;
    }
    println!();

    cmd_default_write(org, &ctx, &team_name, &team_type, &mission, false)
}

fn cmd_default_write(
    org: &str,
    ctx: &ProjectContext,
    team_name: &str,
    team_type: &str,
    mission: &str,
    auto_sync: bool,
) -> i32 {
    let proposed_agents = default_agents_for_type(team_type);
    let agents_with_names: Vec<(String, String)> = proposed_agents
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_string()))
        .collect();

    if team_exists(org, team_name) {
        let old_mission = load_mission(org, team_name).unwrap_or_default();
        let old_trimmed = old_mission.trim();
        let new_trimmed = mission.trim();
        if old_trimmed != new_trimmed {
            if auto_sync {
                if let Err(e) = save_mission(org, team_name, mission) {
                    eprintln!("error saving mission: {}", e);
                    return 1;
                }
                println!("+ Mission updated");
            } else {
                println!("Mission changed:");
                println!("  OLD: {}", old_trimmed);
                println!("  NEW: {}", new_trimmed);
                if confirm("Replace mission?", false) {
                    if let Err(e) = save_mission(org, team_name, mission) {
                        eprintln!("error saving mission: {}", e);
                        return 1;
                    }
                } else {
                    println!("  → keeping existing mission");
                }
            }
        }

        for (agent_name, agent_desc) in &agents_with_names {
            let new_content = build_agent_file(agent_name, agent_desc, team_name, team_type);
            match load_agent(org, team_name, agent_name) {
                None => {
                    if let Err(e) = save_agent(org, team_name, agent_name, &new_content, false) {
                        eprintln!("error saving agent '{}': {}", agent_name, e);
                        return 1;
                    }
                    println!("+ Added agent: {}", agent_name);
                }
                Some(existing_content) => {
                    if existing_content.trim() == new_content.trim() {
                        println!("  (no change) {}", agent_name);
                    } else if auto_sync {
                        if let Err(e) = save_agent(org, team_name, agent_name, &new_content, true) {
                            eprintln!("error saving agent '{}': {}", agent_name, e);
                            return 1;
                        }
                        println!("+ Updated agent: {}", agent_name);
                    } else {
                        println!("Agent '{}' has changed ({} → {} chars).", agent_name, existing_content.len(), new_content.len());
                        if confirm(&format!("Replace '{}'?", agent_name), false) {
                            if let Err(e) = save_agent(org, team_name, agent_name, &new_content, true) {
                                eprintln!("error saving agent '{}': {}", agent_name, e);
                                return 1;
                            }
                            println!("  → replaced (old backed up to .history/)");
                        } else {
                            println!("  → kept existing");
                        }
                    }
                }
            }
        }

        let playbook_section = build_playbook_section(team_name, team_type, &agents_with_names, &ctx.name);
        if let Err(e) = append_playbook(org, team_name, &playbook_section, &ctx.name, &today_str()) {
            eprintln!("error updating playbook: {}", e);
            return 1;
        }
        println!("+ Playbook updated");

        if let Some(mut config) = load_team_config(org, team_name) {
            if !config.projects.contains(&ctx.name) {
                config.projects.push(ctx.name.clone());
            }
            config.updated = crate::hooks::common::now_iso();
            if let Err(e) = save_team_config(&config) {
                eprintln!("error saving config: {}", e);
                return 1;
            }
        }
    } else {
        let store_dir = team_store_dir(org, team_name);
        if let Err(e) = fs::create_dir_all(&store_dir) {
            eprintln!("error creating team directory: {}", e);
            return 1;
        }
        if let Err(e) = fs::create_dir_all(team_agents_dir(org, team_name)) {
            eprintln!("error creating agents directory: {}", e);
            return 1;
        }

        let now = crate::hooks::common::now_iso();
        let config = TeamConfig {
            name: team_name.to_string(),
            org: org.to_string(),
            team_type: team_type.to_string(),
            projects: vec![ctx.name.clone()],
            created: now.clone(),
            updated: now,
        };
        if let Err(e) = save_team_config(&config) {
            eprintln!("error saving config: {}", e);
            return 1;
        }
        println!("+ Created config.json");

        if let Err(e) = save_mission(org, team_name, mission) {
            eprintln!("error saving mission: {}", e);
            return 1;
        }
        println!("+ Created mission.md");

        for (agent_name, agent_desc) in &agents_with_names {
            let content = build_agent_file(agent_name, agent_desc, team_name, team_type);
            if let Err(e) = save_agent(org, team_name, agent_name, &content, false) {
                eprintln!("error saving agent '{}': {}", agent_name, e);
                return 1;
            }
            println!("+ Added agent: {}", agent_name);
        }

        let playbook_content = build_playbook_section(team_name, team_type, &agents_with_names, &ctx.name);
        let playbook_path = store_dir.join("playbook.md");
        if let Err(e) = fs::write(&playbook_path, &playbook_content) {
            eprintln!("error creating playbook: {}", e);
            return 1;
        }
        println!("+ Created playbook.md");

        println!();
        println!("Team '{}' created in org '{}'", team_name, org);
        println!("  Path: {}", store_dir.display());
    }

    println!();

    let do_sync = auto_sync || confirm(&format!("Sync agents to ./.claude/agents/{}/? ", team_name), true);
    if do_sync {
        match sync_to_project(org, team_name) {
            Ok(count) => println!("✓ Synced {} agent(s) to .claude/agents/{}/", count, team_name),
            Err(e) => { eprintln!("error syncing: {}", e); return 1; }
        }
    } else {
        println!("  Run 'epic team sync {}' to sync later", team_name);
    }

    0
}

// ── cmd_list ──────────────────────────────────────────

fn cmd_list(args: &[String]) -> i32 {
    let (_, flags) = parse_flags(args);
    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    let teams = list_teams(&org);
    if teams.is_empty() {
        println!("No teams found in org '{}'.", org);
        println!("Run 'epic team' to create a team.");
        return 0;
    }

    println!("Teams in org '{}':", org);
    for team in &teams {
        if let Some(config) = load_team_config(&org, team) {
            let projects_str = if config.projects.is_empty() {
                "(none)".to_string()
            } else {
                config.projects.join(", ")
            };
            println!("  {:<16} ({:<10}) projects: {}", team, config.team_type, projects_str);
        } else {
            println!("  {:<16} (unknown)", team);
        }
    }
    0
}

// ── cmd_show ──────────────────────────────────────────

fn cmd_show(args: &[String]) -> i32 {
    let (pos, flags) = parse_flags(args);
    let team = match pos.first() {
        Some(t) => t.clone(),
        None => {
            eprintln!("error: show requires <team>");
            eprintln!("Usage: epic team show <team> [--org <name>] [--playbook]");
            return 1;
        }
    };

    if let Err(e) = validate_team_name(&team) {
        eprintln!("error: {}", e);
        return 1;
    }

    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    let show_playbook = flags.contains_key("playbook");

    if !team_exists(&org, &team) {
        eprintln!("error: team '{}' not found in org '{}'", team, org);
        return 1;
    }

    let config = load_team_config(&org, &team);
    let mission = load_mission(&org, &team).unwrap_or_else(|| "(no mission set)".to_string());
    let agents = list_agents(&org, &team);

    println!("Team: {}", team);
    if let Some(ref c) = config {
        println!("Org:  {}", c.org);
        println!("Type: {}", c.team_type);
        println!("Projects: {}", if c.projects.is_empty() { "(none)".to_string() } else { c.projects.join(", ") });
        println!("Created: {}", c.created);
        println!("Updated: {}", c.updated);
    }
    println!("Mission: {}", mission.trim());
    println!();
    println!("Agents ({}):", agents.len());
    for agent_name in &agents {
        // Print first description line from frontmatter
        let desc = load_agent(&org, &team, agent_name)
            .and_then(|content| {
                // Look for "description:" in frontmatter
                content.lines()
                    .find(|line| line.trim_start().starts_with("description:"))
                    .map(|line| line.trim_start_matches("description:").trim().to_string())
            })
            .unwrap_or_default();
        if desc.is_empty() {
            println!("  - {}", agent_name);
        } else {
            println!("  - {}: {}", agent_name, desc);
        }
    }

    if show_playbook {
        println!();
        println!("--- Playbook ---");
        let playbook = load_playbook(&org, &team);
        if playbook.is_empty() {
            println!("(no playbook)");
        } else {
            println!("{}", playbook);
        }
    }

    0
}

// ── cmd_sync ──────────────────────────────────────────

fn cmd_sync(args: &[String]) -> i32 {
    let (pos, flags) = parse_flags(args);
    let team = match pos.first() {
        Some(t) => t.clone(),
        None => {
            eprintln!("error: sync requires <team>");
            eprintln!("Usage: epic team sync <team> [--org <name>] [--global]");
            return 1;
        }
    };

    let org = flags.get("org").cloned().unwrap_or_else(default_org);
    let global = flags.contains_key("global");

    if let Err(e) = validate_team_name(&team) {
        eprintln!("error: {}", e);
        return 1;
    }

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    if !team_exists(&org, &team) {
        eprintln!("error: team '{}' not found in org '{}'", team, org);
        return 1;
    }

    let result = sync_to_dest(&org, &team, global);

    match result {
        Ok(count) => {
            let dest = if global {
                home_dir().join(".claude").join("agents").join(&team)
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".claude").join("agents").join(&team)
            };
            println!("✓ Synced {} agent(s) to {}/", count, dest.display());
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

// ── cmd_link ──────────────────────────────────────────

fn cmd_link(args: &[String]) -> i32 {
    let (pos, flags) = parse_flags(args);

    // If no team name provided, show interactive picker
    if pos.is_empty() {
        let org_filter = flags.get("org").map(|s| s.as_str());
        return cmd_link_interactive(org_filter);
    }

    let team = pos[0].clone();

    if let Err(e) = validate_team_name(&team) {
        eprintln!("error: {}", e);
        return 1;
    }

    // Resolve org: --org flag takes priority, otherwise search all orgs
    let org = if let Some(explicit_org) = flags.get("org") {
        if let Err(e) = validate_org_name(explicit_org) {
            eprintln!("error: {}", e);
            return 1;
        }
        explicit_org.clone()
    } else {
        let orgs = list_orgs();
        let matches: Vec<String> = orgs
            .into_iter()
            .filter(|o| team_exists(o, &team))
            .collect();

        match matches.len() {
            0 => {
                eprintln!("error: team '{}' not found in any org. Run 'epic org list' to browse.", team);
                return 1;
            }
            1 => {
                println!("(using org: {})", matches[0]);
                matches.into_iter().next().unwrap()
            }
            _ => {
                println!("Team '{}' found in multiple orgs:", team);
                for (i, o) in matches.iter().enumerate() {
                    println!("  {}) {}", i + 1, o);
                }
                let input = prompt("Select org [1]: ");
                let idx: usize = if input.is_empty() {
                    1
                } else {
                    match input.parse::<usize>() {
                        Ok(n) if n >= 1 && n <= matches.len() => n,
                        _ => {
                            eprintln!("error: invalid selection");
                            return 1;
                        }
                    }
                };
                matches.into_iter().nth(idx - 1).unwrap()
            }
        }
    };

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    if !team_exists(&org, &team) {
        eprintln!("error: team '{}' not found in org '{}'", team, org);
        eprintln!("Run 'epic team list' to see available teams.");
        return 1;
    }

    // Sync agents to project
    match sync_to_project(&org, &team) {
        Ok(count) => {
            println!("✓ Synced {} agent(s) to .claude/agents/{}/", count, team);
        }
        Err(e) => {
            eprintln!("error syncing agents: {}", e);
            return 1;
        }
    }

    // Add current project to team config
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    match load_team_config(&org, &team) {
        Some(mut config) => {
            if !config.projects.contains(&project_name) {
                config.projects.push(project_name.clone());
                config.updated = crate::hooks::common::now_iso();
                if let Err(e) = save_team_config(&config) {
                    eprintln!("error updating team config: {}", e);
                    return 1;
                }
                println!("+ Added project '{}' to team config", project_name);
            } else {
                println!("  (project '{}' already linked)", project_name);
            }
        }
        None => eprintln!("warning: could not load team config — project registration skipped"),
    }

    println!("Team '{}' linked to project '{}'", team, project_name);
    0
}

fn cmd_link_interactive(org_filter: Option<&str>) -> i32 {
    let orgs = list_orgs();
    if orgs.is_empty() {
        eprintln!("No orgs found. Run 'epic team' to create a team.");
        return 1;
    }

    // Build numbered list of all org/team pairs (filtered by org if provided)
    let mut entries: Vec<(String, String)> = vec![];
    for org in &orgs {
        if org_filter.is_some_and(|filter| org.as_str() != filter) {
            continue;
        }
        for team in list_teams(org) {
            entries.push((org.clone(), team));
        }
    }

    if entries.is_empty() {
        eprintln!("No teams found in any org. Run 'epic team' to create a team.");
        return 1;
    }

    println!("Available teams:");
    for (i, (org, team)) in entries.iter().enumerate() {
        println!("  {}) {}/{}", i + 1, org, team);
    }

    let input = prompt("Select team [1]: ");
    let idx: usize = if input.is_empty() {
        1
    } else {
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= entries.len() => n,
            _ => {
                eprintln!("error: invalid selection");
                return 1;
            }
        }
    };

    let (org, team) = entries.into_iter().nth(idx - 1).unwrap();
    let link_args: Vec<String> = vec![team, "--org".to_string(), org];
    cmd_link(&link_args)
}

// ── cmd_unlink ────────────────────────────────────────
// Alias for `delete` (without --global) — kept for discoverability

fn cmd_unlink(args: &[String]) -> i32 {
    cmd_delete(args)
}

// ── cmd_delete ────────────────────────────────────────

fn cmd_delete(args: &[String]) -> i32 {
    let (pos, flags) = parse_flags(args);
    let team = match pos.first() {
        Some(t) => t.clone(),
        None => {
            eprintln!("error: delete requires <team>");
            eprintln!("Usage: epic team delete <team> [--org <name>] [--global]");
            return 1;
        }
    };

    if let Err(e) = validate_team_name(&team) {
        eprintln!("error: {}", e);
        return 1;
    }

    let global = flags.contains_key("global");

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let local_agents_dir = cwd.join(".claude").join("agents").join(&team);

    // Resolve org: --org flag > frontmatter in any local agent file > "epic"
    let org = flags.get("org").cloned().unwrap_or_else(|| {
        // Try reading org from frontmatter of any synced agent file
        if local_agents_dir.is_dir()
            && let Ok(entries) = fs::read_dir(&local_agents_dir)
        {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("md")
                    && let Ok(content) = fs::read_to_string(entry.path())
                    && let Some(org) = read_org_from_agent_file(&content)
                    && validate_org_name(&org).is_ok()
                {
                    return org;
                }
            }
        }
        default_org()
    });

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    if global {
        // --global: permanently delete from org store (+ local if present)
        if !team_exists(&org, &team) {
            eprintln!("error: team '{}' not found in org '{}'", team, org);
            return 1;
        }

        let store_dir = team_store_dir(&org, &team);
        println!("This will permanently delete:");
        println!("  Global store: {}", store_dir.display());
        if local_agents_dir.exists() {
            println!("  Local agents: {}", local_agents_dir.display());
        }
        println!();
        println!("  ⚠  This cannot be undone. All agents and .history/ backups will be removed.");
        println!();

        if !confirm(&format!("Permanently delete team '{}' from org '{}'?", team, org), false) {
            println!("Aborted.");
            return 0;
        }

        match fs::remove_dir_all(&store_dir) {
            Ok(_) => println!("✓ Deleted global store: {}", store_dir.display()),
            Err(e) => {
                eprintln!("error removing global store: {}", e);
                return 1;
            }
        }
        if local_agents_dir.exists() {
            match fs::remove_dir_all(&local_agents_dir) {
                Ok(_) => println!("✓ Removed local agents: .claude/agents/{}/", team),
                Err(e) => eprintln!("warning: could not remove local agents: {}", e),
            }
        }
        println!();
        println!("Team '{}' permanently deleted from org '{}'.", team, org);
    } else {
        // default: remove from current project only (.claude/agents/{team}/)
        if !local_agents_dir.exists() {
            println!("Team '{}' is not linked to this project (.claude/agents/{}/ not found).", team, team);
            return 0;
        }
        match fs::remove_dir_all(&local_agents_dir) {
            Ok(_) => {
                println!("✓ Removed .claude/agents/{}/", team);
                println!("  (Global store untouched. Use 'epic team link {}' to re-attach.)", team);
            }
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        }
    }

    0
}

// ── cmd_history ───────────────────────────────────────

fn cmd_history(args: &[String]) -> i32 {
    let (pos, flags) = parse_flags(args);

    let team = match pos.first() {
        Some(t) => t.clone(),
        None => {
            eprintln!("error: history requires <team> <agent>");
            eprintln!("Usage: epic team history <team> <agent> [--org <name>]");
            return 1;
        }
    };
    let agent = match pos.get(1) {
        Some(a) => a.clone(),
        None => {
            eprintln!("error: history requires <team> <agent>");
            eprintln!("Usage: epic team history <team> <agent> [--org <name>]");
            return 1;
        }
    };

    if let Err(e) = validate_team_name(&team) {
        eprintln!("error: {}", e);
        return 1;
    }

    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    if !team_exists(&org, &team) {
        eprintln!("error: team '{}' not found in org '{}'", team, org);
        return 1;
    }

    let history = list_history(&org, &team, &agent);
    if history.is_empty() {
        println!("No history found for agent '{}' in team '{}'.", agent, team);
        return 0;
    }

    println!("History for agent '{}' in team '{}':", agent, team);
    for entry in &history {
        println!("  {}", entry);
    }
    0
}

// ── cmd_status ────────────────────────────────────────

fn cmd_status(args: &[String]) -> i32 {
    let (_, flags) = parse_flags(args);
    let _ = flags; // reserved for --json later

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let agents_base = cwd.join(".claude").join("agents");

    if !agents_base.is_dir() {
        println!("Project: {}", project_name);
        println!("No teams linked to this project.");
        println!("Run 'epic org list' to browse available teams.");
        return 0;
    }

    // Collect team subdirs
    let mut team_dirs: Vec<String> = fs::read_dir(&agents_base)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    team_dirs.sort();

    if team_dirs.is_empty() {
        println!("Project: {}", project_name);
        println!("No teams linked to this project.");
        println!("Run 'epic org list' to browse available teams.");
        return 0;
    }

    println!("Project: {}", project_name);
    println!("Linked teams ({}):", team_dirs.len());

    for team in &team_dirs {
        let team_dir = agents_base.join(team);

        // Single pass: collect all .md entries, read first for org, list all for agents
        let (org, agents) = {
            let entries: Vec<_> = fs::read_dir(&team_dir)
                .ok()
                .map(|e| {
                    e.filter_map(|x| x.ok())
                        .filter(|x| {
                            x.path().extension().and_then(|ext| ext.to_str()) == Some("md")
                        })
                        .collect()
                })
                .unwrap_or_default();

            // B5: validate org from frontmatter; treat invalid as "(unknown)"
            let org = entries
                .first()
                .and_then(|e| fs::read_to_string(e.path()).ok())
                .and_then(|content| read_org_from_agent_file(&content))
                .and_then(|o| if validate_org_name(&o).is_ok() { Some(o) } else { None })
                .unwrap_or_else(|| "(unknown)".to_string());

            let mut names: Vec<String> = entries
                .iter()
                .filter_map(|e| e.file_name().into_string().ok())
                .map(|n| n.trim_end_matches(".md").to_string())
                .collect();
            names.sort();

            (org, names)
        };

        // Cross-reference org store for type and mission
        let (team_type, mission_first_line) = if org != "(unknown)" {
            let tc = load_team_config(&org, team);
            let t = tc.as_ref().map(|c| c.team_type.as_str()).unwrap_or("unknown").to_string();
            let m = load_mission(&org, team)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (t, m)
        } else {
            ("unknown".to_string(), String::new())
        };

        let mission_display = if mission_first_line.is_empty() {
            String::new()
        } else {
            format!("   {}", mission_first_line)
        };

        println!(
            "  {:<12} ({}, org: {}){}\n             agents: {}",
            team,
            team_type,
            org,
            mission_display,
            if agents.is_empty() { "(none)".to_string() } else { agents.join(", ") }
        );
    }

    println!();
    println!("Run 'epic team link <team>' to hire more teams.");
    0
}

// ── org subcommand dispatch ───────────────────────────

pub fn dispatch_org(args: &[String]) -> i32 {
    let sub = match args.first().map(|s| s.as_str()) {
        Some(s) if !s.starts_with("--") => s,
        _ => return cmd_org_list(),
    };
    match sub {
        "list" => cmd_org_list(),
        "show" => cmd_org_show(&args[1..]),
        "help" | "--help" | "-h" => {
            print_org_help();
            0
        }
        _ => {
            eprintln!("error: unknown org subcommand '{}'\nRun 'epic org help'.", sub);
            1
        }
    }
}

fn print_org_help() {
    println!("epic org — Browse org team libraries\n");
    println!("USAGE:");
    println!("  epic org <SUBCOMMAND>\n");
    println!("SUBCOMMANDS:");
    println!("  list           List all orgs and their teams (default)");
    println!("  show <org>     Show teams in a specific org");
    println!("  help           Show this help message");
}

fn cmd_org_list() -> i32 {
    let orgs = list_orgs();
    if orgs.is_empty() {
        println!("No orgs found. Run 'epic team' to create a team.");
        return 0;
    }

    println!("Available orgs:");
    for org in &orgs {
        // One read_dir per org is the minimum required (no caching needed for interactive CLI).
        let teams = list_teams(org);
        let count = teams.len();
        let team_word = if count == 1 { "team" } else { "teams" };
        let names = if teams.is_empty() {
            "(none)".to_string()
        } else {
            teams.join(", ")
        };
        println!("  {:<12} {} {}: {}", org, count, team_word, names);
    }
    0
}

fn cmd_org_show(args: &[String]) -> i32 {
    let (pos, _) = parse_flags(args);
    let org = match pos.first() {
        Some(o) => o.clone(),
        None => {
            eprintln!("error: show requires <org>");
            eprintln!("Usage: epic org show <org>");
            return 1;
        }
    };

    if let Err(e) = validate_org_name(&org) {
        eprintln!("error: {}", e);
        return 1;
    }

    let teams = list_teams(&org);
    println!("Org: {}", org);
    if teams.is_empty() {
        println!("Teams (0): (none)");
        return 0;
    }

    println!("Teams ({}):", teams.len());
    for team in &teams {
        let (team_type, mission_first_line) = {
            let tc = load_team_config(&org, team);
            let t = tc.as_ref().map(|c| c.team_type.as_str()).unwrap_or("unknown").to_string();
            let m = load_mission(&org, team)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (t, m)
        };
        println!(
            "  {:<16} ({:<10}) {}",
            team,
            team_type,
            mission_first_line
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_args(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    // ── parse_flags tests (C1) ────────────────────────────

    #[test]
    fn test_parse_flags_boolean() {
        let args = to_args(&["--yes", "--name", "foo"]);
        let (pos, flags) = parse_flags(&args);
        assert_eq!(flags["yes"], "");
        assert_eq!(flags["name"], "foo");
        assert!(pos.is_empty());
    }

    #[test]
    fn test_parse_flags_key_value_equals() {
        let args = to_args(&["--format=json", "--limit=5"]);
        let (_, flags) = parse_flags(&args);
        assert_eq!(flags["format"], "json");
        assert_eq!(flags["limit"], "5");
    }

    #[test]
    fn test_parse_flags_double_dash_separator() {
        let args = to_args(&["--org", "epic", "--", "extra", "args"]);
        let (pos, flags) = parse_flags(&args);
        assert_eq!(flags["org"], "epic");
        assert_eq!(pos, vec!["extra", "args"]);
    }

    #[test]
    fn test_parse_flags_positional_mixed() {
        let args = to_args(&["show", "--org", "epic", "myteam"]);
        let (pos, flags) = parse_flags(&args);
        assert_eq!(pos, vec!["show", "myteam"]);
        assert_eq!(flags["org"], "epic");
    }

    // ── validate_org_name tests (C2) ──────────────────────

    #[test]
    fn test_validate_org_name_valid() {
        assert!(validate_org_name("epic").is_ok());
        assert!(validate_org_name("my-org").is_ok());
        assert!(validate_org_name("Org_2").is_ok());
    }

    #[test]
    fn test_validate_org_name_path_traversal() {
        assert!(validate_org_name("../../etc").is_err());
        assert!(validate_org_name("../secret").is_err());
        assert!(validate_org_name("").is_err());
    }
}
