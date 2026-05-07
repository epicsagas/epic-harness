use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use super::common::*;
use super::mem::store;
use super::telemetry::Telemetry;

/// Atomically acquire a session lock file.
///
/// Returns `true` when this call created the file (this process owns the lock).
/// Returns `false` when the file already exists (`AlreadyExists`) or on any
/// other I/O error (safe fallback — treat as "already running").
fn acquire_session_lock(lock: &Path) -> bool {
    let hd = lock
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(harness_dir);
    let _ = fs::create_dir_all(&hd);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock)
    {
        Ok(_) => true,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => false,
        Err(_) => false,
    }
}

const BANNER: &[&str] = &[
    "",
    "  ┌─┐┌─┐┬┌─┐   ┬ ┬┌─┐┬─┐┌┐┌┌─┐┌─┐┌─┐",
    "  ├┤ ├─┘││     ├─┤├─┤├┬┘│││├┤ └─┐└─┐",
    "  └─┘┴  ┴└─┘   ┴ ┴┴ ┴┴└─┘└┘└─┘└─┘└─┘",
    "          6 commands · auto skills · self-evolving",
    "",
];

const STACK_FILES: &[(&str, &str)] = &[
    ("package.json", "Node.js"),
    ("go.mod", "Go"),
    ("pyproject.toml", "Python"),
    ("Cargo.toml", "Rust"),
    ("build.gradle", "Java/Kotlin"),
    ("Gemfile", "Ruby"),
    ("pom.xml", "Java (Maven)"),
    ("composer.json", "PHP"),
];

fn apply_cold_start_presets(stacks: &[&str]) -> u32 {
    let evolved = evolved_dir();
    let mut applied = 0u32;

    for &stack in stacks {
        let skills: &[(&str, &str)] = match stack {
            "Node.js" => &[
                (
                    "evo-ts-care",
                    include_str!("../../presets/node/evo-ts-care.md"),
                ),
                (
                    "evo-fix-build-fail",
                    include_str!("../../presets/node/evo-fix-build-fail.md"),
                ),
            ],
            "Go" => &[(
                "evo-go-care",
                include_str!("../../presets/go/evo-go-care.md"),
            )],
            "Python" => &[(
                "evo-py-care",
                include_str!("../../presets/python/evo-py-care.md"),
            )],
            "Rust" => &[(
                "evo-rs-care",
                include_str!("../../presets/rust/evo-rs-care.md"),
            )],
            _ => continue,
        };

        for &(name, content) in skills {
            let skill_dir = evolved.join(name);
            if skill_dir.is_dir() {
                continue;
            }
            ensure_dir(&skill_dir);
            let _ = fs::write(skill_dir.join("SKILL.md"), content);
            applied += 1;
        }
    }
    applied
}

fn get_cross_project_hints() -> Vec<String> {
    if !cross_project_file().is_file() {
        return vec![];
    }
    if !global_patterns_file().is_file() {
        return vec![];
    }

    let project_name = cwd()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let records = read_jsonl(&global_patterns_file());
    let other: Vec<_> = records
        .iter()
        .filter(|r| r.get("project").and_then(|p| p.as_str()) != Some(&project_name))
        .collect();

    if other.is_empty() {
        return vec![];
    }

    let mut weak_tool_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    for r in other.iter().rev().take(20) {
        if let Some(tools) = r.get("weak_tools").and_then(|v| v.as_array()) {
            for t in tools.iter().filter_map(|v| v.as_str()) {
                *weak_tool_counts.entry(t.to_string()).or_default() += 1;
            }
        }
    }

    let mut hints = vec![];
    let frequent: Vec<_> = weak_tool_counts.iter().filter(|(_, c)| **c >= 2).collect();
    if !frequent.is_empty() {
        let parts: Vec<String> = frequent
            .iter()
            .map(|(t, c)| format!("{t} weak in {c} projects"))
            .collect();
        hints.push(format!("Cross-project: {}", parts.join(", ")));
    }
    hints
}

pub fn run(_input: &HookInput) -> i32 {
    if !should_run(PROFILE_RESUME) {
        return 0;
    }
    // Guard: SessionStart fires multiple times per session in Claude Code.
    // Use a per-session lock file (keyed by date+pid) to run exactly once.
    // `acquire_session_lock` uses O_CREAT|O_EXCL — atomically prevents the
    // TOCTOU race that the old exists()+write() pattern introduced.
    let lock = harness_dir().join(format!("resume.{}.lock", session_id()));
    if !acquire_session_lock(&lock) {
        return 0;
    }

    let wd = cwd();

    // Migrate legacy .harness/ from project dir to ~/.harness/projects/{slug}/
    let local = local_harness_dir();
    let is_real_dir = local
        .symlink_metadata()
        .map(|m| m.file_type().is_dir())
        .unwrap_or(false);
    if is_real_dir && !harness_exists() {
        ensure_dir(&harness_dir());
        let copied = copy_dir_counted(&local, &harness_dir());
        if copied.errors > 0 {
            hint(
                "resume",
                &format!(
                    "Migration partial: {}/{} files copied — check {}",
                    copied.ok,
                    copied.ok + copied.errors,
                    harness_dir().display()
                ),
            );
        } else {
            // Remove migrated local dir; guard-rules.yaml lives at project root
            // (.harness/guard-rules.yaml) — move it up before deleting the dir.
            let guard_src = local.join("guard-rules.yaml");
            let guard_dst = cwd().join(".harness").join("guard-rules.yaml");
            if guard_src.exists() && !guard_dst.exists() {
                // guard-rules is *inside* local — it will be deleted with the dir.
                // Copy it to a standalone location the user can check into git.
                let root_guard = cwd().join("guard-rules.yaml");
                if !root_guard.exists() {
                    let _ = std::fs::copy(&guard_src, &root_guard);
                }
            }
            let _ = std::fs::remove_dir_all(&local);
            hint(
                "resume",
                &format!(
                    "Migrated .harness/ → {} ({} files). Removed project-local .harness/.",
                    harness_dir().display(),
                    copied.ok
                ),
            );
        }
    }

    // Auto-init ~/.harness/projects/{slug}/
    if !harness_exists() {
        for line in BANNER {
            raw(line);
        }
        ensure_dir(&harness_dir());
        ensure_dir(&obs_dir());
        ensure_dir(&sessions_dir());
        ensure_dir(&memory_dir());
        ensure_dir(&evolved_dir());
        hint(
            "resume",
            &format!(
                "Initialized {} — Ring 3 evolution loop active",
                harness_dir().display()
            ),
        );
    }

    // Seed the default org/team whenever orgs dir is empty (idempotent)
    let default_org = crate::hooks::team::store::default_org();
    if crate::hooks::team::store::install_default_team_if_needed(&default_org) {
        hint(
            "resume",
            &format!(
                "Default team 'core' created in org '{default_org}' — run 'epic team sync core' to activate"
            ),
        );
    }

    // 1. Latest session snapshot
    let mut snaps = list_files(&sessions_dir(), ".json");
    snaps.sort();
    if let Some(latest_name) = snaps.last() {
        let snap: SessionSnapshot = read_json(
            &sessions_dir().join(latest_name),
            SessionSnapshot {
                timestamp: String::new(),
                snap_type: String::new(),
                summary: String::new(),
                pending_tasks: vec![],
                context_usage: None,
                pipeline_state: None,
            },
        );
        if !snap.summary.is_empty() {
            hint("resume", &format!("Previous: {}", snap.summary));
        }
        if !snap.pending_tasks.is_empty() {
            hint(
                "resume",
                &format!("Pending: {}", snap.pending_tasks.join(", ")),
            );
        }
    }

    // 2. Eval metrics
    let metrics: Metrics = read_json(&metrics_file(), default_metrics());
    if metrics.total_sessions > 0 {
        let score_str = metrics
            .score_history
            .last()
            .map(|e| {
                format!(
                    "{}% success, avg_score={}",
                    (e.success_rate * 100.0) as u32,
                    e.avg_score
                )
            })
            .unwrap_or_else(|| {
                format!("{}% avg success", (metrics.avg_success_rate * 100.0) as u32)
            });

        hint(
            "resume",
            &format!(
                "Last session: {score_str} | trend={} ({} sessions)",
                metrics.trend, metrics.total_sessions
            ),
        );

        if metrics.stagnation_count > 0 {
            hint(
                "resume",
                &format!(
                    "Stagnation: {} session(s) without improvement",
                    metrics.stagnation_count
                ),
            );
        }

        if let Some(last) = metrics.score_history.last() {
            let dims = &last.dimension_averages;
            let mut weak = vec![];
            if dims.tool_success < 0.7 {
                weak.push(format!("tool_success={}", dims.tool_success));
            }
            if dims.output_quality < 0.7 {
                weak.push(format!("output_quality={}", dims.output_quality));
            }
            if !weak.is_empty() {
                hint("resume", &format!("Weak dimensions: {}", weak.join(", ")));
            }
        }

        // Session handoff (#10)
        if let Some(ctx) = &metrics.last_error_context {
            hint("resume", &format!("Last errors: {ctx}"));
        }

        // Skill attribution (#6)
        let effective: Vec<_> = metrics
            .skill_attribution
            .values()
            .filter(|a| a.sessions_active >= 2 && a.avg_score_with > a.avg_score_without + 0.02)
            .collect();
        if !effective.is_empty() {
            let names: Vec<_> = effective.iter().map(|s| s.skill_name.as_str()).collect();
            hint("resume", &format!("Top skills: {}", names.join(", ")));
        }
    }

    // 3. Evolved skills
    let evolved = list_dirs(&evolved_dir());
    if !evolved.is_empty() {
        hint("resume", &format!("Evolved skills: {}", evolved.join(", ")));
    }

    // 4. Cold-start presets (#1)
    let stacks: Vec<&str> = STACK_FILES
        .iter()
        .filter(|(f, _)| wd.join(f).is_file())
        .map(|(_, s)| *s)
        .collect();

    if evolved.is_empty() && metrics.total_sessions == 0 && !stacks.is_empty() {
        let applied = apply_cold_start_presets(&stacks);
        if applied > 0 {
            hint(
                "resume",
                &format!(
                    "Cold-start: applied {applied} preset skill(s) for {}",
                    stacks.join(", ")
                ),
            );
        }
    }

    // 5. Memory
    let mem_files = list_files(&memory_dir(), ".md");
    if !mem_files.is_empty() {
        hint("resume", &format!("Memory: {} file(s)", mem_files.len()));
    }

    // 5a. Unified memory context: if ~/.harness/memory/ exists, emit relevant
    //     entries for the current project to stderr so the agent can ingest them.
    //     Runs `epic-harness mem context --project <slug>` non-blocking if available.
    {
        let unified_mem = global_harness_dir()
            .parent()
            .map(|p| p.join("memory"))
            .unwrap_or_default();
        if unified_mem.is_dir() {
            let slug = project_slug();
            // Attempt to surface mem context inline (best-effort, non-fatal)
            match std::process::Command::new("epic-harness")
                .args(["mem", "context", "--project", &slug])
                .output()
            {
                Ok(out) if !out.stdout.is_empty() => {
                    let ctx = String::from_utf8_lossy(&out.stdout);
                    eprintln!("[harness/mem] Relevant memory for '{slug}':");
                    for line in ctx.lines().take(20) {
                        eprintln!("  {line}");
                    }
                }
                _ => {} // binary not yet installed or no entries — silently skip
            }
        }
    }

    // 5b. Knowledge graph recall: smart recall with composite scoring
    {
        let slug = project_slug();
        let scored = store::smart_recall(Some(&slug), None, 10);
        let important: Vec<_> = scored
            .iter()
            .filter(|sn| {
                let t = sn.node.frontmatter.node_type.as_str();
                matches!(
                    t,
                    "decision" | "resolution" | "pattern" | "error" | "concept"
                )
            })
            .take(7)
            .collect();
        if !important.is_empty() {
            hint("resume", "Knowledge graph — relevant memories:");
            for sn in &important {
                let fm = &sn.node.frontmatter;
                hint(
                    "resume",
                    &format!(
                        "  [{}] {} (importance={:.1}, score={:.2})",
                        fm.node_type, fm.title, fm.importance, sn.score
                    ),
                );
            }
        }
    }

    // 5c. Memory decay: gradual importance decay (30+ days untouched → 10% decay, floor 0.05)
    if let Ok(decayed) = store::decay_importance(30, 0.9, 0.05)
        && decayed > 0
    {
        hint(
            "resume",
            &format!("Memory decay: decayed importance for {decayed} node(s)"),
        );
    }
    // Also tag truly ancient nodes as stale (180+ days)
    if let Ok(staled) = store::tag_stale_nodes(180)
        && staled > 0
    {
        hint(
            "resume",
            &format!("Memory cleanup: tagged {staled} ancient node(s) as stale"),
        );
    }

    // 6. Stack
    if !stacks.is_empty() {
        hint("resume", &format!("Stack: {}", stacks.join(", ")));
    }

    // 7. Team
    let team_agents = list_files(&team_dir().join("agents"), ".md");
    if !team_agents.is_empty() {
        let names: Vec<String> = team_agents.iter().map(|a| a.replace(".md", "")).collect();
        hint("resume", &format!("Team: {}", names.join(", ")));
    }

    // 8. Cross-project hints (#2)
    for h in get_cross_project_hints() {
        hint("resume", &h);
    }

    // 9. Orchestration state restoration — inject active agent summary after
    //    context compaction so orchestration state survives session restore.
    if let Some(summary) = restore_orchestration_state(&harness_dir()) {
        hint("resume", &summary);
    }

    // 10. Telemetry — session_started event (consent already ensured in main.rs)
    Telemetry::init().track_session_started();

    0
}

// ── Orchestration State Restoration ──────────────────

/// Checks for an active orchestration run and builds a summary of all agent
/// states. Returns `None` when orchestration is disabled, no run exists, or
/// the run is not in "running" status. Must not fail on partially present
/// state files.
fn restore_orchestration_state(harness_dir: &Path) -> Option<String> {
    let run_path = harness_dir.join("orchestrator").join("run.json");
    if !run_path.exists() {
        return None;
    }

    // Only activate when EPIC_ORCHESTRATION is enabled
    if std::env::var("EPIC_ORCHESTRATION").unwrap_or_default() != "enabled" {
        return None;
    }

    let run_json = std::fs::read_to_string(&run_path).ok()?;
    let run: serde_json::Value = serde_json::from_str(&run_json).ok()?;

    if run["status"].as_str()? != "running" {
        return None;
    }

    // Build a summary of all agent states
    let agents_dir = harness_dir.join("orchestrator").join("agents");
    let mut agent_summaries = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let status_path = entry.path().join("status.json");
            if let Ok(status_json) = std::fs::read_to_string(&status_path)
                && let Ok(status) = serde_json::from_str::<serde_json::Value>(&status_json)
            {
                agent_summaries.push(format!(
                    "- {}: {} (started: {})",
                    entry.file_name().to_string_lossy(),
                    status["status"].as_str().unwrap_or("unknown"),
                    status["started_at"].as_str().unwrap_or("n/a")
                ));
            }
        }
    }

    let run_id = run["id"].as_str().unwrap_or("unknown");
    Some(format!(
        "## Orchestration State Restored\nRun ID: {}\nStatus: running\nAgents:\n{}",
        run_id,
        agent_summaries.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper: return a unique lock path inside a temp dir for this test.
    fn temp_lock(dir: &std::path::Path, name: &str) -> PathBuf {
        dir.join(format!("{name}.lock"))
    }

    #[test]
    fn acquire_session_lock_first_call_succeeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock = temp_lock(dir.path(), "session_first");
        assert!(
            acquire_session_lock(&lock),
            "first acquire must return true"
        );
        assert!(lock.exists(), "lock file must be created");
    }

    #[test]
    fn acquire_session_lock_second_call_returns_false() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock = temp_lock(dir.path(), "session_second");

        // First call: acquires lock.
        assert!(acquire_session_lock(&lock));
        // Second call: lock file already exists — must return false (TOCTOU-safe).
        assert!(
            !acquire_session_lock(&lock),
            "second acquire on same lock must return false"
        );
    }

    // ── restore_orchestration_state ──────────────────

    #[test]
    fn orchestration_returns_none_when_no_orchestrator_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            restore_orchestration_state(dir.path()),
            None,
            "must return None when orchestrator dir does not exist"
        );
    }

    #[test]
    fn orchestration_returns_none_when_env_not_set() {
        let dir = tempfile::tempdir().expect("tempdir");
        let orch_dir = dir.path().join("orchestrator");
        fs::create_dir_all(&orch_dir).expect("create orchestrator dir");
        fs::write(
            orch_dir.join("run.json"),
            r#"{"id":"run-1","status":"running"}"#,
        )
        .expect("write run.json");

        // Ensure env is not set
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }
        assert_eq!(
            restore_orchestration_state(dir.path()),
            None,
            "must return None when EPIC_ORCHESTRATION is not set"
        );
    }

    #[test]
    fn orchestration_returns_none_when_status_not_running() {
        let dir = tempfile::tempdir().expect("tempdir");
        let orch_dir = dir.path().join("orchestrator");
        fs::create_dir_all(&orch_dir).expect("create orchestrator dir");
        fs::write(
            orch_dir.join("run.json"),
            r#"{"id":"run-2","status":"completed"}"#,
        )
        .expect("write run.json");

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = restore_orchestration_state(dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        assert_eq!(
            result, None,
            "must return None when run status is not running"
        );
    }

    #[test]
    fn orchestration_returns_summary_when_active() {
        let dir = tempfile::tempdir().expect("tempdir");
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");
        let builder_dir = agents_dir.join("builder");
        let reviewer_dir = agents_dir.join("reviewer");
        fs::create_dir_all(&builder_dir).expect("create builder dir");
        fs::create_dir_all(&reviewer_dir).expect("create reviewer dir");

        fs::write(
            orch_dir.join("run.json"),
            r#"{"id":"run-42","status":"running"}"#,
        )
        .expect("write run.json");

        fs::write(
            builder_dir.join("status.json"),
            r#"{"status":"idle","started_at":"2026-05-07T10:00:00Z"}"#,
        )
        .expect("write builder status");
        fs::write(
            reviewer_dir.join("status.json"),
            r#"{"status":"working","started_at":"2026-05-07T10:01:00Z"}"#,
        )
        .expect("write reviewer status");

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = restore_orchestration_state(dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        let summary = result.expect("must return Some when active orchestration exists");
        assert!(
            summary.contains("run-42"),
            "summary must contain run ID: {summary}"
        );
        assert!(
            summary.contains("Status: running"),
            "summary must contain status: {summary}"
        );
        assert!(
            summary.contains("builder: idle"),
            "summary must contain builder agent: {summary}"
        );
        assert!(
            summary.contains("reviewer: working"),
            "summary must contain reviewer agent: {summary}"
        );
    }

    #[test]
    fn orchestration_handles_missing_agent_status_gracefully() {
        let dir = tempfile::tempdir().expect("tempdir");
        let orch_dir = dir.path().join("orchestrator");
        let agents_dir = orch_dir.join("agents");
        // Create agent dir without status.json
        let builder_dir = agents_dir.join("builder");
        fs::create_dir_all(&builder_dir).expect("create builder dir");

        fs::write(
            orch_dir.join("run.json"),
            r#"{"id":"run-99","status":"running"}"#,
        )
        .expect("write run.json");

        unsafe {
            std::env::set_var("EPIC_ORCHESTRATION", "enabled");
        }
        let result = restore_orchestration_state(dir.path());
        unsafe {
            std::env::remove_var("EPIC_ORCHESTRATION");
        }

        let summary = result.expect("must return Some even with missing agent status files");
        assert!(
            summary.contains("run-99"),
            "summary must contain run ID: {summary}"
        );
        // No agent lines should be present since builder has no status.json
        assert!(
            !summary.contains("builder:"),
            "summary must not list builder when its status.json is missing: {summary}"
        );
    }
}
