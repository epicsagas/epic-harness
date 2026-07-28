use std::path::PathBuf;
use std::sync::LazyLock;

pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn stable_path_hash(path: &std::path::Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let text = path.to_string_lossy();
    #[cfg(windows)]
    let text = text.to_lowercase();
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn legacy_project_slug_for_root(root: &std::path::Path) -> String {
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let slug = sanitize_slug_name(&name);
    if slug.is_empty() {
        "project".into()
    } else {
        slug
    }
}

fn project_slug_for_root(root: &std::path::Path) -> String {
    format!(
        "{}-{:012x}",
        legacy_project_slug_for_root(root),
        stable_path_hash(root) & 0xffff_ffff_ffff
    )
}

fn canonical_project_root() -> PathBuf {
    let cwd_path = cwd();

    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        && output.status.success()
    {
        let common_git = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if common_git.is_absolute()
            && let Some(root) = common_git.parent()
        {
            return root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        }
    }

    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        && output.status.success()
    {
        let root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if !root.as_os_str().is_empty() {
            return root.canonicalize().unwrap_or(root);
        }
    }

    cwd_path.canonicalize().unwrap_or(cwd_path)
}

/// Returns a stable collision-resistant slug for the canonical project root.
pub fn project_slug() -> String {
    static SLUG: LazyLock<String> =
        LazyLock::new(|| project_slug_for_root(&canonical_project_root()));
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
fn list_harness_project_slugs_in(root: &std::path::Path) -> Vec<String> {
    if !root.is_dir() {
        return vec![];
    }
    let mut slugs: Vec<String> = std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    slugs.sort();
    slugs
}

pub fn list_harness_project_slugs() -> Vec<String> {
    list_harness_project_slugs_in(&harness_projects_root())
}

fn resolve_external_harness_dir_in(root: &std::path::Path, slug: &str) -> std::io::Result<PathBuf> {
    if slug.is_empty() || matches!(slug, "." | "..") || slug.contains('/') || slug.contains('\\') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid harness project slug: {slug}"),
        ));
    }
    let candidate = root.join(slug);
    let metadata = candidate.symlink_metadata()?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "harness project is not a regular directory: {}",
                candidate.display()
            ),
        ));
    }
    if !list_harness_project_slugs_in(root)
        .iter()
        .any(|known| known == slug)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("unknown harness project slug: {slug}"),
        ));
    }
    let canonical_root = root.canonicalize()?;
    let canonical_project = candidate.canonicalize()?;
    if !canonical_project.starts_with(&canonical_root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "harness project escapes projects root: {}",
                candidate.display()
            ),
        ));
    }
    Ok(canonical_project)
}

/// Resolve an externally supplied project slug to an exact, existing project
/// directory without following a project-directory symlink or escaping the
/// canonical projects root.
pub fn resolve_external_harness_dir(slug: &str) -> std::io::Result<PathBuf> {
    resolve_external_harness_dir_in(&harness_projects_root(), slug)
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

/// Per-project edit-manifest log for the HarnessX falsifiability contract
/// (Table 9). Each shipped edit appends its manifest here.
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

    #[test]
    fn same_basename_repositories_have_distinct_project_slugs() {
        let first = project_slug_for_root(std::path::Path::new("/work/acme/service"));
        let second = project_slug_for_root(std::path::Path::new("/work/other/service"));

        assert_ne!(first, second);
        assert!(first.starts_with("service-"));
        assert!(second.starts_with("service-"));
    }

    #[test]
    fn legacy_basename_remains_available_for_explicit_merge() {
        let root = std::path::Path::new("/work/acme/service");
        assert_eq!(legacy_project_slug_for_root(root), "service");
        assert_ne!(
            legacy_project_slug_for_root(root),
            project_slug_for_root(root)
        );
    }

    #[test]
    fn external_project_resolver_requires_an_exact_known_slug() {
        let root = tempfile::tempdir().unwrap();
        let known = root.path().join("known-project");
        std::fs::create_dir(&known).unwrap();

        assert_eq!(
            resolve_external_harness_dir_in(root.path(), "known-project").unwrap(),
            known.canonicalize().unwrap()
        );
        assert!(resolve_external_harness_dir_in(root.path(), "unknown-project").is_err());
    }

    #[test]
    fn external_project_resolver_rejects_empty_and_path_syntax() {
        let root = tempfile::tempdir().unwrap();

        for slug in [
            "",
            ".",
            "..",
            "../project",
            "project/child",
            "project\\child",
        ] {
            assert_eq!(
                resolve_external_harness_dir_in(root.path(), slug)
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidInput,
                "{slug:?} must be rejected as invalid input"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn external_project_resolver_rejects_a_symlinked_project_directory() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("linked-project")).unwrap();

        assert_eq!(
            resolve_external_harness_dir_in(root.path(), "linked-project")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
}
