//! store.rs — Team data storage and retrieval

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

// ── Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    pub name: String,
    pub org: String,
    #[serde(rename = "type")]
    pub team_type: String,
    pub projects: Vec<String>,
    pub created: String,
    pub updated: String,
}

// ── Private helpers ────────────────────────────────────

fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        return PathBuf::from(up);
    }
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return PathBuf::from(format!("{}{}", drive, path));
    }
    panic!("[harness] FATAL: Home directory not detected. Please set HOME or USERPROFILE.");
}

fn to_title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn today_str() -> String {
    let iso = crate::hooks::common::now_iso();
    iso[..10].to_string()
}

// ── Path helpers ───────────────────────────────────────

pub fn orgs_base_dir() -> PathBuf {
    home_dir().join(".harness").join("orgs")
}

pub fn org_dir(org: &str) -> PathBuf {
    orgs_base_dir().join(org)
}

pub fn team_store_dir(org: &str, team: &str) -> PathBuf {
    org_dir(org).join("teams").join(team)
}

pub fn team_agents_dir(org: &str, team: &str) -> PathBuf {
    team_store_dir(org, team).join("agents")
}

pub fn team_history_dir(org: &str, team: &str) -> PathBuf {
    team_store_dir(org, team).join(".history")
}

// ── CRUD ──────────────────────────────────────────────

#[allow(dead_code)]
pub fn list_orgs() -> Vec<String> {
    let base = orgs_base_dir();
    if !base.exists() {
        return vec![];
    }
    let mut orgs: Vec<String> = fs::read_dir(&base)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    orgs.sort();
    orgs
}

pub fn list_teams(org: &str) -> Vec<String> {
    let teams_dir = org_dir(org).join("teams");
    if !teams_dir.exists() {
        return vec![];
    }
    let mut teams: Vec<String> = fs::read_dir(&teams_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    teams.sort();
    teams
}

pub fn team_exists(org: &str, team: &str) -> bool {
    team_store_dir(org, team).is_dir()
}

pub fn load_team_config(org: &str, team: &str) -> Option<TeamConfig> {
    let path = team_store_dir(org, team).join("config.json");
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_team_config(config: &TeamConfig) -> io::Result<()> {
    let dir = team_store_dir(&config.org, &config.name);
    fs::create_dir_all(&dir)?;
    let path = dir.join("config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn load_mission(org: &str, team: &str) -> Option<String> {
    let path = team_store_dir(org, team).join("mission.md");
    fs::read_to_string(&path).ok()
}

pub fn save_mission(org: &str, team: &str, content: &str) -> io::Result<()> {
    let dir = team_store_dir(org, team);
    fs::create_dir_all(&dir)?;
    let path = dir.join("mission.md");
    fs::write(&path, content)?;
    Ok(())
}

pub fn load_playbook(org: &str, team: &str) -> String {
    let path = team_store_dir(org, team).join("playbook.md");
    fs::read_to_string(&path).unwrap_or_default()
}

pub fn append_playbook(org: &str, team: &str, section: &str, _project: &str, _date: &str) -> io::Result<()> {
    let dir = team_store_dir(org, team);
    fs::create_dir_all(&dir)?;
    let path = dir.join("playbook.md");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let new_content = if existing.is_empty() {
        section.to_string()
    } else {
        format!("{}\n\n---\n\n{}", existing, section)
    };
    fs::write(&path, new_content)?;
    Ok(())
}

pub fn list_agents(org: &str, team: &str) -> Vec<String> {
    let agents_dir = team_agents_dir(org, team);
    if !agents_dir.exists() {
        return vec![];
    }
    let mut agents: Vec<String> = fs::read_dir(&agents_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.ends_with(".md"))
                .map(|name| name.trim_end_matches(".md").to_string())
                .collect()
        })
        .unwrap_or_default();
    agents.sort();
    agents
}

pub fn load_agent(org: &str, team: &str, agent_name: &str) -> Option<String> {
    let path = team_agents_dir(org, team).join(format!("{}.md", agent_name));
    fs::read_to_string(&path).ok()
}

pub fn save_agent(org: &str, team: &str, agent_name: &str, content: &str, backup: bool) -> io::Result<()> {
    let agents_dir = team_agents_dir(org, team);
    fs::create_dir_all(&agents_dir)?;
    let agent_path = agents_dir.join(format!("{}.md", agent_name));

    if backup && agent_path.exists() {
        let history_dir = team_history_dir(org, team);
        fs::create_dir_all(&history_dir)?;
        let date = &crate::hooks::common::now_iso()[..10];
        let backup_name = format!("{}-{}.md", agent_name, date);
        let backup_path = history_dir.join(&backup_name);
        fs::copy(&agent_path, &backup_path)?;
    }

    fs::write(&agent_path, content)?;
    Ok(())
}

pub fn list_history(org: &str, team: &str, agent_name: &str) -> Vec<String> {
    let history_dir = team_history_dir(org, team);
    if !history_dir.exists() {
        return vec![];
    }
    let prefix = format!("{}-", agent_name);
    let mut history: Vec<String> = fs::read_dir(&history_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.starts_with(&prefix) && name.ends_with(".md"))
                .collect()
        })
        .unwrap_or_default();
    history.sort();
    history
}

// ── Defaults ──────────────────────────────────────────

pub fn default_org() -> String {
    "epic".to_string()
}

pub fn default_agents_for_type(team_type: &str) -> Vec<(&'static str, &'static str)> {
    match team_type {
        "stream" => vec![
            ("domain-expert", "Deep knowledge of the domain, business logic, and feature design"),
            ("reviewer", "Code review, quality assurance, and standards enforcement"),
            ("tester", "Test strategy, coverage analysis, and quality validation"),
        ],
        "platform" => vec![
            ("api-designer", "API design, contracts, versioning, and developer experience"),
            ("infra-specialist", "Infrastructure, deployment, reliability, and scalability"),
            ("dx-agent", "Developer experience, tooling, and platform usability"),
        ],
        "enabling" => vec![
            ("specialist", "Domain specialist providing expertise and enablement to other teams"),
        ],
        "subsystem" => vec![
            ("domain-specialist", "Deep subsystem knowledge, internals, and component ownership"),
            ("integration-tester", "Integration testing, interface contracts, and cross-system validation"),
        ],
        _ => vec![
            ("domain-expert", "Deep knowledge of the domain, business logic, and feature design"),
        ],
    }
}

pub fn build_agent_file(role: &str, description: &str, team_name: &str, _team_type: &str) -> String {
    let title = to_title_case(role);
    format!(
        "---\nname: {role}\ndescription: {description}\ntools: [Read, Edit, Write, Bash, Grep, Glob]\nmodel: sonnet\n---\n# {title}\n\nYou are the **{title}** for the **{team_name}** team.\n\n{description}\n"
    )
}

/// Inject `org` and `team` fields into frontmatter, and replace/append `## Team Context`.
/// Called at sync time on canonical agent content before writing to .claude/agents/.
pub fn inject_team_context(agent_content: &str, org: &str, team_name: &str, team_type: &str, mission: &str) -> String {
    let type_label = match team_type {
        "stream" => "Stream-aligned",
        "platform" => "Platform",
        "enabling" => "Enabling",
        "subsystem" => "Subsystem",
        _ => team_type,
    };

    // ── 1. Inject org + team into frontmatter ─────────────────────────────
    let content = if agent_content.starts_with("---") {
        // Find closing ---
        if let Some(end) = agent_content[3..].find("\n---") {
            let fm_body = &agent_content[3..3 + end];        // between the two ---
            let after   = &agent_content[3 + end + 4..];    // everything after closing ---

            // Strip any existing org:/team: lines then append fresh ones
            let cleaned_fm: String = fm_body
                .lines()
                .filter(|l| !l.starts_with("org:") && !l.starts_with("team:"))
                .collect::<Vec<_>>()
                .join("\n");

            format!("---{}\norg: {}\nteam: {}\n---{}", cleaned_fm, org, team_name, after)
        } else {
            agent_content.to_string()
        }
    } else {
        agent_content.to_string()
    };

    // ── 2. Inject/replace ## Team Context section ──────────────────────────
    let context_section = format!(
        "## Team Context\n**Org**: {}\n**Team**: {} ({})\n**Mission**: {}\n**Full playbook**: `epic team show {} --playbook`\n",
        org, team_name, type_label, mission, team_name
    );

    if let Some(pos) = content.find("## Team Context") {
        let before = &content[..pos];
        format!("{}\n{}", before.trim_end_matches('\n'), context_section)
    } else {
        format!("{}\n{}", content.trim_end_matches('\n'), context_section)
    }
}

/// Read the `org` field from an agent file's frontmatter.
/// Returns None if the file has no frontmatter or no `org:` field.
pub fn read_org_from_agent_file(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    let end = content[3..].find("\n---")?;
    let fm = &content[3..3 + end];
    for line in fm.lines() {
        if let Some(val) = line.strip_prefix("org:") {
            let v = val.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn build_playbook_section(team_name: &str, team_type: &str, agents: &[(String, String)], project: &str) -> String {
    let coordination_notes = match team_type {
        "stream" => "Stream-aligned teams own end-to-end delivery for their domain. Coordinate with platform teams for shared infrastructure and enabling teams for capability uplift.",
        "platform" => "Platform teams provide self-service capabilities to stream teams. Maintain clear API contracts, SLOs, and developer documentation.",
        "enabling" => "Enabling teams temporarily collaborate with stream teams to build capability. Time-box engagements and transfer knowledge back to the stream team.",
        "subsystem" => "Subsystem teams own complex components consumed by multiple stream teams. Maintain clear interface contracts and versioning policies.",
        _ => "Coordinate with other teams through clear interface contracts and shared standards.",
    };

    let mut agent_roster = String::new();
    for (name, desc) in agents {
        agent_roster.push_str(&format!("- **{}**: {}\n", name, desc));
    }

    format!(
        "## {} Team Playbook\n**Type**: {}  **Project**: {}\n### Agent Roster\n{}\n### Coordination\n{}\n",
        team_name, team_type, project, agent_roster, coordination_notes
    )
}

// Keep today_str accessible within tests
#[allow(dead_code)]
pub(crate) fn today_str_pub() -> String {
    today_str()
}
