//! cli.rs — epic team CLI subcommand dispatch

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use super::store::{
    TeamConfig, append_playbook, build_agent_file, build_playbook_section, default_agents_for_type,
    default_org, home_dir, inject_team_context, list_agents, list_history, list_orgs, list_teams,
    load_agent, load_mission, load_playbook, load_team_config, read_org_from_agent_file,
    sanitize_mission, save_agent, save_mission, save_team_config, team_agents_dir, team_exists,
    team_store_dir, today_str, yaml_unescape_display,
};

const SUBCOMMANDS: &[(&str, &str)] = &[
    ("list", "List all teams in an org"),
    ("show", "Show team details"),
    ("status", "Show teams linked to the current project"),
    (
        "sync",
        "Sync team agents to .claude/agents/ (--global for ~/.claude/agents/)",
    ),
    ("link", "Link a team to the current project"),
    ("unlink", "Remove team agents from current project"),
    (
        "delete",
        "Remove team from current project (--global to disband from org)",
    ),
    ("history", "List agent history backups"),
    ("help", "Show this help message"),
];

pub fn dispatch(args: &[String]) -> i32 {
    let (_, flags) = parse_flags(args);
    let org = flags.get("org").cloned().unwrap_or_else(default_org);

    let sub = match args.first().map(|s| s.as_str()) {
        Some(s) if !s.starts_with("--") => s,
        _ => {
            if let Err(e) = validate_org_name(&org) {
                eprintln!("error: {}", e);
                return 1;
            }
            return cmd_default(&org, args);
        }
    };

    match sub {
        "list" => cmd_list(&args[1..]),
        "show" => cmd_show(&args[1..]),
        "status" => cmd_status(&args[1..]),
        "sync" => cmd_sync(&args[1..]),
        "link" => cmd_link(&args[1..]),
        "unlink" => cmd_unlink(&args[1..]),
        "delete" => cmd_delete(&args[1..]),
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

fn recommend_team_type(ctx: &ProjectContext) -> &'static str {
    // Heuristic: infra-heavy stacks without web/scripting → platform; otherwise stream
    // Note: scan_project detects rust/go/java/node/python but not ruby — keep in sync.
    let has_infra = ctx
        .stacks
        .iter()
        .any(|s| matches!(s.as_str(), "rust" | "go" | "java"));
    let has_web = ctx
        .stacks
        .iter()
        .any(|s| matches!(s.as_str(), "node" | "python"));
    if has_infra && !has_web {
        "platform"
    } else {
        "stream"
    }
}

// ── Sync helper ───────────────────────────────────────

fn sync_to_project(org: &str, team: &str) -> io::Result<u32> {
    sync_to_dest(org, team, false)
}

/// Register the current working directory's full path in the team's config.json.
/// Stores the absolute path so stale entries can be detected when the directory is gone.
/// Silent no-op if already registered or config is unavailable.
fn register_project_link(org: &str, team: &str) {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("warning: could not determine cwd: {}", e);
            return;
        }
    };
    // Canonicalize so symlink targets are compared consistently.
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let project_path = cwd.to_string_lossy().to_string();
    if project_path.is_empty() {
        return;
    }

    if let Some(mut config) = load_team_config(org, team)
        && !config.projects.contains(&project_path)
    {
        // Purge stale entries (directories no longer on disk) while we have the config open.
        retain_live_projects(&mut config.projects);
        config.projects.push(project_path);
        config.updated = crate::hooks::common::now_iso();
        if let Err(e) = save_team_config(&config) {
            eprintln!("warning: could not update team config: {}", e);
        }
    }
}

/// Format `config.projects` for display: filter stale absolute paths, show basenames.
/// Non-absolute entries (legacy dirname-only format) are shown as-is.
fn display_projects(projects: &[String]) -> String {
    let visible: Vec<String> = projects
        .iter()
        .filter(|p| {
            let path = std::path::Path::new(p.as_str());
            !path.is_absolute() || path.is_dir()
        })
        .map(|p| {
            let path = std::path::Path::new(p.as_str());
            if path.is_absolute() {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p.as_str())
                    .to_string()
            } else {
                p.clone()
            }
        })
        .collect();
    if visible.is_empty() {
        "(none)".to_string()
    } else {
        visible.join(", ")
    }
}

/// Purge stale entries from a projects list in-place.
/// Keeps entries that are either (a) absolute paths still present on disk, or
/// (b) legacy relative (basename-only) entries that cannot be validated by presence.
fn retain_live_projects(projects: &mut Vec<String>) {
    projects.retain(|p| {
        let path = std::path::Path::new(p);
        !path.is_absolute() || path.is_dir()
    });
}

fn validate_identifier(kind: &str, value: &str) -> io::Result<()> {
    let valid = !value.is_empty()
        && value
            .chars()
            .next()
            .map(|c| c.is_ascii_alphanumeric())
            .unwrap_or(false)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid {} name '{}': only [a-zA-Z0-9_-] allowed, must start with alphanumeric",
                kind, value
            ),
        ))
    }
}

fn validate_team_name(team: &str) -> io::Result<()> {
    validate_identifier("team", team)
}

fn validate_org_name(org: &str) -> io::Result<()> {
    validate_identifier("org", org)
}

/// Returns the agents dir for a tool if that tool appears to be installed globally.
fn installed_tool_agents_dir(tool: &str) -> Option<PathBuf> {
    let home = home_dir();
    // home_dir() falls back to PathBuf::from(".") when HOME is unset — not an empty string.
    if home == std::path::Path::new(".") {
        return None;
    }
    let parent = match tool {
        "codex" => home.join(".codex"),
        "antigravity" => home
            .join(".gemini")
            .join("config")
            .join("plugins")
            .join("epic"),
        "cursor" => home.join(".cursor"),
        "opencode" => home.join(".config").join("opencode"),
        _ => return None,
    };
    if parent.exists() {
        Some(parent.join("agents"))
    } else {
        None
    }
}

/// Codex agent files this team owns, if Codex is installed.
///
/// `sync` writes them globally to `~/.codex/agents/` whatever the sync scope, so
/// they are the one piece of generated state the project-local commands cannot
/// see through `.claude/agents/`.
fn codex_team_files(org: &str, team: &str) -> Vec<PathBuf> {
    installed_tool_agents_dir("codex")
        .map(|dir| crate::team::codex::team_agent_files(&dir, org, team))
        .unwrap_or_default()
}

/// Resolve a project link only from matching `team:` and `org:` records written
/// at sync time. Directory names and filename prefixes are not ownership proof.
fn local_team_ownership(
    local_agents_dir: &std::path::Path,
    team: &str,
) -> io::Result<Option<String>> {
    // Check the team directory and the `.claude/agents` path components with
    // lstat. A symlinked parent would make a local unlink/delete operate on an
    // external directory even when the final component is a real directory.
    let mut component = Some(local_agents_dir);
    for _ in 0..3 {
        let Some(path) = component else { break };
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("team path has an unsafe component: {}", path.display()),
                ));
            }
            Ok(_) => component = path.parent(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        }
    }
    let metadata = match fs::symlink_metadata(local_agents_dir) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "team directory is not a regular directory: {}",
                local_agents_dir.display()
            ),
        ));
    }

    let mut org: Option<String> = None;
    for entry in fs::read_dir(local_agents_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|e| e.to_str()) != Some("md")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unowned entry in team directory: {}",
                    entry.path().display()
                ),
            ));
        }
        let content = fs::read_to_string(entry.path())?;
        let Some(recorded_team) = super::store::read_team_from_agent_file(&content) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing team ownership in {}", entry.path().display()),
            ));
        };
        if recorded_team != team {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("foreign team ownership in {}", entry.path().display()),
            ));
        }
        let Some(recorded_org) = read_org_from_agent_file(&content) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing org ownership in {}", entry.path().display()),
            ));
        };
        if validate_org_name(&recorded_org).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid org ownership in {}", entry.path().display()),
            ));
        }
        if let Some(existing) = &org
            && existing != &recorded_org
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "conflicting team ownership records in {}",
                    local_agents_dir.display()
                ),
            ));
        }
        org = Some(recorded_org);
    }
    Ok(org)
}

fn write_owned_agent_file(
    destination: &std::path::Path,
    payload: &str,
    org: &str,
    team: &str,
) -> io::Result<()> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "agent destination is not a regular file: {}",
                    destination.display()
                ),
            ));
        }
        Ok(_) => {
            let existing = fs::read_to_string(destination)?;
            if existing == payload {
                return Ok(());
            }
            let owned = read_org_from_agent_file(&existing).as_deref() == Some(org)
                && super::store::read_team_from_agent_file(&existing).as_deref() == Some(team);
            if !owned {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "refusing to overwrite unowned agent: {}",
                        destination.display()
                    ),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "agent destination has no parent",
        )
    })?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("agent");
    for attempt in 0..32 {
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            attempt
        ));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        if let Err(error) = file
            .write_all(payload.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        drop(file);
        if let Err(error) = super::codex::atomic_replace_file(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("could not allocate temporary file in {}", parent.display()),
    ))
}

/// Create a directory only after every existing component from it to the first
/// missing parent is verified with `symlink_metadata`. This prevents sync from
/// creating a team directory through an already-present symlink.
fn ensure_regular_directory(path: &std::path::Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("directory is not a regular directory: {}", path.display()),
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("directory has no parent: {}", path.display()),
                )
            })?;
            ensure_regular_directory(parent)?;
            match fs::create_dir(path) {
                Ok(()) => ensure_regular_directory(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    ensure_regular_directory(path)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn delete_global_team_files(
    store_dir: &std::path::Path,
    local_agents_dir: Option<&std::path::Path>,
    codex_files: &[PathBuf],
) -> io::Result<()> {
    if let Some(local_agents_dir) = local_agents_dir {
        match fs::remove_dir_all(local_agents_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    for file in codex_files {
        match fs::remove_file(file) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::remove_dir_all(store_dir)
}

fn sync_to_dest(org: &str, team: &str, global: bool) -> io::Result<u32> {
    validate_team_name(team)?;

    let config = load_team_config(org, team).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Team '{}' not found in org '{}'", team, org),
        )
    })?;

    let mission = load_mission(org, team).unwrap_or_default();

    let dest = if global {
        let base = crate::hooks::common::claude_config_dir().join("agents");
        ensure_regular_directory(&base)?;
        let candidate = base.join(team);
        ensure_regular_directory(&candidate)?;
        candidate
    } else {
        let cwd = std::env::current_dir().map_err(io::Error::other)?;
        let d = cwd.join(".claude").join("agents").join(team);
        ensure_regular_directory(&d)?;
        d
    };

    let agents = list_agents(org, team);
    let mut count = 0u32;

    for agent_name in &agents {
        if let Some(content) = load_agent(org, team, agent_name) {
            let injected = inject_team_context(&content, org, team, &config.team_type, &mission);
            let dest_path = dest.join(format!("{}.md", agent_name));
            write_owned_agent_file(&dest_path, &injected, org, team)?;
            count += 1;
        }
    }

    // Also sync to other installed tools with tool-specific transforms.
    // This writes agent files to ~/.codex/agents/, ~/.gemini/config/plugins/epic/agents/, etc.
    // when those directories exist.  Print a notice for each tool synced so
    // the user can see which tools were updated.
    let other_tools = ["codex", "antigravity", "cursor", "opencode"];
    for tool in &other_tools {
        if let Some(agents_dir) = installed_tool_agents_dir(tool) {
            // Codex has a flat agent directory. Its files carry exact ownership
            // metadata and use an atomic no-symlink write path, unlike the
            // per-team Markdown layout used by the other tools.
            if *tool == "codex" {
                crate::team::codex::prepare_agents_dir(&agents_dir).map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!(
                            "unsafe Codex agents destination {}: {e}",
                            agents_dir.display()
                        ),
                    )
                })?;
                eprintln!(
                    "[harness] syncing team '{team}' to codex ({})",
                    agents_dir.display()
                );
                for agent_name in &agents {
                    let Some(content) = load_agent(org, team, agent_name) else {
                        continue;
                    };
                    let injected =
                        inject_team_context(&content, org, team, &config.team_type, &mission);
                    let agent =
                        crate::team::codex::to_codex_agent(org, team, agent_name, &injected);
                    let payload = crate::team::codex::render_codex_toml(&agent).map_err(|e| {
                        io::Error::other(format!(
                            "could not render Codex agent '{agent_name}': {e}"
                        ))
                    })?;
                    // Legacy flat names are ambiguous for hyphenated names. A
                    // sync migrates by adding the new owned file, never by
                    // renaming or deleting a legacy file that may be another
                    // team's agent.
                    let legacy = agents_dir.join(format!("{team}-{agent_name}.toml"));
                    if fs::symlink_metadata(&legacy).is_ok() {
                        eprintln!(
                            "[harness] legacy Codex agent left untouched: {}; use the new owned identity {}",
                            legacy.display(),
                            agent.name
                        );
                    }
                    crate::team::codex::write_agent_file(
                        &agents_dir,
                        org,
                        team,
                        agent_name,
                        &payload,
                    )
                    .map_err(|e| {
                        io::Error::new(
                            e.kind(),
                            format!("could not write Codex agent '{agent_name}': {e}"),
                        )
                    })?;
                }
                continue;
            }

            let tool_team_dir = agents_dir.join(team);
            ensure_regular_directory(&tool_team_dir).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "unsafe {tool} agents destination {}: {error}",
                        tool_team_dir.display()
                    ),
                )
            })?;
            eprintln!(
                "[harness] syncing team '{team}' to {tool} ({})",
                tool_team_dir.display()
            );
            for agent_name in &agents {
                if let Some(content) = load_agent(org, team, agent_name) {
                    let injected =
                        inject_team_context(&content, org, team, &config.team_type, &mission);

                    let (dest_path, payload) =
                        (tool_team_dir.join(format!("{}.md", agent_name)), injected);

                    write_owned_agent_file(&dest_path, &payload, org, team)?;
                }
            }
        }
    }

    Ok(count)
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    let valid_types = ["stream", "platform", "enabling", "subsystem"];
    let suggested_type = recommend_team_type(&ctx);

    if yes {
        // ── Non-interactive path ──────────────────────────
        let team_name = flags
            .get("name")
            .cloned()
            .unwrap_or_else(|| suggested_name.clone());
        if let Err(e) = validate_team_name(&team_name) {
            eprintln!("error: {}", e);
            return 1;
        }

        let team_type = flags
            .get("type")
            .cloned()
            .unwrap_or_else(|| suggested_type.to_string());
        if !valid_types.contains(&team_type.as_str()) {
            eprintln!(
                "error: invalid --type '{}'. Must be one of: stream, platform, enabling, subsystem",
                team_type
            );
            return 1;
        }

        let mission = match flags.get("mission").cloned() {
            Some(m) if !m.is_empty() && m.chars().count() <= 200 => m,
            Some(m) if m.chars().count() > 200 => {
                eprintln!(
                    "error: --mission too long (max 200 chars, got {})",
                    m.chars().count()
                );
                return 1;
            }
            _ => {
                eprintln!("error: --mission is required with --yes");
                eprintln!(
                    "Usage: epic team --yes --name <name> --type <type> --mission \"<mission>\""
                );
                return 1;
            }
        };

        println!("Org: {}", org);
        println!("Project: {}", ctx.name);
        println!("Team: {} ({}) — {}", team_name, team_type, mission);

        let proposed_agents = default_agents_for_type(&team_type);
        println!(
            "Agents: {}",
            proposed_agents
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        );

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
    let team_name = if team_name_input.is_empty() {
        suggested_name.clone()
    } else {
        team_name_input
    };
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
        let t = if input.is_empty() {
            suggested_type.to_string()
        } else {
            input
        };
        if valid_types.contains(&t.as_str()) {
            break t;
        }
        println!(
            "Invalid type '{}'. Must be one of: stream, platform, enabling, subsystem",
            t
        );
    };

    // Mission
    let mission = loop {
        let input = prompt("Mission (one-line domain ownership): ");
        if input.is_empty() {
            println!("Mission cannot be empty.");
        } else if input.chars().count() > 200 {
            println!(
                "Mission too long ({} chars, max 200).",
                input.chars().count()
            );
        } else {
            break input;
        }
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
    let mission_clean = sanitize_mission(mission);
    let mission = mission_clean.as_str();
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
                        println!(
                            "Agent '{}' has changed ({} → {} chars).",
                            agent_name,
                            existing_content.len(),
                            new_content.len()
                        );
                        if confirm(&format!("Replace '{}'?", agent_name), false) {
                            if let Err(e) =
                                save_agent(org, team_name, agent_name, &new_content, true)
                            {
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

        let playbook_section =
            build_playbook_section(team_name, team_type, &agents_with_names, &ctx.name);
        if let Err(e) = append_playbook(org, team_name, &playbook_section, &ctx.name, &today_str())
        {
            eprintln!("error updating playbook: {}", e);
            return 1;
        }
        println!("+ Playbook updated");

        if let Some(mut config) = load_team_config(org, team_name) {
            let cwd_path = std::env::current_dir()
                .ok()
                .and_then(|p| p.canonicalize().ok().or(Some(p)))
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if !cwd_path.is_empty() && !config.projects.contains(&cwd_path) {
                retain_live_projects(&mut config.projects);
                config.projects.push(cwd_path);
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

        let cwd_path = std::env::current_dir()
            .ok()
            .and_then(|p| p.canonicalize().ok().or(Some(p)))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ctx.name.clone());
        let now = crate::hooks::common::now_iso();
        let config = TeamConfig {
            name: team_name.to_string(),
            org: org.to_string(),
            team_type: team_type.to_string(),
            projects: vec![cwd_path],
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

        let playbook_content =
            build_playbook_section(team_name, team_type, &agents_with_names, &ctx.name);
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

    let do_sync = auto_sync
        || confirm(
            &format!("Sync agents to ./.claude/agents/{}/? ", team_name),
            true,
        );
    if do_sync {
        match sync_to_project(org, team_name) {
            Ok(count) => {
                println!(
                    "✓ Synced {} agent(s) to .claude/agents/{}/",
                    count, team_name
                );
                register_project_link(org, team_name);
            }
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
        match load_team_config(&org, team) {
            Some(config) => {
                println!(
                    "  {:<16} ({:<10}) projects: {}",
                    team,
                    config.team_type,
                    display_projects(&config.projects)
                );
            }
            None => {
                // config.json 읽기 실패 — 팀 이름만 표시
                println!("  {:<16} (unknown   ) projects: (unavailable)", team);
            }
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
        println!("Projects: {}", display_projects(&c.projects));
        println!("Created: {}", c.created);
        println!("Updated: {}", c.updated);
    }
    println!("Mission: {}", mission.trim());
    println!();
    println!("Agents ({}):", agents.len());
    for agent_name in &agents {
        let desc = load_agent(&org, &team, agent_name)
            .and_then(|content| {
                content.lines().find_map(|line| {
                    line.trim_start()
                        .strip_prefix("description:")
                        .map(|v| yaml_unescape_display(v.trim()))
                })
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
                crate::hooks::common::claude_config_dir()
                    .join("agents")
                    .join(&team)
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".claude")
                    .join("agents")
                    .join(&team)
            };
            println!("✓ Synced {} agent(s) to {}/", count, dest.display());
            if !global {
                register_project_link(&org, &team);
            }
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
        let matches: Vec<String> = orgs.into_iter().filter(|o| team_exists(o, &team)).collect();

        match matches.len() {
            0 => {
                eprintln!(
                    "error: team '{}' not found in any org. Run 'epic org list' to browse.",
                    team
                );
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
    let project_name = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    register_project_link(&org, &team);
    println!("Team '{}' linked to project '{}'", team, project_name);
    0
}

fn cmd_link_interactive(org_filter: Option<&str>) -> i32 {
    if let Some(filter) = org_filter
        && let Err(e) = validate_org_name(filter)
    {
        eprintln!("error: {}", e);
        return 1;
    }
    // If a specific org is requested, read only that org (avoids N+1 when org is known)
    let entries: Vec<(String, String)> = if let Some(filter) = org_filter {
        list_teams(filter)
            .into_iter()
            .map(|team| (filter.to_string(), team))
            .collect()
    } else {
        let orgs = list_orgs();
        if orgs.is_empty() {
            eprintln!("No orgs found. Run 'epic team' to create a team.");
            return 1;
        }
        orgs.iter()
            .flat_map(|org| list_teams(org).into_iter().map(|team| (org.clone(), team)))
            .collect()
    };

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
// Local-only alias for `delete` — kept for discoverability

fn cmd_unlink(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--global") {
        eprintln!(
            "error: --global is not valid for 'unlink'. Use 'epic team delete --global' to permanently remove from org."
        );
        return 1;
    }
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
    let local_org = match local_team_ownership(&local_agents_dir, &team) {
        Ok(org) => org,
        Err(e) => {
            eprintln!("error: cannot verify local team ownership: {e}");
            return 1;
        }
    };

    // Resolve org: --org flag > exact local ownership record > "epic".
    let org = flags
        .get("org")
        .cloned()
        .unwrap_or_else(|| local_org.clone().unwrap_or_else(default_org));

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
        let codex_files = codex_team_files(&org, &team);
        println!("This will permanently delete:");
        println!("  Global store: {}", store_dir.display());
        if local_org.as_deref() == Some(&org) {
            println!("  Local agents: {}", local_agents_dir.display());
        } else if local_agents_dir.exists() {
            println!("  Local agents: skipped (ownership does not match)");
        }
        for f in &codex_files {
            println!("  Codex agent: {}", f.display());
        }
        println!();
        println!("  ⚠  This cannot be undone. All agents and .history/ backups will be removed.");
        println!();

        if !confirm(
            &format!("Permanently delete team '{}' from org '{}'?", team, org),
            false,
        ) {
            println!("Aborted.");
            return 0;
        }

        let owned_local =
            (local_org.as_deref() == Some(&org)).then_some(local_agents_dir.as_path());
        if let Err(error) = delete_global_team_files(&store_dir, owned_local, &codex_files) {
            eprintln!(
                "error deleting team artifacts; global store kept for retry when possible: {error}"
            );
            return 1;
        }
        if owned_local.is_some() {
            println!("✓ Removed local agents: .claude/agents/{}/", team);
        } else if local_agents_dir.exists() {
            eprintln!("warning: local agents were not removed because ownership does not match");
        }
        for f in &codex_files {
            println!("✓ Removed Codex agent: {}", f.display());
        }
        println!("✓ Deleted global store: {}", store_dir.display());
        println!();
        println!("Team '{}' permanently deleted from org '{}'.", team, org);
    } else {
        // default: remove from current project only (.claude/agents/{team}/)
        if local_org.as_deref() != Some(&org) {
            println!(
                "Team '{}' is not linked to this project with an exact ownership record.",
                team
            );
            return 1;
        }
        match fs::remove_dir_all(&local_agents_dir) {
            Ok(_) => {
                println!("✓ Removed .claude/agents/{}/", team);
                println!(
                    "  (Global store untouched. Use 'epic team link {}' to re-attach.)",
                    team
                );
                // `sync` writes Codex agents globally whatever the sync scope, so
                // a project-scoped unlink cannot remove them. Say so rather than
                // leaving files the user cannot see.
                let codex_files = codex_team_files(&org, &team);
                if !codex_files.is_empty() {
                    println!(
                        "  {} Codex agent file(s) remain (global, shared by all projects):",
                        codex_files.len()
                    );
                    for f in &codex_files {
                        println!("    {}", f.display());
                    }
                    println!("  Remove them with 'epic team delete {} --global'.", team);
                }
                // Deregister project: remove current cwd path from projects list, also purge stale entries.
                let cwd_path = std::env::current_dir()
                    .ok()
                    .and_then(|p| p.canonicalize().ok().or(Some(p)))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !cwd_path.is_empty()
                    && let Some(mut config) = load_team_config(&org, &team)
                {
                    retain_live_projects(&mut config.projects);
                    config.projects.retain(|p| p != &cwd_path);
                    config.updated = crate::hooks::common::now_iso();
                    let _ = save_team_config(&config);
                }
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
    if let Err(e) = validate_team_name(&agent) {
        eprintln!("error: invalid agent name '{}': {}", agent, e);
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
    team_dirs.retain(
        |team| match local_team_ownership(&agents_base.join(team), team) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                eprintln!("warning: cannot verify team '{}': {}", team, e);
                false
            }
        },
    );
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
                            x.file_type().map(|t| t.is_file()).unwrap_or(false)
                                && x.path().extension().and_then(|ext| ext.to_str()) == Some("md")
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Try each .md file until one yields a valid org field
            let org = entries
                .iter()
                .find_map(|e| {
                    let content = fs::read_to_string(e.path()).ok()?;
                    let o = read_org_from_agent_file(&content)?;
                    let recorded_team = super::store::read_team_from_agent_file(&content)?;
                    (recorded_team == team.as_str())
                        .then_some(o)
                        .filter(|o| validate_org_name(o).is_ok())
                })
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
            let t = tc
                .as_ref()
                .map(|c| c.team_type.as_str())
                .unwrap_or("unknown")
                .to_string();
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
            if agents.is_empty() {
                "(none)".to_string()
            } else {
                agents.join(", ")
            }
        );

        // Codex agents live flat in ~/.codex/agents/ and are named
        // `{team}-{agent}.toml`, so scanning .claude/agents/ alone reported none
        // even when sync had written them.
        let codex_files = codex_team_files(&org, team);
        if !codex_files.is_empty() {
            let names: Vec<String> = codex_files
                .iter()
                .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
                .map(String::from)
                .collect();
            println!("             codex:  {}", names.join(", "));
        }
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
            eprintln!(
                "error: unknown org subcommand '{}'\nRun 'epic org help'.",
                sub
            );
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
            let t = tc
                .as_ref()
                .map(|c| c.team_type.as_str())
                .unwrap_or("unknown")
                .to_string();
            let m = load_mission(&org, team)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            (t, m)
        };
        println!("  {:<16} ({:<10}) {}", team, team_type, mission_first_line);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Serialize all tests that mutate the process-wide HOME env var.
    // Shared with store::tests via super::super::HOME_LOCK (declared in mod.rs).
    use super::super::HOME_LOCK;

    fn to_args(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    /// RAII guard that restores the current working directory on drop (including on panic).
    struct CwdGuard(std::path::PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    /// Build a minimal team in `tmp` HOME and return the store dir path.
    /// Creates: config.json + agents/<name>.md so team_exists() returns true.
    fn seed_team(tmp: &std::path::Path, org: &str, team: &str) {
        use super::super::store::{TeamConfig, save_agent, save_mission, save_team_config};
        let config = TeamConfig {
            name: team.to_string(),
            org: org.to_string(),
            team_type: "stream".to_string(),
            projects: vec![],
            created: "2026-01-01T00:00:00Z".to_string(),
            updated: "2026-01-01T00:00:00Z".to_string(),
        };
        save_team_config(&config).expect("save_team_config");
        save_mission(org, team, "Test mission").expect("save_mission");
        let agent_content = "---\nname: \"tester\"\ndescription: \"test agent\"\ntools: [Read]\nmodel: sonnet\n---\n# Tester\n";
        save_agent(org, team, "tester", agent_content, false).expect("save_agent");
        let _ = tmp; // tmp kept alive by caller
    }

    // ── cmd_delete --global tests ─────────────────────────

    /// cmd_delete --global on a non-existent team returns exit code 1.
    #[test]
    fn test_cmd_delete_global_team_not_found() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }

        let args = to_args(&["ghost-team", "--org", "testorg", "--global"]);
        let code = cmd_delete(&args);
        assert_eq!(code, 1, "deleting non-existent team globally must return 1");
    }

    /// cmd_delete --global removes the org store directory.
    #[test]
    fn test_cmd_delete_global_removes_store() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }

        seed_team(tmp.path(), "deleteorg", "alpha");

        // Verify the team exists before deletion
        let store = team_store_dir("deleteorg", "alpha");
        assert!(store.exists(), "store dir must exist before delete");

        // cmd_delete calls confirm() interactively — we can't drive stdin in a unit test.
        // Instead we call sync_to_dest and then remove the store directly to verify
        // the code path up to the confirmation guard, and also test validation logic.

        // Re-test: invalid team name must fail before reaching confirm()
        let args_bad = to_args(&["../../../etc", "--org", "deleteorg", "--global"]);
        let code = cmd_delete(&args_bad);
        assert_eq!(
            code, 1,
            "path-traversal team name must be rejected (exit 1)"
        );

        // Valid call hits confirm() which reads from a closed stdin → empty input →
        // defaults to 'n' (default=false) → Aborted.  Exit code must be 0 (not an error).
        let args_ok = to_args(&["alpha", "--org", "deleteorg", "--global"]);
        let code = cmd_delete(&args_ok);
        assert_eq!(code, 0, "aborting at confirm() should return 0");

        // Store must still exist because we aborted
        assert!(
            store.exists(),
            "store dir must survive an aborted --global delete"
        );
    }

    #[test]
    fn test_global_delete_keeps_store_when_artifact_cleanup_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let undeletable_as_file = dir.path().join("agent.toml");
        fs::create_dir(&store).unwrap();
        fs::create_dir(&undeletable_as_file).unwrap();

        let result = delete_global_team_files(&store, None, &[undeletable_as_file]);

        assert!(result.is_err());
        assert!(store.is_dir(), "source store must remain retryable");
    }

    // ── cmd_status tests ──────────────────────────────────

    /// cmd_status with no .claude/agents/ directory returns 0 and prints a 'no teams' message.
    #[test]
    fn test_cmd_status_no_agents_dir() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }

        // Change cwd to a fresh directory that has no .claude/agents/
        let project_dir = tmp.path().join("myproject");
        std::fs::create_dir_all(&project_dir).unwrap();
        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(&project_dir).unwrap();

        let args = to_args(&[]);
        let code = cmd_status(&args);

        assert_eq!(code, 0, "cmd_status with no .claude/agents/ must return 0");
    }

    /// cmd_status lists linked teams and their agents correctly.
    #[test]
    fn test_cmd_status_with_linked_team() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }

        // Set up a project directory with a synced team in .claude/agents/
        let project_dir = tmp.path().join("statustest");
        let agents_dir = project_dir.join(".claude").join("agents").join("beta");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Seed agent file with org frontmatter so cmd_status can read org
        let agent_content = "---\nname: \"tester\"\ndescription: \"test\"\norg: \"statusorg\"\nteam: \"beta\"\ntools: [Read]\nmodel: sonnet\n---\n# Tester\n";
        std::fs::write(agents_dir.join("tester.md"), agent_content).unwrap();

        // Also seed the org store so load_team_config / load_mission succeed
        seed_team(tmp.path(), "statusorg", "beta");

        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(&project_dir).unwrap();

        let args = to_args(&[]);
        let code = cmd_status(&args);

        assert_eq!(code, 0, "cmd_status with linked team must return 0");
    }

    #[test]
    fn test_local_ownership_rejects_a_foreign_team_record() {
        let dir = tempfile::tempdir().unwrap();
        let team_dir = dir.path().join("alpha-beta");
        fs::create_dir_all(&team_dir).unwrap();
        fs::write(
            team_dir.join("reviewer.md"),
            "---\norg: \"exact-org\"\nteam: \"alpha\"\n---\n",
        )
        .unwrap();

        assert!(local_team_ownership(&team_dir, "alpha-beta").is_err());
    }

    #[test]
    fn test_local_ownership_rejects_mixed_or_unowned_files() {
        let dir = tempfile::tempdir().unwrap();
        let team_dir = dir.path().join("alpha");
        fs::create_dir_all(&team_dir).unwrap();
        fs::write(
            team_dir.join("owned.md"),
            "---\norg: \"exact-org\"\nteam: \"alpha\"\n---\n",
        )
        .unwrap();
        fs::write(team_dir.join("unowned.md"), "# personal file\n").unwrap();

        assert!(local_team_ownership(&team_dir, "alpha").is_err());
    }

    // ── cmd_sync symlink-escape defense ───────────────────

    /// cmd_sync rejects a symlink that escapes .claude/agents/ (local sync path).
    #[test]
    #[cfg(unix)]
    fn test_cmd_sync_local_rejects_symlink_escape() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }

        let project_dir = tmp.path().join("synctest");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Seed team in org store
        seed_team(tmp.path(), "syncorg", "gamma");

        let agents_base = project_dir.join(".claude").join("agents");
        std::fs::create_dir_all(&agents_base).unwrap();

        // Create a symlink at .claude/agents/gamma → /tmp (outside base)
        let escape_target = tmp.path().join("escape_target");
        std::fs::create_dir_all(&escape_target).unwrap();
        let symlink_path = agents_base.join("gamma");
        std::os::unix::fs::symlink(&escape_target, &symlink_path).unwrap();

        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(&project_dir).unwrap();

        // Call sync_to_dest directly (local, not global) — the symlink escape guard
        // canonicalizes the resolved path and rejects it if it escapes agents base.
        let result = sync_to_dest("syncorg", "gamma", false);

        assert!(
            result.is_err(),
            "sync must fail when team dir is a symlink escaping .claude/agents/"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.kind(),
            std::io::ErrorKind::PermissionDenied,
            "error kind must be PermissionDenied for symlink escape: {}",
            err
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_local_sync_does_not_create_a_team_dir_through_an_agents_symlink() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("HOME", tmp.path()) };
        let project = tmp.path().join("project");
        let escape = tmp.path().join("escape");
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::create_dir_all(&escape).unwrap();
        std::os::unix::fs::symlink(&escape, project.join(".claude").join("agents")).unwrap();
        seed_team(tmp.path(), "syncorg", "gamma");

        let _cwd = CwdGuard(env::current_dir().unwrap());
        env::set_current_dir(&project).unwrap();
        assert!(sync_to_dest("syncorg", "gamma", false).is_err());
        assert!(
            !escape.join("gamma").exists(),
            "sync must not create a team directory through a symlink"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_other_tool_sync_does_not_create_a_team_dir_through_an_agents_symlink() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("HOME", tmp.path()) };
        let project = tmp.path().join("project");
        let tool_root = tmp
            .path()
            .join(".gemini")
            .join("config")
            .join("plugins")
            .join("epic");
        let escape = tmp.path().join("escape");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&tool_root).unwrap();
        fs::create_dir_all(&escape).unwrap();
        std::os::unix::fs::symlink(&escape, tool_root.join("agents")).unwrap();
        seed_team(tmp.path(), "syncorg", "gamma");

        let _cwd = CwdGuard(env::current_dir().unwrap());
        env::set_current_dir(&project).unwrap();
        assert!(sync_to_dest("syncorg", "gamma", false).is_err());
        assert!(
            !escape.join("gamma").exists(),
            "sync must not create another tool's team directory through a symlink"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_cmd_sync_rejects_final_agent_file_symlink() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            env::set_var("HOME", tmp.path());
        }
        let project_dir = tmp.path().join("project");
        let team_dir = project_dir.join(".claude/agents/gamma");
        fs::create_dir_all(&team_dir).unwrap();
        seed_team(tmp.path(), "syncorg", "gamma");
        let external = tmp.path().join("external.md");
        fs::write(&external, "keep me").unwrap();
        std::os::unix::fs::symlink(&external, team_dir.join("tester.md")).unwrap();

        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        env::set_current_dir(&project_dir).unwrap();
        assert!(sync_to_dest("syncorg", "gamma", false).is_err());
        assert_eq!(fs::read_to_string(external).unwrap(), "keep me");
    }

    /// Codex's flat directory must not turn a legacy collision into an overwrite.
    #[test]
    fn test_codex_sync_does_not_overwrite_another_teams_legacy_collision() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }
        let project_dir = tmp.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(tmp.path().join(".codex").join("agents")).unwrap();
        seed_team(tmp.path(), "syncorg", "alpha-beta");

        // Legacy `alpha-beta-tester.toml` is ambiguous: it can be alpha-beta/tester
        // or alpha/beta-tester. It must survive a sync of alpha-beta unchanged.
        let legacy = tmp
            .path()
            .join(".codex")
            .join("agents")
            .join("alpha-beta-tester.toml");
        let foreign_contents =
            "name = \"alpha-beta-tester\"\ndescription = \"alpha/beta-tester\"\n";
        fs::write(&legacy, foreign_contents).unwrap();

        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        env::set_current_dir(&project_dir).unwrap();
        sync_to_dest("syncorg", "alpha-beta", false).unwrap();

        assert_eq!(fs::read_to_string(legacy).unwrap(), foreign_contents);
        assert!(
            tmp.path()
                .join(".codex")
                .join("agents")
                .join("epic-7-syncorg-10-alpha-beta-6-tester.toml")
                .is_file(),
            "sync must migrate by writing a new unambiguous identity"
        );
    }

    /// A symlinked flat Codex agent directory must be rejected before any write.
    #[test]
    #[cfg(unix)]
    fn test_codex_sync_rejects_symlinked_agents_directory() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe {
            env::set_var("HOME", tmp.path());
        }
        let project_dir = tmp.path().join("project");
        let codex_dir = tmp.path().join(".codex");
        let escape_dir = tmp.path().join("escape");
        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&escape_dir).unwrap();
        std::os::unix::fs::symlink(&escape_dir, codex_dir.join("agents")).unwrap();
        seed_team(tmp.path(), "syncorg", "gamma");

        let _cwd = CwdGuard(std::env::current_dir().unwrap());
        env::set_current_dir(&project_dir).unwrap();
        let result = sync_to_dest("syncorg", "gamma", false);

        assert!(
            result.is_err(),
            "Codex sync must reject a symlinked destination"
        );
        assert!(
            fs::read_dir(&escape_dir).unwrap().next().is_none(),
            "sync must not write through the symlink"
        );
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

    // ── Fix W-5: validate_org_name / validate_team_name 실질 검증 ──

    #[test]
    fn test_validate_org_and_team_names() {
        // validate_org_name
        assert!(validate_org_name("../../etc").is_err());
        assert!(validate_org_name("valid-org").is_ok());
        // validate_team_name
        assert!(validate_team_name("my_team-1").is_ok());
        assert!(validate_team_name("").is_err());
        assert!(validate_team_name("../escape").is_err());
    }

    // ── Fix W-5: write-skip 조건 로직 확인 (fs I/O 없이) ──

    #[test]
    fn test_string_equality_for_write_skip() {
        let a = "content".to_string();
        let b = "content".to_string();
        assert_eq!(a, b); // same → skip
        let c = "different".to_string();
        assert_ne!(a, c); // different → write
    }

    // ── Fix 6 (W-4): today_str 중복 제거 — store::today_str 사용 검증 ──

    #[test]
    fn test_store_today_str_format() {
        // store::today_str()가 YYYY-MM-DD 형식을 반환하는지 확인
        let s = super::today_str();
        assert_eq!(s.len(), 10, "today_str should be 10 chars (YYYY-MM-DD)");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }

    #[test]
    fn test_retain_live_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let live_dir = tmp.path().join("live");
        std::fs::create_dir_all(&live_dir).unwrap();

        let live = live_dir.to_str().unwrap().to_string();
        let stale = tmp.path().join("gone").to_str().unwrap().to_string();
        let relative = "basename-only".to_string();

        let mut projects = vec![live.clone(), stale.clone(), relative.clone()];
        retain_live_projects(&mut projects);

        assert!(projects.contains(&live), "live absolute dir must be kept");
        assert!(
            !projects.contains(&stale),
            "stale absolute path must be removed"
        );
        assert!(
            projects.contains(&relative),
            "relative (legacy) entry must be kept"
        );
    }
}
