use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Returns a stable slug for the current project: `{sanitized-dirname}-{hash6}`.
///
/// Uses git root when available (same slug for all subdirs of a repo and worktrees
/// sharing the same commondir). Falls back to CWD-based hashing outside git repos.
///
/// - Name is sanitized to `[a-zA-Z0-9_-]` to be safe as a directory component.
/// - 6-char hex hash (24 bits) prevents collisions between same-named projects.
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
                    let safe_name = sanitize_slug_name(&name);

                    // For worktrees, use git common dir to get a stable identity
                    // (worktrees share the same commondir).
                    let hash_input = git_common_dir(&root_path).unwrap_or_else(|| root.clone());
                    return format!("{}-{:06x}", safe_name, hash_path(&hash_input));
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
        let safe_name = sanitize_slug_name(&name);
        let full = cwd_path.to_string_lossy();
        format!("{}-{:06x}", safe_name, hash_path(&full))
    });
    SLUG.clone()
}

/// Sanitize a name for use as a slug component.
fn sanitize_slug_name(name: &str) -> String {
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

/// Compute a 24-bit hash of a path string (6 hex chars).
fn hash_path(s: &str) -> u32 {
    let mut h: u32 = 0;
    for b in s.bytes() {
        h = h.wrapping_shl(5).wrapping_sub(h).wrapping_add(b as u32);
    }
    h & 0x00ff_ffff
}

/// Get the git common directory for a worktree root.
/// Returns the commondir path which is shared across all worktrees of the same repo.
fn git_common_dir(git_root: &Path) -> Option<String> {
    let git_dir = git_root.join(".git");
    // For worktrees, .git is a file pointing to the real git dir.
    if git_dir.is_file() {
        if let Ok(content) = std::fs::read_to_string(&git_dir) {
            // Format: "gitdir: /path/to/.git/worktrees/<name>"
            if let Some(gitdir_path) = content.strip_prefix("gitdir: ") {
                let gitdir_path = gitdir_path.trim();
                // The commondir file is in the worktree-specific git dir.
                let commondir_path = PathBuf::from(gitdir_path).join("commondir");
                if let Ok(common) = std::fs::read_to_string(&commondir_path) {
                    let common = common.trim();
                    // commondir may be relative to the gitdir's parent.
                    if PathBuf::from(common).is_absolute() {
                        return Some(common.to_string());
                    }
                    if let Some(parent) = PathBuf::from(gitdir_path).parent() {
                        return Some(parent.join(common).to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    // Regular repo: .git is the directory itself.
    None
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
