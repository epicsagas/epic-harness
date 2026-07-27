//! codex.rs — Render team agents as native Codex custom agents.
//!
//! Epic stores agents as Claude-style Markdown with YAML frontmatter:
//!
//! ```text
//! ---
//! name: ops
//! description: CI/CD pipelines, deployment, release management
//! tools: [Read, Edit, Write, Bash, Grep, Glob]
//! model: sonnet
//! skills: [verify, secure]
//! ---
//! # Ops
//! ...body...
//! ```
//!
//! Codex does not read that. A Codex custom agent is a **flat TOML file** in
//! `~/.codex/agents/` (or `.codex/agents/`) whose required keys are `name`,
//! `description` and `developer_instructions`. Copying the Markdown across, as
//! the sync previously did, produced files Codex silently ignores.
//!
//! Deliberately dropped when converting:
//!
//! * `model: sonnet` — Codex's optional `model` expects a Codex model slug.
//!   There is no defensible mapping from an Anthropic alias, and omitting the
//!   key makes the agent inherit the parent model, which is the sane default.
//! * `tools:` — those are Claude tool names and mean nothing to Codex.
//! * `skills:` — Epic's own concept, already described in the body.

use serde::Serialize;

/// A Codex custom agent definition. Field names are the TOML keys Codex reads.
#[derive(Debug, Serialize, PartialEq)]
pub struct CodexAgent {
    pub name: String,
    pub description: String,
    pub developer_instructions: String,
}

/// Split YAML frontmatter from the Markdown body.
///
/// Returns `(frontmatter, body)`; frontmatter is empty when the document has none.
fn split_frontmatter(md: &str) -> (&str, &str) {
    // Tolerate a leading BOM/newlines before the opening fence.
    let trimmed = md.trim_start_matches('\u{feff}').trim_start();
    let Some(rest) = trimmed.strip_prefix("---") else {
        return ("", trimmed);
    };
    match rest.find("\n---") {
        Some(end) => {
            let fm = &rest[..end];
            // Skip past "\n---" then the remainder of that line.
            let after = &rest[end + 4..];
            let body = after.strip_prefix('\n').unwrap_or(after);
            (fm, body)
        }
        None => ("", trimmed),
    }
}

/// Read a scalar frontmatter key, ignoring list/nested values.
fn frontmatter_value(fm: &str, key: &str) -> Option<String> {
    for line in fm.lines() {
        let line = line.trim_end();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let v = rest.trim();
        // Lists (`[a, b]`) are not scalars we can use as a description.
        if v.starts_with('[') {
            return None;
        }
        let v = v.trim_matches('"').trim_matches('\'').trim();
        if v.is_empty() {
            return None;
        }
        return Some(v.to_string());
    }
    None
}

/// Build the Codex agent identity for a team member.
///
/// Codex discovers agent files flatly and matches on the `name` field, so team
/// membership is encoded in the name to keep two teams' `reviewer` agents
/// distinct.
pub fn codex_agent_name(team: &str, agent: &str) -> String {
    format!("{team}-{agent}")
}

/// The Codex agent files a team owns, inside `agents_dir`.
///
/// `sync` writes flat `{team}-{agent}.toml` files, so nothing else can find or
/// remove them by team — `status` reported no Codex agents at all and `unlink`
/// left them behind. The prefix match is the inverse of `codex_agent_name`.
pub fn team_agent_files(agents_dir: &std::path::Path, team: &str) -> Vec<std::path::PathBuf> {
    let prefix = format!("{team}-");
    let Ok(entries) = std::fs::read_dir(agents_dir) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("toml")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    files.sort();
    files
}

/// Convert one team agent Markdown document into a Codex agent definition.
pub fn to_codex_agent(team: &str, agent: &str, markdown: &str) -> CodexAgent {
    let (fm, body) = split_frontmatter(markdown);

    let description = frontmatter_value(fm, "description")
        .unwrap_or_else(|| format!("{agent} agent for team {team}"));

    let instructions = body.trim();
    let developer_instructions = if instructions.is_empty() {
        // Codex requires the key; an empty prompt would make the agent useless,
        // so fall back to something honest rather than shipping a blank.
        format!("Act as the {agent} agent for team {team}.")
    } else {
        instructions.to_string()
    };

    CodexAgent {
        name: codex_agent_name(team, agent),
        description,
        developer_instructions,
    }
}

/// Serialize a Codex agent to TOML.
///
/// Uses the `toml` serializer rather than string formatting so quotes, newlines
/// and backslashes in the instructions are escaped correctly.
pub fn render_codex_toml(agent: &CodexAgent) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(agent)
}

#[cfg(test)]
mod discovery_tests {
    use super::team_agent_files;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_only_this_teams_toml_agents() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        for name in [
            "alpha-reviewer.toml",
            "alpha-builder.toml",
            "beta-reviewer.toml",
            "alpha-notes.md",
        ] {
            fs::write(base.join(name), "x").unwrap();
        }
        fs::create_dir(base.join("alpha-subdir.toml")).unwrap();

        let found: Vec<String> = team_agent_files(base, "alpha")
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();
        assert_eq!(found, vec!["alpha-builder.toml", "alpha-reviewer.toml"]);
    }

    #[test]
    fn a_missing_directory_yields_nothing() {
        let dir = tempdir().unwrap();
        assert!(team_agent_files(&dir.path().join("absent"), "alpha").is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nname: ops\ndescription: CI/CD pipelines and releases\ntools: [Read, Edit, Bash]\nmodel: sonnet\nskills: [verify]\n---\n# Ops\n\nOwn the deploy path.\n";

    #[test]
    fn extracts_description_and_body() {
        let a = to_codex_agent("core", "ops", SAMPLE);
        assert_eq!(a.name, "core-ops");
        assert_eq!(a.description, "CI/CD pipelines and releases");
        assert!(a.developer_instructions.starts_with("# Ops"));
        assert!(a.developer_instructions.contains("Own the deploy path."));
    }

    /// The whole point of the conversion: no Claude-only keys survive.
    #[test]
    fn drops_claude_specific_frontmatter() {
        let toml_out = render_codex_toml(&to_codex_agent("core", "ops", SAMPLE)).unwrap();
        assert!(!toml_out.contains("model = \"sonnet\""));
        assert!(!toml_out.contains("tools"));
        assert!(!toml_out.contains("skills = "));
        // Required Codex keys present.
        assert!(toml_out.contains("name = \"core-ops\""));
        assert!(toml_out.contains("description = "));
        assert!(toml_out.contains("developer_instructions = "));
    }

    #[test]
    fn output_parses_back_as_toml_with_required_keys() {
        let toml_out = render_codex_toml(&to_codex_agent("core", "ops", SAMPLE)).unwrap();
        // `from_str` parses a TOML *document*; `str::parse` parses a single value.
        let v: toml::Table = toml::from_str(&toml_out).expect("valid TOML document");
        for key in ["name", "description", "developer_instructions"] {
            assert!(v.get(key).is_some(), "missing required key {key}");
        }
    }

    #[test]
    fn quotes_and_newlines_survive_round_trip() {
        let md = "---\nname: x\ndescription: has \"quotes\" inside\n---\nLine1\n\"quoted\"\nback\\slash\n";
        let agent = to_codex_agent("t", "x", md);
        let toml_out = render_codex_toml(&agent).unwrap();
        let v: toml::Table = toml::from_str(&toml_out).expect("valid TOML despite quotes");
        assert_eq!(
            v.get("description").and_then(|d| d.as_str()),
            Some("has \"quotes\" inside")
        );
        let di = v
            .get("developer_instructions")
            .and_then(|d| d.as_str())
            .unwrap();
        assert!(di.contains("\"quoted\""));
        assert!(di.contains("back\\slash"));
    }

    #[test]
    fn falls_back_when_description_missing() {
        let md = "---\nname: solo\ntools: [Read]\n---\nBody here\n";
        let a = to_codex_agent("core", "solo", md);
        assert_eq!(a.description, "solo agent for team core");
    }

    #[test]
    fn list_valued_description_is_not_used() {
        // A malformed `description: [a, b]` must not become the description.
        let md = "---\ndescription: [a, b]\n---\nBody\n";
        let a = to_codex_agent("core", "z", md);
        assert_eq!(a.description, "z agent for team core");
    }

    #[test]
    fn handles_document_without_frontmatter() {
        let a = to_codex_agent("core", "plain", "Just a body\n");
        assert_eq!(a.description, "plain agent for team core");
        assert_eq!(a.developer_instructions, "Just a body");
    }

    #[test]
    fn empty_body_gets_honest_fallback() {
        let a = to_codex_agent("core", "bare", "---\nname: bare\n---\n");
        assert!(a.developer_instructions.contains("bare"));
        assert!(!a.developer_instructions.is_empty());
    }

    #[test]
    fn agent_name_scopes_by_team() {
        assert_eq!(codex_agent_name("alpha", "reviewer"), "alpha-reviewer");
        assert_ne!(
            codex_agent_name("alpha", "reviewer"),
            codex_agent_name("beta", "reviewer")
        );
    }
}
