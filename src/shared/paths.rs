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

        // Try git first: rev-parse --show-toplevel gives the worktree root.
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
