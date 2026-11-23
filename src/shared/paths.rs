use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// `(--git-common-dir, --show-toplevel)` from one `git rev-parse` call, shared
/// by `project_slug` and `legacy_project_slug`. `--git-common-dir` points at
/// the shared git dir even from a linked worktree (so the slug stays stable
/// inside an orbit-{goal_slug} worktree); `--show-toplevel` is only the
/// fallback name. `None` outside a work tree, in a bare repo or without git.
fn git_rev_parse() -> &'static Option<(String, String)> {
    static REV: LazyLock<Option<(String, String)>> = LazyLock::new(|| {
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--git-common-dir", "--show-toplevel"])
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        Some((lines.next()?.to_string(), lines.next()?.to_string()))
    });
    &REV
}

/// Returns a stable slug for the current project: the sanitized name of the
/// git common dir's repo (`<name>/.git`, `<name>.git`, or a submodule's
/// `modules/<name>`).
///
/// One `git rev-parse` call gives a slug that is the same for every subdir and
/// linked worktree of a repo, including separate git dirs and submodules.
/// Falls back to CWD dirname outside git repos.
///
/// - Name is sanitized to `[a-zA-Z0-9_-]` to be safe as a directory component.
/// - Project names must be unique — same-named directories are considered the same project.
pub fn project_slug() -> String {
    static SLUG: LazyLock<String> = LazyLock::new(|| {
        let cwd_path = cwd();

        if let Some((common, top)) = git_rev_parse()
            && let Some(slug) = slug_from_rev_parse(common, top, &cwd_path)
        {
            return slug;
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

/// Slug from `git rev-parse --git-common-dir --show-toplevel` output.
///
/// A git dir named `.git` or any dot-name (`.bare`) gives its parent dir name
/// (normal repo, primary or linked worktree, bare-container layout); any other
/// git dir name (separate git dir `backing.git`, submodule `modules/<name>`)
/// gives its own name minus a trailing `.git`, falling back to the toplevel
/// dirname when nothing is left. `None` means the caller uses its own fallback.
fn slug_from_rev_parse(common_dir: &str, toplevel: &str, cwd: &Path) -> Option<String> {
    let top_name = || {
        Path::new(toplevel)
            .file_name()
            .map(|n| sanitize_slug_name(&n.to_string_lossy()))
            .filter(|n| !n.is_empty())
    };
    let b = common_dir.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/';
    let common = if drive || Path::new(common_dir).is_absolute() {
        PathBuf::from(common_dir)
    } else {
        match std::fs::canonicalize(cwd.join(common_dir)) {
            Ok(p) => p,
            Err(_) => return top_name(),
        }
    };
    let name = common
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with('.') {
        // A repo at the filesystem root has no parent name: stay cwd-independent.
        return match common.parent().and_then(Path::file_name) {
            Some(parent) => Some(sanitize_slug_name(&parent.to_string_lossy())),
            None => top_name().or_else(|| Some("project".into())),
        };
    }
    let stem = name.strip_suffix(".git").unwrap_or(&name);
    if stem.is_empty() {
        top_name()
    } else {
        Some(sanitize_slug_name(stem))
    }
}

/// Pre-0.10 slug for this checkout when it differs from `project_slug()`
/// (separate git dirs, submodules and symlinked `.git` moved to a new slug);
/// `None` otherwise.
pub fn legacy_project_slug() -> Option<String> {
    let (common, top) = git_rev_parse().as_ref()?;
    legacy_slug(common, top, &project_slug())
}

/// Whether this checkout is a submodule, whose pre-0.10 slug was one bucket
/// shared by every submodule.
pub fn is_submodule_checkout() -> bool {
    git_rev_parse()
        .as_ref()
        .is_some_and(|(common, _)| common.contains("/modules/"))
}

/// The full 0.9.x rule: an absolute common dir gives its parent dir name,
/// anything else the toplevel dirname.
fn legacy_slug(common_dir: &str, toplevel: &str, new: &str) -> Option<String> {
    let old = slug_from_git_common_dir(common_dir).unwrap_or_else(|| {
        let name = Path::new(toplevel).file_name();
        sanitize_slug_name(&name.map_or("project".into(), |n| n.to_string_lossy()))
    });
    (old != new).then_some(old)
}

/// Pre-0.10 slug from `git rev-parse --git-common-dir` output, kept only for
/// legacy detection: `Some` only for an
/// absolute path (linked worktree), where the main repo is the parent of the
/// common `.git` dir. A normal checkout prints the relative `.git` (`None`).
///
/// Git on Windows prints drive paths (`C:/repo/.git`), which `is_absolute`
/// rejects on other hosts, so the drive form is matched explicitly.
fn slug_from_git_common_dir(raw: &str) -> Option<String> {
    let b = raw.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/';
    if !drive && !std::path::Path::new(raw).is_absolute() {
        return None;
    }
    // common_git = /main/repo/.git  →  parent = /main/repo
    let repo_root = PathBuf::from(raw).parent()?.to_path_buf();
    let name = repo_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    Some(sanitize_slug_name(&name))
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

/// Data root: `~/.harness`, or `HARNESS_DIR` when set (isolated test
/// environments relocate the whole store without touching the real one).
pub fn harness_root() -> PathBuf {
    if let Ok(d) = std::env::var("HARNESS_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    // Unit tests must never touch the developer's real `~/.harness`: default
    // to a per-process temp dir (callers create it on demand).
    #[cfg(test)]
    return std::env::temp_dir().join(format!("epic-harness-test-{}", std::process::id()));
    #[cfg(not(test))]
    dirs_home().join(".harness")
}

/// Per-project data lives in `~/.harness/projects/{slug}/` — outside the
/// project tree so it never pollutes git and survives project deletion.
pub fn harness_dir() -> PathBuf {
    static DIR: LazyLock<PathBuf> =
        LazyLock::new(|| harness_root().join("projects").join(project_slug()));
    DIR.clone()
}

pub fn harness_projects_root() -> PathBuf {
    harness_root().join("projects")
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

/// Timestamp of the newest observation the last `reflect` round consumed.
///
/// Claude Code fires reflect once per session (SessionEnd), but Codex's `Stop`
/// is turn-scoped: without a watermark every turn re-analyzes the whole day,
/// re-counting the same observations as a fresh session and re-running
/// seeding, stagnation and attribution against data already scored.
pub fn reflect_watermark_file() -> PathBuf {
    harness_dir().join("reflect_watermark.txt")
}

/// Per-project reflection cursor (JSON). Supersedes reflect_watermark.txt,
/// which is still dual-written through 0.9.x for downgrade safety.
pub fn reflect_cursor_file() -> PathBuf {
    harness_dir().join("reflect_cursor.json")
}

/// Date of the last observation-retention sweep (`YYYYMMDD`).
///
/// Keeps the sweep to once a day: Codex's `Stop` is turn-scoped, so an
/// unguarded sweep would issue a DELETE on every turn.
pub fn retention_marker_file() -> PathBuf {
    harness_dir().join("retention_last_sweep.txt")
}

/// Whether an externally supplied project selector is a safe directory name:
/// Unicode letters/digits plus `.`, `_`, `-` (what `sanitize_slug_name` keeps),
/// excluding `.` and `..`. Callers that take a slug from
/// a request must check this before it reaches `resolve_harness_dir`.
pub fn is_valid_project_slug(p: &str) -> bool {
    p != "."
        && p != ".."
        && !p.is_empty()
        && p.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
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
    harness_root().join("global")
}
pub fn global_patterns_file() -> PathBuf {
    global_harness_dir().join("patterns.jsonl")
}

/// Path to the global operational database: `~/.harness/harness.db`
/// Shared across all projects, alongside `memory.db`.
pub fn global_harness_db_path() -> PathBuf {
    harness_root().join("harness.db")
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
    fn linked_worktree_slug_handles_posix_and_windows_drive_paths() {
        assert_eq!(
            slug_from_git_common_dir("/main/repo/.git").as_deref(),
            Some("repo")
        );
        // Git on Windows prints a drive path with forward slashes.
        assert_eq!(
            slug_from_git_common_dir("C:/main/repo/.git").as_deref(),
            Some("repo")
        );
        // Normal checkout: relative, caller falls through to --show-toplevel.
        assert_eq!(slug_from_git_common_dir(".git"), None);
    }

    #[test]
    fn rev_parse_slug_golden_table() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("rootname");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        let slug = |c: &str, t: &str, cwd: &Path| slug_from_rev_parse(c, t, cwd);

        assert_eq!(slug(".git", "/t", &root).as_deref(), Some("rootname"));
        assert_eq!(
            slug("../.git", "/t", &root.join("sub")).as_deref(),
            Some("rootname")
        );
        assert_eq!(
            slug("../../.git", "/t", &root.join("sub/deep")).as_deref(),
            Some("rootname")
        );
        // Linked worktree: absolute common dir of the main checkout.
        assert_eq!(
            slug("/a/main/.git", "/a/wt", &root).as_deref(),
            Some("main")
        );
        // Separate git dir and submodule git dir.
        assert_eq!(
            slug("/x/metadata/backing.git", "/w/proj", &root).as_deref(),
            Some("backing")
        );
        assert_eq!(
            slug("/s/.git/modules/libsub", "/s/libsub", &root).as_deref(),
            Some("libsub")
        );
        // Windows drive form on every host.
        assert_eq!(
            slug("C:/repo/.git", "C:/repo", &root).as_deref(),
            Some("repo")
        );
        assert_eq!(
            slug("/x/store/.git", "/x/store", &root).as_deref(),
            Some("store")
        );
        // Dot-named git dirs use the parent dir name like `.git`.
        assert_eq!(slug("/x/y/.dotfiles", "/w/p", &root).as_deref(), Some("y"));
        assert_eq!(
            slug("/x/alpha/.bare", "/x/alpha/main", &root).as_deref(),
            Some("alpha")
        );
        // Repo at the filesystem root: toplevel dirname, else a literal.
        assert_eq!(slug("/.git", "/w/proj", &root).as_deref(), Some("proj"));
        assert_eq!(slug("/.git", "/", &root).as_deref(), Some("project"));
        // Sanitized on every branch.
        assert_eq!(
            slug("/x/my repo/.git", "/w/p", &root).as_deref(),
            Some("my_repo")
        );
        // Empty stem falls back to the toplevel dirname.
        assert_eq!(slug("/", "/w/proj", &root).as_deref(), Some("proj"));
        // canonicalize failure: toplevel dirname, or None when that is empty too.
        assert_eq!(
            slug(".git", "/w/top", &root.join("missing")).as_deref(),
            Some("top")
        );
        assert_eq!(slug(".git", "/", &root.join("missing")), None);
    }

    #[test]
    fn rev_parse_slug_equals_legacy_for_normal_layouts() {
        // 0.9.1 rule: absolute common dir gives its parent, else toplevel dirname.
        let legacy = |common: &str, top: &str| {
            slug_from_git_common_dir(common).unwrap_or_else(|| {
                sanitize_slug_name(&Path::new(top).file_name().unwrap().to_string_lossy())
            })
        };
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("normal-repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let top = root.to_string_lossy().into_owned();
        let common_abs = format!("{top}/.git");
        let cases = [
            (".git", top.as_str(), root.clone()),
            ("../.git", top.as_str(), root.join("sub")),
            (common_abs.as_str(), "/somewhere/linked-wt", root.clone()),
        ];
        for (common, toplevel, cwd) in cases {
            assert_eq!(
                slug_from_rev_parse(common, toplevel, &cwd).unwrap(),
                legacy(common, toplevel),
                "{common}"
            );
        }
    }

    #[test]
    fn legacy_slug_detects_moved_slugs_only() {
        // Separate git dir: 0.9.1 used the parent dir name, 0.10 uses `backing`.
        assert_eq!(
            legacy_slug("/x/metadata/backing.git", "/w/proj", "backing").as_deref(),
            Some("metadata")
        );
        // Submodule: every one shared `modules` before.
        assert_eq!(
            legacy_slug("/s/.git/modules/libsub", "/s/libsub", "libsub").as_deref(),
            Some("modules")
        );
        // Normal repo and linked worktree: unchanged slug.
        assert_eq!(legacy_slug(".git", "/w/repo", "repo"), None);
        assert_eq!(legacy_slug("/a/main/.git", "/w/wt", "main"), None);
        assert_eq!(
            legacy_slug("/x/alpha/.bare", "/x/alpha/main", "alpha"),
            None
        );
        // Symlinked `.git`: git prints `.git`, 0.9.x used the toplevel dirname.
        assert_eq!(
            legacy_slug(".git", "/w/r2", "realrepo").as_deref(),
            Some("r2")
        );
    }

    #[test]
    fn project_slug_allowlist() {
        for ok in [
            "proj-a",
            "my.proj_1",
            "A",
            "a..b",
            ".hidden",
            "프로젝트",
            "앱-1",
        ] {
            assert!(is_valid_project_slug(ok), "{ok:?}");
        }
        for bad in [
            "",
            ".",
            "..",
            "../x",
            "a/b",
            "a\\b",
            "/abs",
            "C:",
            "a b",
            "a%2fb",
            "a\0b",
            "a?b",
            "a\u{ff0f}b",
        ] {
            assert!(!is_valid_project_slug(bad), "{bad:?}");
        }
    }

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
