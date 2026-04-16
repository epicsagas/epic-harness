//! cli.rs — epic team CLI subcommand dispatch

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use super::store::{
    append_playbook, build_agent_file, build_playbook_section, default_agents_for_type,
    default_org, inject_team_context, list_agents, list_history, list_teams, load_agent,
    load_mission, load_playbook, load_team_config, read_org_from_agent_file, save_agent,
    save_mission, save_team_config, team_agents_dir, team_exists, team_store_dir, TeamConfig,
};

const SUBCOMMANDS: &[(&str, &str)] = &[
    ("list",    "List all teams in an org"),
    ("show",    "Show team details"),
    ("sync",    "Sync team agents to .claude/agents/"),
    ("link",    "Link a team to the current project"),
    ("unlink",  "Remove team agents from current project"),
    ("delete",  "Remove team from current project (--global to disband from org)"),
    ("history", "List agent history backups"),
    ("help",    "Show this help message"),
];

pub fn dispatch(args: &[String]) -> i32 {
    let sub = match args.first().map(|s| s.as_str()) {
        Some(s) => s,
        None => return cmd_default(),
    };

    match sub {
        "list"    => cmd_list(&args[1..]),
        "show"    => cmd_show(&args[1..]),
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
        if args[i].starts_with("--") {
            let key = args[i].trim_start_matches('-').to_string();
            let val = args.get(i + 1).cloned().unwrap_or_default();
            flags.insert(key, val);
            i += 2;
        } else {
            positional.push(args[i].clone());
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
    readme_excerpt: String,
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

    let readme_excerpt = fs::read_to_string(cwd.join("README.md"))
        .unwrap_or_default()
        .chars()
        .take(400)
        .collect();

    ProjectContext {
        name,
        stacks,
        readme_excerpt,
    }
}

fn recommend_team_type(_ctx: &ProjectContext) -> &'static str {
    "stream"
}

// ── Sync helper ───────────────────────────────────────

fn sync_to_project(org: &str, team: &str) -> io::Result<u32> {
    let config = load_team_config(org, team)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Team '{}' not found in org '{}'", team, org)))?;

    let mission = load_mission(org, team).unwrap_or_default();

    let cwd = std::env::current_dir()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let dest = cwd.join(".claude").join("agents").join(team);
    fs::create_dir_all(&dest)?;

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

    Ok(count)
}

fn today_str() -> String {
    let iso = crate::hooks::common::now_iso();
    iso[..10].to_string()
}

// ── cmd_default: interactive design flow ──────────────

fn cmd_default() -> i32 {
    // 1. Resolve org — default "epic", override with --org at any subcommand
    let org = "epic".to_string();
    println!("Org: {}  (use --org <name> to target a different org)", org);
    println!();

    // 2. Scan project
    let ctx = scan_project();
    println!("Project: {}", ctx.name);
    if ctx.stacks.is_empty() {
        println!("Stack: (none detected)");
    } else {
        println!("Stack: {}", ctx.stacks.join(", "));
    }
    println!();

    // 3. List existing teams
    let existing_teams = list_teams(&org);
    if !existing_teams.is_empty() {
        println!("Existing teams in '{}': {}", org, existing_teams.join(", "));
        println!();
    }

    // 4. Prompt team name
    let suggested_name: String = ctx
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let team_name_input = prompt(&format!("Team name [{}]: ", suggested_name));
    let team_name = if team_name_input.is_empty() {
        suggested_name.clone()
    } else {
        team_name_input
    };

    if team_name.is_empty() {
        eprintln!("error: team name cannot be empty");
        return 1;
    }

    // 5. Prompt team type
    let suggested_type = recommend_team_type(&ctx);
    let valid_types = ["stream", "platform", "enabling", "subsystem"];
    let team_type = loop {
        let input = prompt(&format!(
            "Team type (stream/platform/enabling/subsystem) [{}]: ",
            suggested_type
        ));
        let t = if input.is_empty() {
            suggested_type.to_string()
        } else {
            input
        };
        if valid_types.contains(&t.as_str()) {
            break t;
        }
        println!("Invalid type '{}'. Must be one of: stream, platform, enabling, subsystem", t);
    };

    // 6. Prompt mission
    let mission = loop {
        let input = prompt("Mission (one-line domain ownership): ");
        if !input.is_empty() {
            break input;
        }
        println!("Mission cannot be empty.");
    };

    println!();

    // 7. Show proposed agents
    let proposed_agents = default_agents_for_type(&team_type);
    println!("Proposed agents:");
    for (name, desc) in &proposed_agents {
        println!("  - {}: {}", name, desc);
    }
    println!("  (based on {} template)", team_type);
    println!();

    // 8. Confirm
    if !confirm("Proceed?", true) {
        println!("Aborted.");
        return 0;
    }
    println!();

    // 9. Write phase
    let agents_with_names: Vec<(String, String)> = proposed_agents
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_string()))
        .collect();

    if team_exists(&org, &team_name) {
        // Existing team — update path

        // a. Check mission
        let old_mission = load_mission(&org, &team_name).unwrap_or_default();
        let old_trimmed = old_mission.trim();
        let new_trimmed = mission.trim();
        if old_trimmed != new_trimmed {
            println!("Mission changed:");
            println!("  OLD: {}", old_trimmed);
            println!("  NEW: {}", new_trimmed);
            if confirm("Replace mission?", false) {
                if let Err(e) = save_mission(&org, &team_name, &mission) {
                    eprintln!("error saving mission: {}", e);
                    return 1;
                }
            } else {
                println!("  → keeping existing mission");
            }
        }

        // b. For each proposed agent
        for (agent_name, agent_desc) in &agents_with_names {
            let new_content = build_agent_file(agent_name, agent_desc, &team_name, &team_type);
            match load_agent(&org, &team_name, agent_name) {
                None => {
                    // New agent
                    if let Err(e) = save_agent(&org, &team_name, agent_name, &new_content, false) {
                        eprintln!("error saving agent '{}': {}", agent_name, e);
                        return 1;
                    }
                    println!("+ Added agent: {}", agent_name);
                }
                Some(existing_content) => {
                    if existing_content.trim() == new_content.trim() {
                        println!("  (no change) {}", agent_name);
                    } else {
                        println!("Agent '{}' has changed.", agent_name);
                        println!(
                            "  (new version has {} chars, old had {} chars)",
                            new_content.len(),
                            existing_content.len()
                        );
                        if confirm(&format!("Replace '{}'?", agent_name), false) {
                            if let Err(e) = save_agent(&org, &team_name, agent_name, &new_content, true) {
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

        // c. Append playbook section
        let playbook_section = build_playbook_section(
            &team_name,
            &team_type,
            &agents_with_names,
            &ctx.name,
        );
        if let Err(e) = append_playbook(&org, &team_name, &playbook_section, &ctx.name, &today_str()) {
            eprintln!("error updating playbook: {}", e);
            return 1;
        }
        println!("+ Playbook updated");

        // d. Update config
        if let Some(mut config) = load_team_config(&org, &team_name) {
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
        // New team

        // Create directories
        let store_dir = team_store_dir(&org, &team_name);
        if let Err(e) = fs::create_dir_all(&store_dir) {
            eprintln!("error creating team directory: {}", e);
            return 1;
        }
        if let Err(e) = fs::create_dir_all(team_agents_dir(&org, &team_name)) {
            eprintln!("error creating agents directory: {}", e);
            return 1;
        }

        // b. Save config
        let now = crate::hooks::common::now_iso();
        let config = TeamConfig {
            name: team_name.clone(),
            org: org.clone(),
            team_type: team_type.clone(),
            projects: vec![ctx.name.clone()],
            created: now.clone(),
            updated: now,
        };
        if let Err(e) = save_team_config(&config) {
            eprintln!("error saving config: {}", e);
            return 1;
        }
        println!("+ Created config.json");

        // c. Save mission
        if let Err(e) = save_mission(&org, &team_name, &mission) {
            eprintln!("error saving mission: {}", e);
            return 1;
        }
        println!("+ Created mission.md");

        // d. Create agents
        for (agent_name, agent_desc) in &agents_with_names {
            let content = build_agent_file(agent_name, agent_desc, &team_name, &team_type);
            if let Err(e) = save_agent(&org, &team_name, agent_name, &content, false) {
                eprintln!("error saving agent '{}': {}", agent_name, e);
                return 1;
            }
            println!("+ Added agent: {}", agent_name);
        }

        // e. Create playbook
        let playbook_content = build_playbook_section(
            &team_name,
            &team_type,
            &agents_with_names,
            &ctx.name,
        );
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

    // 10. Sync phase
    if confirm(&format!("Sync agents to ./.claude/agents/{}/? ", team_name), true) {
        match sync_to_project(&org, &team_name) {
            Ok(count) => println!("✓ Synced {} agent(s) to .claude/agents/{}/", count, team_name),
            Err(e) => {
                eprintln!("error syncing: {}", e);
                return 1;
            }
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

    let org = flags.get("org").cloned().unwrap_or_else(default_org);
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
            eprintln!("Usage: epic team sync <team> [--org <name>]");
            return 1;
        }
    };

    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    if !team_exists(&org, &team) {
        eprintln!("error: team '{}' not found in org '{}'", team, org);
        return 1;
    }

    match sync_to_project(&org, &team) {
        Ok(count) => {
            println!("✓ Synced {} agent(s) to .claude/agents/{}/", count, team);
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
    let team = match pos.first() {
        Some(t) => t.clone(),
        None => {
            eprintln!("error: link requires <team>");
            eprintln!("Usage: epic team link <team> [--org <name>]");
            return 1;
        }
    };

    let org = flags.get("org").cloned().unwrap_or_else(default_org);

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

    if let Some(mut config) = load_team_config(&org, &team) {
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

    println!("Team '{}' linked to project '{}'", team, project_name);
    0
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

    let global = flags.contains_key("global");

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let local_agents_dir = cwd.join(".claude").join("agents").join(&team);

    // Resolve org: --org flag > frontmatter in any local agent file > "epic"
    let org = flags.get("org").cloned().unwrap_or_else(|| {
        // Try reading org from frontmatter of any synced agent file
        if local_agents_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&local_agents_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("md") {
                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            if let Some(org) = read_org_from_agent_file(&content) {
                                return org;
                            }
                        }
                    }
                }
            }
        }
        default_org()
    });

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

    let org = flags.get("org").cloned().unwrap_or_else(default_org);

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
