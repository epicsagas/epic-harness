use std::fs;

use super::common::*;

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

    0
}
