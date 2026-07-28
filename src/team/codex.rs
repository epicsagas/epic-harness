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
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const ID_PREFIX: &str = "epic-";
const ID_V2_PREFIX: &str = "epic-v2-";
const OWNERSHIP_ORG_PREFIX: &str = "# epic-harness-org: ";
const OWNERSHIP_TEAM_PREFIX: &str = "# epic-harness-team: ";
const OWNERSHIP_AGENT_PREFIX: &str = "# epic-harness-agent: ";

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
fn legacy_codex_agent_name(org: &str, team: &str, agent: &str) -> String {
    // Length prefixes make the boundary reversible even when either component
    // contains hyphens or the same team name exists in two organisations.
    format!(
        "{ID_PREFIX}{}-{org}-{}-{team}-{}-{agent}",
        org.len(),
        team.len(),
        agent.len()
    )
}

fn requires_casefold_safe_identity(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| byte.is_ascii_uppercase() || byte == b'_')
}

fn hex_component(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex_component(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

pub fn codex_agent_name(org: &str, team: &str, agent: &str) -> String {
    if [org, team, agent]
        .into_iter()
        .any(requires_casefold_safe_identity)
    {
        format!(
            "{ID_V2_PREFIX}{}-{}-{}",
            hex_component(org),
            hex_component(team),
            hex_component(agent)
        )
    } else {
        legacy_codex_agent_name(org, team, agent)
    }
}

fn decode_codex_agent_name(name: &str) -> Option<(String, String, String)> {
    if let Some(rest) = name.strip_prefix(ID_V2_PREFIX) {
        let mut components = rest.split('-');
        let org = decode_hex_component(components.next()?)?;
        let team = decode_hex_component(components.next()?)?;
        let agent = decode_hex_component(components.next()?)?;
        return components.next().is_none().then_some((org, team, agent));
    }

    let rest = name.strip_prefix(ID_PREFIX)?;
    let (org_len, rest) = rest.split_once('-')?;
    let org_len = org_len.parse::<usize>().ok()?;
    if rest.len() < org_len + 1 {
        return None;
    }
    let (org, rest) = rest.split_at(org_len);
    let rest = rest.strip_prefix('-')?;
    let (team_len, rest) = rest.split_once('-')?;
    let team_len = team_len.parse::<usize>().ok()?;
    if rest.len() < team_len + 1 {
        return None;
    }
    let (team, rest) = rest.split_at(team_len);
    let rest = rest.strip_prefix('-')?;
    let (agent_len, agent) = rest.split_once('-')?;
    let agent_len = agent_len.parse::<usize>().ok()?;
    (agent.len() == agent_len).then_some((org.to_string(), team.to_string(), agent.to_string()))
}

fn ownership_from_contents(content: &str) -> Option<(&str, &str, &str)> {
    let org = content
        .lines()
        .find_map(|line| line.strip_prefix(OWNERSHIP_ORG_PREFIX))?;
    let team = content
        .lines()
        .find_map(|line| line.strip_prefix(OWNERSHIP_TEAM_PREFIX))?;
    let agent = content
        .lines()
        .find_map(|line| line.strip_prefix(OWNERSHIP_AGENT_PREFIX))?;
    (!org.is_empty() && !team.is_empty() && !agent.is_empty()).then_some((org, team, agent))
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn has_exact_ownership(path: &Path, org: &str, team: &str) -> bool {
    if !is_regular_file(path) {
        return false;
    }
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Some((recorded_org, recorded_team, agent)) = ownership_from_contents(&content) else {
        return false;
    };
    let name = codex_agent_name(recorded_org, recorded_team, agent);
    let legacy_name = legacy_codex_agent_name(recorded_org, recorded_team, agent);
    recorded_org == org
        && recorded_team == team
        && matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some(file_name)
                if file_name == format!("{name}.toml")
                    || file_name == format!("{legacy_name}.toml")
        )
}

/// The Codex agent files a team owns, inside `agents_dir`.
///
/// Ownership is recorded in the generated file and must agree with its reversible
/// filename. Legacy `{team}-{agent}.toml` files are deliberately not selected:
/// their ownership cannot be proven once names contain hyphens.
pub fn team_agent_files(agents_dir: &Path, org: &str, team: &str) -> Vec<PathBuf> {
    if fs::symlink_metadata(agents_dir)
        .map(|metadata| metadata.file_type().is_symlink() || !metadata.is_dir())
        .unwrap_or(true)
    {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(agents_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("toml")
                && has_exact_ownership(p, org, team)
        })
        .collect();
    files.sort();
    files
}

/// Create `agents_dir` only when its parent and the destination directory are
/// real directories. This refuses a symlink before a sync can write through it.
pub fn prepare_agents_dir(agents_dir: &Path) -> io::Result<()> {
    let parent = agents_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Codex agents directory has no parent",
        )
    })?;
    let parent_meta = fs::symlink_metadata(parent)?;
    if parent_meta.file_type().is_symlink() || !parent_meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Codex config directory is not a regular directory: {}",
                parent.display()
            ),
        ));
    }
    match fs::symlink_metadata(agents_dir) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Codex agents directory is not a regular directory: {}",
                agents_dir.display()
            ),
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(agents_dir)?;
            prepare_agents_dir(agents_dir)
        }
        Err(e) => Err(e),
    }
}

/// Atomically write one owned Codex agent. The destination is never followed if
/// it is a symlink, and an unrelated regular file is never overwritten.
pub fn write_agent_file(
    agents_dir: &Path,
    org: &str,
    team: &str,
    agent: &str,
    payload: &str,
) -> io::Result<PathBuf> {
    prepare_agents_dir(agents_dir)?;
    let name = codex_agent_name(org, team, agent);
    let destination = agents_dir.join(format!("{name}.toml"));
    match fs::symlink_metadata(&destination) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "refusing to replace Codex agent symlink: {}",
                    destination.display()
                ),
            ));
        }
        Ok(meta) if !meta.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Codex agent destination is not a regular file: {}",
                    destination.display()
                ),
            ));
        }
        Ok(_) => {
            let existing = fs::read_to_string(&destination)?;
            if ownership_from_contents(&existing) != Some((org, team, agent)) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "refusing to overwrite unowned Codex agent: {}",
                        destination.display()
                    ),
                ));
            }
            if existing == payload {
                return Ok(destination);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    for attempt in 0..32 {
        let temporary = agents_dir.join(format!(".{name}.{}.{}.tmp", std::process::id(), attempt));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        if let Err(e) = file
            .write_all(payload.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&temporary);
            return Err(e);
        }
        drop(file);
        if let Err(e) = atomic_replace_file(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            return Err(e);
        }
        let legacy = agents_dir.join(format!(
            "{}.toml",
            legacy_codex_agent_name(org, team, agent)
        ));
        if legacy != destination && has_exact_ownership(&legacy, org, team) {
            fs::remove_file(legacy)?;
        }
        return Ok(destination);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate temporary Codex agent file in {}",
            agents_dir.display()
        ),
    ))
}

#[cfg(not(windows))]
pub(crate) fn atomic_replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(any(windows, test))]
const fn windows_replace_flags() -> u32 {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
}

#[cfg(windows)]
pub(crate) fn atomic_replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let source: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            windows_replace_flags(),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Convert one team agent Markdown document into a Codex agent definition.
pub fn to_codex_agent(org: &str, team: &str, agent: &str, markdown: &str) -> CodexAgent {
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
        name: codex_agent_name(org, team, agent),
        description,
        developer_instructions,
    }
}

/// Serialize a Codex agent to TOML.
///
/// Uses the `toml` serializer rather than string formatting so quotes, newlines
/// and backslashes in the instructions are escaped correctly.
pub fn render_codex_toml(agent: &CodexAgent) -> Result<String, toml::ser::Error> {
    let rendered = toml::to_string_pretty(agent)?;
    Ok(match decode_codex_agent_name(&agent.name) {
        Some((org, team, agent)) => {
            format!(
                "{OWNERSHIP_ORG_PREFIX}{org}\n{OWNERSHIP_TEAM_PREFIX}{team}\n{OWNERSHIP_AGENT_PREFIX}{agent}\n{rendered}"
            )
        }
        None => rendered,
    })
}

#[cfg(test)]
mod discovery_tests {
    use super::{codex_agent_name, team_agent_files};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_only_this_teams_toml_agents() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        for (team, agent) in [
            ("alpha", "reviewer"),
            ("alpha", "builder"),
            ("beta", "reviewer"),
        ] {
            let name = codex_agent_name("epic", team, agent);
            fs::write(
                base.join(format!("{name}.toml")),
                format!("# epic-harness-org: epic\n# epic-harness-team: {team}\n# epic-harness-agent: {agent}\nname = \"{name}\"\n"),
            )
            .unwrap();
        }
        fs::write(base.join("alpha-notes.md"), "x").unwrap();
        fs::create_dir(base.join("alpha-subdir.toml")).unwrap();

        let found: Vec<String> = team_agent_files(base, "epic", "alpha")
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();
        assert_eq!(
            found,
            vec![
                "epic-4-epic-5-alpha-7-builder.toml",
                "epic-4-epic-5-alpha-8-reviewer.toml"
            ]
        );
    }

    #[test]
    fn a_missing_directory_yields_nothing() {
        let dir = tempdir().unwrap();
        assert!(team_agent_files(&dir.path().join("absent"), "epic", "alpha").is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn a_symlinked_agents_directory_is_not_scanned() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("agents");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(team_agent_files(&link, "epic", "alpha").is_empty());
    }

    #[test]
    fn deletion_selection_requires_exact_recorded_ownership() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        // This is the legacy spelling for alpha/beta-reviewer. A prefix scan
        // incorrectly assigns it to alpha-beta as well.
        fs::write(
            base.join("alpha-beta-reviewer.toml"),
            "# epic-harness-org: epic\n# epic-harness-team: alpha\n# epic-harness-agent: beta-reviewer\nname = \"alpha-beta-reviewer\"\n",
        )
        .unwrap();

        assert!(
            team_agent_files(base, "epic", "alpha-beta").is_empty(),
            "a file without exact alpha-beta ownership must never be a deletion target"
        );
    }

    #[test]
    fn deletion_selection_includes_legacy_owned_identity() {
        let dir = tempdir().unwrap();
        let legacy = dir.path().join("epic-4-Epic-5-Alpha-8-reviewer.toml");
        fs::write(
            &legacy,
            "# epic-harness-org: Epic\n# epic-harness-team: Alpha\n# epic-harness-agent: reviewer\nname = \"legacy\"\n",
        )
        .unwrap();

        assert_eq!(team_agent_files(dir.path(), "Epic", "Alpha"), vec![legacy]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nname: ops\ndescription: CI/CD pipelines and releases\ntools: [Read, Edit, Bash]\nmodel: sonnet\nskills: [verify]\n---\n# Ops\n\nOwn the deploy path.\n";

    #[test]
    fn extracts_description_and_body() {
        let a = to_codex_agent("epic", "core", "ops", SAMPLE);
        assert_eq!(a.name, "epic-4-epic-4-core-3-ops");
        assert_eq!(a.description, "CI/CD pipelines and releases");
        assert!(a.developer_instructions.starts_with("# Ops"));
        assert!(a.developer_instructions.contains("Own the deploy path."));
    }

    /// The whole point of the conversion: no Claude-only keys survive.
    #[test]
    fn drops_claude_specific_frontmatter() {
        let toml_out = render_codex_toml(&to_codex_agent("epic", "core", "ops", SAMPLE)).unwrap();
        assert!(!toml_out.contains("model = \"sonnet\""));
        assert!(!toml_out.contains("tools"));
        assert!(!toml_out.contains("skills = "));
        // Required Codex keys present.
        assert!(toml_out.contains("name = \"epic-4-epic-4-core-3-ops\""));
        assert!(toml_out.contains("description = "));
        assert!(toml_out.contains("developer_instructions = "));
    }

    #[test]
    fn output_parses_back_as_toml_with_required_keys() {
        let toml_out = render_codex_toml(&to_codex_agent("epic", "core", "ops", SAMPLE)).unwrap();
        // `from_str` parses a TOML *document*; `str::parse` parses a single value.
        let v: toml::Table = toml::from_str(&toml_out).expect("valid TOML document");
        for key in ["name", "description", "developer_instructions"] {
            assert!(v.get(key).is_some(), "missing required key {key}");
        }
    }

    #[test]
    fn quotes_and_newlines_survive_round_trip() {
        let md = "---\nname: x\ndescription: has \"quotes\" inside\n---\nLine1\n\"quoted\"\nback\\slash\n";
        let agent = to_codex_agent("epic", "t", "x", md);
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
        let a = to_codex_agent("epic", "core", "solo", md);
        assert_eq!(a.description, "solo agent for team core");
    }

    #[test]
    fn list_valued_description_is_not_used() {
        // A malformed `description: [a, b]` must not become the description.
        let md = "---\ndescription: [a, b]\n---\nBody\n";
        let a = to_codex_agent("epic", "core", "z", md);
        assert_eq!(a.description, "z agent for team core");
    }

    #[test]
    fn handles_document_without_frontmatter() {
        let a = to_codex_agent("epic", "core", "plain", "Just a body\n");
        assert_eq!(a.description, "plain agent for team core");
        assert_eq!(a.developer_instructions, "Just a body");
    }

    #[test]
    fn empty_body_gets_honest_fallback() {
        let a = to_codex_agent("epic", "core", "bare", "---\nname: bare\n---\n");
        assert!(a.developer_instructions.contains("bare"));
        assert!(!a.developer_instructions.is_empty());
    }

    #[test]
    fn agent_name_scopes_by_team() {
        assert_eq!(
            codex_agent_name("epic", "alpha", "reviewer"),
            "epic-4-epic-5-alpha-8-reviewer"
        );
        assert_ne!(
            codex_agent_name("epic", "alpha", "reviewer"),
            codex_agent_name("epic", "beta", "reviewer")
        );
        assert_ne!(
            codex_agent_name("org-a", "alpha", "reviewer"),
            codex_agent_name("org-b", "alpha", "reviewer")
        );
    }

    #[test]
    fn hyphenated_team_and_agent_names_have_distinct_reversible_identities() {
        let first = codex_agent_name("epic", "alpha-beta", "reviewer");
        let second = codex_agent_name("epic", "alpha", "beta-reviewer");

        assert_ne!(first, second);
        assert_eq!(
            decode_codex_agent_name(&first),
            Some(("epic".into(), "alpha-beta".into(), "reviewer".into()))
        );
        assert_eq!(
            decode_codex_agent_name(&second),
            Some(("epic".into(), "alpha".into(), "beta-reviewer".into()))
        );
    }

    #[test]
    fn case_distinct_names_remain_distinct_on_case_insensitive_filesystems() {
        let upper = codex_agent_name("epic", "Alpha", "reviewer");
        let lower = codex_agent_name("epic", "alpha", "reviewer");

        assert_ne!(upper.to_lowercase(), lower.to_lowercase());
    }

    #[test]
    fn atomic_write_leaves_one_owned_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let agent = to_codex_agent("epic", "alpha-beta", "reviewer", SAMPLE);
        let payload = render_codex_toml(&agent).unwrap();

        let path =
            write_agent_file(dir.path(), "epic", "alpha-beta", "reviewer", &payload).unwrap();

        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_file());
        assert_eq!(fs::read_to_string(&path).unwrap(), payload);
        assert!(fs::read_dir(dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn windows_atomic_replace_contract_replaces_and_flushes_destination() {
        assert_eq!(windows_replace_flags(), 0x1 | 0x8);
    }
}
