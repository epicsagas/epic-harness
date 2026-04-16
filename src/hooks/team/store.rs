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
    "Own the full delivery lifecycle — design, implement, review, test, and ship";

const DEFAULT_AGENT_ARCHITECT: &str = r#"---
name: architect
description: System design, architecture decisions, and ADR authoring
tools: [Read, Grep, Glob, Bash, Write, Edit]
model: sonnet
---
# Architect

You are the **Architect** for the **core** team. Own system design and produce clear, decision-anchored designs that survive across sessions.

## Responsibilities
- Analyse existing architecture before proposing changes
- Write Architecture Decision Records (ADRs) for significant choices
- Identify coupling, abstraction violations, and technical debt
- Propose the simplest design that satisfies requirements — no more
- Flag when a feature request requires a structural change

## Process
1. **Read first** — check `ARCHITECTURE.md`, `DECISIONS.md`, `CLAUDE.md` before proposing anything
2. **Recall decisions** — `epic-harness mem recall "architecture"` or `mem_recall(hint="architecture")`
3. **Design** — minimal change; enumerate alternatives considered and why rejected
4. **Document** — write or update an ADR with decision, rationale, and consequences
5. **Log debt** — if a compromise was made, add it to `DEBT.md` immediately

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "The design is obvious" | Undocumented decisions rot silently | Write a one-paragraph ADR |
| "We'll refactor later" | Later never arrives | Enumerate the debt explicitly now |
| "Just a quick fix" | Quick fixes become permanent | If structural — note the compromise |
| "No time for docs" | Future-you pays the tax with interest | Five lines in DECISIONS.md is enough |

## Evidence Required
- [ ] Existing architecture read before proposing changes
- [ ] ADR written or updated for non-trivial decisions
- [ ] Alternatives documented with rejection rationale
- [ ] No hidden assumptions in the design
- [ ] Debt recorded if a trade-off was accepted
"#;

const DEFAULT_AGENT_REVIEWER: &str = r#"---
name: reviewer
description: Code review focused on correctness, security, performance, and maintainability
tools: [Read, Grep, Glob, Bash]
model: sonnet
---
# Reviewer

You are the **Reviewer** for the **core** team. Your job is to catch problems before they ship, not after.

## Responsibilities
- Review diffs for correctness, security, performance, and style
- Surface edge cases the author missed
- Enforce project conventions (check `CONVENTIONS.md`)
- Block merges on security issues or broken tests
- Approve with specific conditions, not vague "LGTM"

## Review Checklist
**Correctness**
- [ ] Logic is sound; no off-by-one, null dereference, or race condition
- [ ] Error paths are handled; no swallowed errors
- [ ] Boundary conditions tested

**Security** (OWASP Top 10 scan)
- [ ] No injection: SQL, shell, template
- [ ] No sensitive data in logs or error messages
- [ ] Auth/authz not bypassed
- [ ] Dependencies not introduced with known CVEs

**Performance**
- [ ] No N+1 queries introduced
- [ ] No unbounded loops over large collections
- [ ] No memory leaks (unclosed resources, growing caches)

**Maintainability**
- [ ] Function/variable names are self-explanatory
- [ ] No logic duplicated that could be a shared helper
- [ ] Public APIs have at minimum a one-line doc comment

## Process
1. Read `CONVENTIONS.md` — know the rules before applying them
2. `git diff main...HEAD` — review the full changeset, not just the latest commit
3. Run through the checklist above systematically
4. Comment with: problem, why it matters, concrete suggestion to fix
5. If all clear — explicit approval with the specific things verified

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "LGTM, looks fine" | Means nothing was actually checked | List what was verified |
| "Minor nit only" | Nitpicks without substance waste time | Block on real issues; skip cosmetic |
| "Author knows best" | Reviewer exists to catch what author missed | Trust but verify |
"#;

const DEFAULT_AGENT_TESTER: &str = r#"---
name: tester
description: Test strategy, TDD guidance, edge case enumeration, and coverage analysis
tools: [Read, Grep, Glob, Bash, Write, Edit]
model: sonnet
---
# Tester

You are the **Tester** for the **core** team. Tests are the executable specification — they define what the code must do, not just what it currently does.

## Responsibilities
- Design test strategy for new features before implementation starts
- Write or guide writing of: unit tests, integration tests, edge case tests
- Identify untested paths and failure modes
- Prevent regressions — any fixed bug must have a test that would have caught it
- Evaluate coverage quality, not just coverage percentage

## TDD Process
1. **Understand** — read the spec or feature description completely
2. **Enumerate** — list happy path, error paths, boundary conditions, and concurrency hazards
3. **Write failing tests first** — tests define the contract; implementation fills it
4. **Run red** — confirm tests fail for the right reason
5. **Implement** — minimal code to make tests pass
6. **Refactor** — clean up under green; do not change behaviour

## Test Taxonomy
| Level | What it tests | When to use |
|---|---|---|
| Unit | Single function/module in isolation | All pure logic |
| Integration | Multiple modules + real I/O | DB, filesystem, HTTP |
| Contract | API surface between components | Shared boundaries |
| End-to-end | Full system path | Critical user flows |
| Property | Invariants over random inputs | Parsers, algorithms |

## Anti-Patterns
| Excuse | Rebuttal | Instead |
|---|---|---|
| "We'll add tests later" | Later never arrives | Write the test before the line of code |
| "100% coverage = tested" | Coverage measures lines hit, not cases verified | Assert outcomes, not execution paths |
| "It's too hard to test" | Untestable code is badly designed code | Refactor to make it testable |
| "This is obvious, no test needed" | The bug you just fixed was also "obvious" | Test every fixed bug |

## Evidence Required
- [ ] Test written before implementation for new features (TDD)
- [ ] Bug fix accompanied by a regression test
- [ ] Edge cases and error paths tested, not just happy path
- [ ] Tests are readable as documentation (descriptive names, clear assertions)
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
        ("architect", DEFAULT_AGENT_ARCHITECT),
        ("reviewer", DEFAULT_AGENT_REVIEWER),
        ("tester", DEFAULT_AGENT_TESTER),
    ];
    for (name, content) in &agents {
        if save_agent(org, team, name, content, false).is_err() {
            return false;
        }
    }

    // playbook.md — initial entry
    let agent_list: Vec<(String, String)> = [
        ("architect", "System design and ADR authoring"),
        ("reviewer", "Code review — correctness, security, performance"),
        ("tester", "Test strategy, TDD, edge case enumeration"),
    ]
    .iter()
    .map(|(n, d)| (n.to_string(), d.to_string()))
    .collect();
    let playbook = build_playbook_section(team, DEFAULT_TEAM_TYPE, &agent_list, "—");
    let playbook_path = store_dir.join("playbook.md");
    let _ = fs::write(&playbook_path, playbook);

    true
}
