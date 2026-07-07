use std::path::PathBuf;
use std::sync::LazyLock;

pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Returns a stable slug for the current project: the sanitized git root dirname.
///
/// Uses git root when available (same slug for all subdirs of a repo).
/// Falls back to CWD dirname outside git repos.
///
/// - Name is sanitized to `[a-zA-Z0-9_-]` to be safe as a directory component.
/// - Project names must be unique — same-named directories are considered the same project.
pub fn project_slug() -> String {
    static SLUG: LazyLock<String> = LazyLock::new(|| {
        let cwd_path = cwd();

        // Prefer --git-common-dir: returns an absolute path in linked worktrees,
        // pointing to the main repo's .git directory, so the slug stays stable
        // even when the hook fires from inside an orbit-{goal_slug} worktree.
        // In a normal (non-worktree) checkout it returns the relative string ".git",
        // in which case we fall back to --show-toplevel as before.
        if let Ok(out) = std::process::Command::new("git")
            .args(["rev-parse", "--git-common-dir"])
            .output()
        {
            if out.status.success() {
                let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
                // Absolute path → linked worktree; derive project root from common .git dir.
                if raw.starts_with('/') {
                    let common_git = PathBuf::from(&raw);
                    // common_git = /main/repo/.git  →  parent = /main/repo
                    if let Some(repo_root) = common_git.parent() {
                        let name = repo_root
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "project".into());
                        return sanitize_slug_name(&name);
                    }
                }
            }
        }

        // Normal repo (--git-common-dir returned ".git"): use --show-toplevel.
        if let Ok(git_root) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if git_root.status.success() {
                let root = String::from_utf8_lossy(&git_root.stdout).trim().to_string();
                if !root.is_empty() {
                    let root_path = PathBuf::from(&root);
                    let name = root_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "project".into());
                    return sanitize_slug_name(&name);
                }
            }
        }

        // Fallback: CWD-based (non-git directories).
        let name = cwd_path
            .components()
            .filter_map(|c| {
                if let std::path::Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                }
            })
            .next_back()
            .unwrap_or("project")
            .to_string();
        sanitize_slug_name(&name)
    });
    SLUG.clone()
}

/// Sanitize a name for use as a slug component.
pub(crate) fn sanitize_slug_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Per-project data lives in `~/.harness/projects/{slug}/` — outside the
/// project tree so it never pollutes git and survives project deletion.
pub fn harness_dir() -> PathBuf {
    static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
        dirs_home()
            .join(".harness")
            .join("projects")
            .join(project_slug())
    });
    DIR.clone()
}

pub fn harness_projects_root() -> PathBuf {
    dirs_home().join(".harness").join("projects")
}

/// Lists all project slugs that have harness data directories.
pub fn list_harness_project_slugs() -> Vec<String> {
    let root = harness_projects_root();
    if !root.is_dir() {
        return vec![];
    }
    let mut slugs: Vec<String> = std::fs::read_dir(&root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    slugs.sort();
    slugs
}

/// Returns the harness data directory for a given project slug.
pub fn harness_dir_for_slug(slug: &str) -> PathBuf {
    harness_projects_root().join(slug)
}

/// Legacy project-local path used for migration detection only.
pub(crate) fn local_harness_dir() -> PathBuf {
    cwd().join(".harness")
}

pub fn obs_dir() -> PathBuf {
    harness_dir().join("obs")
}
pub fn sessions_dir() -> PathBuf {
    harness_dir().join("sessions")
}
pub fn memory_dir() -> PathBuf {
    harness_dir().join("memory")
}
pub fn evolved_dir() -> PathBuf {
    harness_dir().join("evolved")
}
pub fn evolved_backup_dir() -> PathBuf {
    harness_dir().join("evolved_backup")
}
pub fn team_dir() -> PathBuf {
    harness_dir().join("team")
}
pub fn orbit_dir() -> PathBuf {
    harness_dir().join("orbit")
}

pub fn metrics_file() -> PathBuf {
    harness_dir().join("metrics.json")
}
pub fn evolution_file() -> PathBuf {
    harness_dir().join("evolution.jsonl")
}

/// Per-project solved-task registry for the seesaw constraint (R5).
pub fn project_seesaw_path() -> PathBuf {
    harness_dir().join("seesaw.json")
}

/// Per-project variant pool for ensemble routing / variant isolation (R6).
pub fn variant_pool_path() -> PathBuf {
    harness_dir().join("variants.json")
}

/// Session-start state written by `resume` (SessionStart) and read by
/// `reflect` (SessionEnd) so the holdout partition uses the same `date` on
/// both ends — otherwise a session spanning UTC midnight attributes an
/// active-injected skill to the holdout arm (or vice versa).
pub fn session_start_file() -> PathBuf {
    harness_dir().join("session_start.json")
}

/// Per-project edit-manifest log for the HarnessX falsifiability contract
/// (Table 9). Each shipped edit appends its manifest here.
///
/// STATUS (2026-06): write path is wired (reflect appends on every shipped
/// edit). The READ path — a Critic that tails recent manifests to verify the
/// prior round's predictions held — is a deferred follow-up; today the Critic
/// only consults the in-round reward-hacking flag, not historical manifests.
/// So this ledger currently accumulates without a consumer; the cross-round
/// falsifiability loop is not yet closed.
pub fn manifests_file() -> PathBuf {
    harness_dir().join("manifests.jsonl")
}

/// Per-project pending-synthesis manifest log. Each seeded skill eligible for
/// host-agent synthesis gets one record here; `epic-harness evolve
/// accept-synth` consumes them. Unconsumed records leave the template skill
/// body in place — synthesis can only improve a skill, never block seeding.
/// (Dedicated file: `EditManifest` in `manifests.jsonl` is the falsifiability
/// ledger and shares no fields with a pending-synthesis record.)
pub fn pending_synth_file() -> PathBuf {
    harness_dir().join("pending_synth.jsonl")
}

/// Resolve the per-project harness dir for a request. Falls back to the
/// CWD-derived dir when `project` is `None`/empty (preserves existing callers
/// that don't pass a project). Used by the dashboard read-path to scope
/// file-backed readers (seesaw/variants/snapshot/manifests) to the selected
/// project.
pub fn resolve_harness_dir(project: Option<&str>) -> PathBuf {
    match project {
        Some(p) if !p.is_empty() => harness_dir_for_slug(p),
        _ => harness_dir(),
    }
}

pub fn project_seesaw_path_for(project: Option<&str>) -> PathBuf {
    resolve_harness_dir(project).join("seesaw.json")
}
pub fn variant_pool_path_for(project: Option<&str>) -> PathBuf {
    resolve_harness_dir(project).join("variants.json")
}
pub fn evolved_dir_for(project: Option<&str>) -> PathBuf {
    resolve_harness_dir(project).join("evolved")
}
pub fn manifests_file_for(project: Option<&str>) -> PathBuf {
    resolve_harness_dir(project).join("manifests.jsonl")
}

/// guard-rules.yaml stays in the project tree only if the user/team explicitly
/// created it there. Otherwise, we default to the per-project global directory
/// to keep the project tree clean.
pub fn guard_rules_file() -> PathBuf {
    let local = local_harness_dir().join("guard-rules.yaml");
    if local.is_file() {
        local
    } else {
        harness_dir().join("guard-rules.yaml")
    }
}

pub fn global_harness_dir() -> PathBuf {
    dirs_home().join(".harness").join("global")
}
pub fn global_patterns_file() -> PathBuf {
    global_harness_dir().join("patterns.jsonl")
}

/// Path to the global operational database: `~/.harness/harness.db`
/// Shared across all projects, alongside `memory.db`.
pub fn global_harness_db_path() -> PathBuf {
    dirs_home().join(".harness").join("harness.db")
}

/// Opt-in marker lives in the global dir (not per-project).
pub fn cross_project_file() -> PathBuf {
    global_harness_dir().join(".cross-project-enabled")
}

pub(crate) fn dirs_home() -> PathBuf {
    // Check HOME (Linux/macOS) then USERPROFILE (Windows)
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        return PathBuf::from(up);
    }
    // Windows fallback: HOMEDRIVE + HOMEPATH
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return PathBuf::from(format!("{}{}", drive, path));
    }

    // Fail loudly if home directory cannot be determined.
    // Falling back to /tmp is insecure as it's typically world-readable.
    panic!("[harness] FATAL: Home directory not detected. Please set HOME or USERPROFILE.");
}

pub fn claude_config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR")
        && !d.is_empty()
    {
        return PathBuf::from(d);
    }
    dirs_home().join(".claude")
}

pub fn claude_json_path() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_SETTINGS_PATH")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR")
        && !d.is_empty()
    {
        return PathBuf::from(&d)
            .parent()
            .map(|p| p.join(".claude.json"))
            .unwrap_or_else(|| PathBuf::from(d).join(".claude.json"));
    }
    dirs_home().join(".claude.json")
}

#[allow(dead_code)]
pub fn claude_plugin_cache_dir() -> PathBuf {
    if let Ok(d) = std::env::var("CLAUDE_CODE_PLUGIN_CACHE_DIR")
        && !d.is_empty()
    {
        return PathBuf::from(d);
    }
    claude_config_dir().join("plugins")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_path_helpers_resolve_by_slug() {
        // AC1: file-backed reader paths scope to the requested project.
        // Distinct slugs → distinct dirs; None/"" fall back to the CWD default.
        let a = project_seesaw_path_for(Some("proj-a"));
        let b = project_seesaw_path_for(Some("proj-b"));
        assert_ne!(a, b);
        assert!(a.to_string_lossy().contains("proj-a"));
        assert!(b.to_string_lossy().contains("proj-b"));
        // None and "" both resolve to the CWD harness_dir.
        assert_eq!(
            project_seesaw_path_for(None),
            project_seesaw_path_for(Some(""))
        );
    }
}
