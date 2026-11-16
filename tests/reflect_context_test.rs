//! `epic reflect --context` through the real binary. Every run uses a temp HOME,
//! HARNESS_DIR and cwd; database rows are synthetic.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use epic_harness::shared::evolution::EvolutionRecord;
use epic_harness::shared::types::SessionSnapshot;
use epic_harness::store::{evolution, sessions};

const SLUG: &str = "synthetic-project";

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Env {
    /// `via_link` reaches HARNESS_DIR and HOME through a symlinked parent, so
    /// their canonical form differs from the configured path.
    fn new(via_link: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let real = root.join("real");
        let base = if via_link {
            symlink(&real, root.join("link")).unwrap_or(());
            root.join("link")
        } else {
            real.clone()
        };
        let cwd = root.join(SLUG);
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(real.join("home/.config/epic-harness")).unwrap();
        fs::write(
            real.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
        fs::create_dir_all(real.join("harness/projects").join(SLUG)).unwrap();
        Self {
            _temp: temp,
            root,
            cwd,
            home: base.join("home"),
            harness: base.join("harness"),
        }
    }

    fn reflect(&self, args: &[&str]) -> Out {
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(["reflect", "--context"])
            .args(args)
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .output()
            .unwrap();
        Out {
            code: out.status.code().unwrap(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let out = self.reflect(args);
        assert_eq!(out.code, 0, "{}", out.stderr);
        serde_json::from_str(&out.stdout).unwrap()
    }
}

// ── #165 path containment ────────────────────────────────────────────

#[test]
fn project_path_with_a_noncanonical_root_is_accepted() {
    let env = Env::new(true);
    let out = env.reflect(&["--project", SLUG]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!out.stderr.contains("escapes"), "{}", out.stderr);
}

#[test]
fn project_symlink_escaping_the_root_is_rejected() {
    let env = Env::new(true);
    let outside = env.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, env.root.join("real/harness/projects/evil")).unwrap();
    let out = env.reflect(&["--project", "evil"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("escapes harness root"),
        "{}",
        out.stderr
    );
    let out = env.reflect(&["--project", "../outside"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("invalid project slug"),
        "{}",
        out.stderr
    );
}

fn alcove(env: &Env, vault: &str) -> serde_json::Value {
    let cfg = format!("[context.alcove]\nvault_path = \"{vault}\"\n");
    fs::write(env.root.join("real/harness/config.toml"), cfg).unwrap();
    env.json(&["--source", "alcove"])["extra_sources"]["alcove"].clone()
}

#[test]
fn alcove_vault_under_a_noncanonical_home_is_accepted() {
    let env = Env::new(true);
    let vault = env.root.join("real/home/vault");
    fs::create_dir_all(&vault).unwrap();
    fs::write(vault.join("note.md"), "# synthetic note\n").unwrap();
    let a = alcove(&env, "~/vault");
    assert!(a.get("error").is_none(), "{a}");
    assert_eq!(a["docs_collected"], 1, "{a}");
}

#[test]
fn alcove_vault_symlink_escaping_home_is_rejected() {
    let env = Env::new(true);
    let outside = env.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, env.root.join("real/home/vault")).unwrap();
    let a = alcove(&env, "~/vault");
    assert!(a["error"].as_str().unwrap().contains("escapes home"), "{a}");
}

// ── #206 project scope ───────────────────────────────────────────────

fn seed(db: &Path, rows: &[(&str, &str)]) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        sqlx::any::install_default_drivers();
        let pool = sqlx::AnyPool::connect(&format!("sqlite://{}", db.display()))
            .await
            .unwrap();
        for (project, summary) in rows {
            let rec = EvolutionRecord {
                timestamp: "2026-01-01T00:00:00Z".into(),
                observations: 1,
                success_rate: 1.0,
                avg_score: 1.0,
                error_patterns: Default::default(),
                failure_patterns: vec![],
                skills_seeded: 0,
                skills_rolled_back: 0,
                total_evolved: 0,
                analysis_summary: (*summary).into(),
                edit_type: Default::default(),
                manifests: vec![],
            };
            evolution::insert_record_pool(&pool, &rec, project)
                .await
                .unwrap();
            let snap = SessionSnapshot {
                timestamp: "2026-01-01T00:00:00Z".into(),
                snap_type: "manual".into(),
                summary: (*summary).into(),
                pending_tasks: vec![],
                context_usage: None,
                pipeline_state: None,
            };
            sessions::insert_snapshot_pool(&pool, &snap, 1, project)
                .await
                .unwrap();
        }
        pool.close().await;
    });
}

fn seeded_env() -> Env {
    let env = Env::new(false);
    // The first run creates the database and schema.
    env.json(&["--project", SLUG]);
    seed(
        &env.harness.join("harness.db"),
        &[
            ("foreign-project", "FOREIGN_SUMMARY"),
            (SLUG, "LOCAL_SUMMARY"),
        ],
    );
    env
}

fn summaries(v: &serde_json::Value) -> Vec<String> {
    v["session_snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["summary"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn project_context_excludes_foreign_snapshots_and_evolution_records() {
    let env = seeded_env();
    let v = env.json(&["--project", SLUG]);
    assert_eq!(v["evolution_stats"]["total_sessions"], 1, "{v}");
    assert_eq!(summaries(&v), ["LOCAL_SUMMARY"], "{v}");
    assert!(
        v["scope_note"]
            .as_str()
            .unwrap()
            .contains("selected project")
    );
}

#[test]
fn default_context_is_scoped_to_the_working_project() {
    let env = seeded_env();
    let v = env.json(&[]);
    assert_eq!(v["evolution_stats"]["total_sessions"], 1, "{v}");
    assert_eq!(summaries(&v), ["LOCAL_SUMMARY"], "{v}");
}

#[test]
fn all_projects_context_still_aggregates() {
    let env = seeded_env();
    let v = env.json(&["--all-projects"]);
    assert_eq!(v["evolution_stats"]["total_sessions"], 2, "{v}");
    assert_eq!(summaries(&v).len(), 2, "{v}");
    assert!(v["scope_note"].as_str().unwrap().contains("aggregated"));
}
