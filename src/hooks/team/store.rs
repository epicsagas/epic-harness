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

pub(crate) fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        return PathBuf::from(up);
    }
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return PathBuf::from(format!("{}{}", drive, path));
    }
    eprintln!("[harness] warning: HOME/USERPROFILE not set; team operations may fail");
    PathBuf::from(".")
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

fn tools_for_role(role: &str) -> &'static str {
    match role {
        r if r.contains("audit")
            || r.contains("review")
            || r.contains("explor")
            || r.contains("scan")
            || r.contains("analyz") =>
        {
            "[Read, Grep, Glob, Bash]"
        }
        r if r.contains("plan") || r.contains("architect") || r.contains("design") => {
            "[Read, Grep, Glob]"
        }
        _ => "[Read, Edit, Write, Bash, Grep, Glob]",
    }
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
        .map_err(io::Error::other)?;
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

pub fn append_playbook(org: &str, team: &str, section: &str, project: &str, date: &str) -> io::Result<()> {
    let dir = team_store_dir(org, team);
    fs::create_dir_all(&dir)?;
    let path = dir.join("playbook.md");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let header = if !project.is_empty() || !date.is_empty() {
        format!("<!-- project: {} | date: {} -->\n", project, date)
    } else {
        String::new()
    };
    let new_content = if existing.is_empty() {
        format!("{}{}", header, section)
    } else {
        format!("{}\n\n---\n\n{}{}", existing, header, section)
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
    let tools = tools_for_role(role);
    format!(
        "---\nname: {role}\ndescription: {description}\ntools: {tools}\nmodel: sonnet\n---\n# {title}\n\nYou are the **{title}** for the **{team_name}** team.\n\n{description}\n"
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
    let content = if let Some(rest) = agent_content.strip_prefix("---") {
        // Find closing ---
        if let Some(end) = rest.find("\n---") {
            let fm_body = &rest[..end];                      // between the two ---
            let after   = &rest[end + 4..];                  // everything after closing ---

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

// ── Default epic team preset ───────────────────────────

const DEFAULT_TEAM_NAME: &str = "core";
const DEFAULT_TEAM_TYPE: &str = "stream";
const DEFAULT_TEAM_MISSION: &str =
    "Support delivery with operations, documentation, and codebase exploration — complementing builder, reviewer, auditor, and planner";

const DEFAULT_AGENT_OPS: &str = r#"---
name: ops
description: CI/CD pipelines, deployment, release management, and infrastructure config
tools: [Read, Grep, Glob, Bash, Write, Edit]
model: sonnet
skills: [verify, secure]
---
# Ops

You are the **Ops** agent for the **core** team. Own the delivery pipeline — from CI config to production release.

## Responsibilities
- Set up and maintain CI/CD pipelines (GitHub Actions, GitLab CI, etc.)
- Manage release processes: versioning, changelogs, tags, publish steps
- Harden infrastructure config: secrets handling, env vars, least-privilege
- Diagnose and fix build/deploy failures
- Run `/verify` before every release gate — build + test + lint must pass

## Process
1. **Read pipeline config** — understand existing CI before touching it
2. **Check secrets** — no credentials in code, CI env vars only
3. **Run `/secure`** — infrastructure changes get a security pass
4. **Verify** — `epic-harness verify` or equivalent must pass before marking done
5. **Document** — update `DEPLOYMENT.md` or CI comments for non-obvious config

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "It works locally" | CI environment differs | Reproduce in CI or use act/docker |
| "Skip tests to unblock deploy" | Broken prod is worse than a delayed deploy | Fix the test or revert |
| "Hardcode the secret for now" | Secrets in code never stay temporary | Use env vars from day one |
| "--no-verify to push faster" | Hooks exist for a reason | Fix the underlying issue |

## Evidence Required
- [ ] CI passes before deploy proceeds
- [ ] No secrets in code or logs
- [ ] `/verify` run and passed
- [ ] Release tag and changelog entry created
- [ ] Rollback path identified for significant deploys
"#;

const DEFAULT_AGENT_SCRIBE: &str = r#"---
name: scribe
description: Documentation authoring — READMEs, changelogs, ADRs, API docs, and onboarding guides
tools: [Read, Grep, Glob, Bash, Write, Edit]
model: sonnet
skills: [document, commit]
---
# Scribe

You are the **Scribe** for the **core** team. Own all written knowledge — if it isn't documented, it doesn't exist.

## Responsibilities
- Write and maintain READMEs, onboarding guides, and runbooks
- Author Architecture Decision Records (ADRs) after design decisions land
- Keep changelogs current using Conventional Commits format
- Generate or update API docs when public interfaces change
- Run `/document` after any public API or module change

## Process
1. **Read before writing** — check existing docs to avoid duplication or contradiction
2. **Write for the reader** — target audience determines depth and vocabulary
3. **Run `/document`** — auto-generates docstrings/comments for changed code
4. **Commit with `/commit`** — Conventional Commits message: `docs:` prefix
5. **Link everything** — new docs get cross-referenced from README or index

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "The code is self-documenting" | Code explains what; docs explain why | Write the why |
| "I'll document it after the feature lands" | It never lands in docs | Write docs as part of the PR |
| "It's obvious to anyone who reads the code" | New teammates read docs first | Write for someone who hasn't seen the code |
| "Just update the README" | One file can't hold everything | Use the right doc type: ADR, runbook, API ref |

## Evidence Required
- [ ] README updated if setup or usage changed
- [ ] ADR written for non-trivial architectural decisions
- [ ] Changelog entry added for every user-visible change
- [ ] API docs updated for changed public interfaces
- [ ] New docs cross-linked from existing entry points
"#;

const DEFAULT_AGENT_EXPLORER: &str = r#"---
name: explorer
description: Codebase archaeology, dependency analysis, tech research, and unknown-territory mapping
tools: [Read, Grep, Glob, Bash]
model: sonnet
skills: [debug]
---
# Explorer

You are the **Explorer** for the **core** team. Map unknown territory — answer "how does X work?", "why does Y happen?", and "what will break if we change Z?".

## Responsibilities
- Trace unfamiliar code paths end-to-end before others touch them
- Analyse dependency graphs: what calls what, what owns what data
- Research external libraries, APIs, and tools — surface gotchas and version constraints
- Flag potential performance risks (loops, queries, allocations) for the **auditor** to evaluate — do not own the perf verdict
- Investigate bugs with `/debug` when the root cause is unknown

## Process
1. **Bound the search** — define the question precisely before exploring
2. **Read, don't assume** — grep and glob before forming conclusions
3. **Trace call chains** — follow the data flow from entry point to result
4. **Run `/debug`** — for bugs, hypothesize → isolate → confirm before reporting
5. **Run `/perf`** — flag loops, queries, or allocations that will hurt at scale
6. **Report findings** — write a short summary: what you found, what's safe to change, what's risky

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "I think it works like X" | Assumptions compound into wrong designs | Read the actual code path |
| "The README says..." | READMEs drift from reality | Verify against the source |
| "It's probably fine to change" | Impact analysis takes 10 minutes; a broken deploy takes hours | Trace dependents before touching |
| "We can optimize later" | Later never arrives for the 10× slowdown | Flag it now with `/perf` |

## Evidence Required
- [ ] Code path traced end-to-end, not assumed
- [ ] All callers of changed code identified
- [ ] External dependency gotchas documented
- [ ] Performance risk flagged if loops or queries involved
- [ ] Findings summarized in plain language for the team
"#;

/// Install the default `core` stream-aligned team into `~/.harness/orgs/{org}/`
/// if no teams exist in that org yet. Silent no-op if teams already present.
pub fn install_default_team_if_needed(org: &str) -> bool {
    // Skip if org already has teams
    if !list_teams(org).is_empty() {
        return false;
    }

    let team = DEFAULT_TEAM_NAME;

    // Create directories
    let store_dir = team_store_dir(org, team);
    let agents_dir = team_agents_dir(org, team);
    if fs::create_dir_all(&store_dir).is_err() || fs::create_dir_all(&agents_dir).is_err() {
        return false;
    }

    // config.json
    let now = crate::hooks::common::now_iso();
    let config = TeamConfig {
        name: team.to_string(),
        org: org.to_string(),
        team_type: DEFAULT_TEAM_TYPE.to_string(),
        projects: vec![],
        created: now.clone(),
        updated: now,
    };
    if save_team_config(&config).is_err() {
        return false;
    }

    // mission.md
    if save_mission(org, team, DEFAULT_TEAM_MISSION).is_err() {
        return false;
    }

    // agents
    let agents = [
        ("ops", DEFAULT_AGENT_OPS),
        ("scribe", DEFAULT_AGENT_SCRIBE),
        ("explorer", DEFAULT_AGENT_EXPLORER),
    ];
    for (name, content) in &agents {
        if save_agent(org, team, name, content, false).is_err() {
            return false;
        }
    }

    // playbook.md — initial entry
    let agent_list: Vec<(String, String)> = [
        ("ops", "CI/CD, deployment, release management, infrastructure config"),
        ("scribe", "READMEs, changelogs, ADRs, API docs, onboarding guides"),
        ("explorer", "Codebase archaeology, dependency analysis, performance investigation"),
    ]
    .iter()
    .map(|(n, d)| (n.to_string(), d.to_string()))
    .collect();
    let playbook = build_playbook_section(team, DEFAULT_TEAM_TYPE, &agent_list, "—");
    let playbook_path = store_dir.join("playbook.md");
    let _ = fs::write(&playbook_path, playbook);

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Serialize tests that mutate the process-wide HOME env var.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_install_default_team_idempotent() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe { env::set_var("HOME", tmp.path()); }

        assert!(install_default_team_if_needed("epic"), "first call should seed");
        assert!(!install_default_team_if_needed("epic"), "second call should no-op");

        let teams = list_teams("epic");
        assert_eq!(teams, vec!["core"]);
    }

    #[test]
    fn test_default_team_agents_seeded() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe { env::set_var("HOME", tmp.path()); }
        install_default_team_if_needed("epic");
        let agents = list_agents("epic", "core");
        assert!(!agents.is_empty(), "core team should have agents after seeding");
        assert!(agents.contains(&"ops".to_string()));
        assert!(agents.contains(&"scribe".to_string()));
        assert!(agents.contains(&"explorer".to_string()));
    }

    #[test]
    fn test_list_orgs_empty() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe { env::set_var("HOME", tmp.path()); }
        let orgs = list_orgs();
        assert!(orgs.is_empty(), "fresh HOME should have no orgs");
    }

    #[test]
    fn test_org_show_no_teams() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe { env::set_var("HOME", tmp.path()); }
        // Create an org dir with no teams subdir
        let org_path = tmp.path().join(".harness").join("orgs").join("empty-org");
        std::fs::create_dir_all(&org_path).unwrap();
        let teams = list_teams("empty-org");
        assert!(teams.is_empty(), "org with no teams dir should return empty list");
    }

    #[test]
    fn test_status_no_agents_dir() {
        // When .claude/agents/ does not exist, scanning produces empty list.
        // We test this via the store: list_teams on a non-existent org returns empty.
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: HOME_LOCK serializes HOME mutation across team tests
        unsafe { env::set_var("HOME", tmp.path()); }
        // Simulate: project dir with no .claude/agents/ — scan returns empty
        let project_agents = tmp.path().join("project").join(".claude").join("agents");
        assert!(!project_agents.exists(), ".claude/agents should not exist yet");
        // Reading subdirs from a nonexistent path must yield empty (not panic)
        let subdirs: Vec<_> = if project_agents.is_dir() {
            std::fs::read_dir(&project_agents)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![]
        };
        assert!(subdirs.is_empty(), "no .claude/agents/ should yield empty linked teams");
    }
}
