//! store_merge_test.rs — Integration tests for `merge-project`.
//! Uses HARNESS_DIR to redirect the harness root to a temp dir.

use std::path::Path;
use std::sync::Mutex;

use epic_harness::store::merge_project::run_merge;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor, Row};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn set_root(root: &Path) {
    // SAFETY: callers hold ENV_LOCK, so env access is serialized within this test binary.
    unsafe {
        std::env::set_var("HARNESS_DIR", root);
    }
}

/// Run `f` on a fresh connection to `db` (created if missing).
fn with_db<T>(db: &Path, f: impl AsyncFnOnce(&mut sqlx::sqlite::SqliteConnection) -> T) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut conn = SqliteConnectOptions::new()
                .filename(db)
                .create_if_missing(true)
                .connect()
                .await
                .unwrap();
            f(&mut conn).await
        })
}

fn exec(db: &Path, sql: &'static str) {
    with_db(db, async |c| {
        c.execute(sql).await.unwrap();
    });
}

fn scalar(db: &Path, sql: &'static str) -> String {
    with_db(db, async |c| {
        c.fetch_one(sql).await.unwrap().get::<String, _>(0)
    })
}

fn seed_project(root: &Path, slug: &str) {
    let obs = root.join("projects").join(slug).join("obs");
    std::fs::create_dir_all(&obs).unwrap();
    std::fs::write(obs.join("session_a.jsonl"), "{}\n").unwrap();
}

#[test]
fn file_copy_failure_keeps_source_and_fails() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    // A regular file where the target `obs` directory must be makes every copy fail.
    let to = tmp.path().join("projects").join("dst-proj");
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join("obs"), "not a dir").unwrap();

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_ne!(code, 0, "failed file copy must produce a non-zero exit");
    assert!(
        tmp.path()
            .join("projects/src-proj/obs/session_a.jsonl")
            .exists(),
        "source must survive a failed transfer"
    );
}

#[test]
fn global_db_error_rolls_back_and_keeps_source() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let db = tmp.path().join("harness.db");
    exec(&db, "CREATE TABLE observations (project TEXT NOT NULL)");
    exec(&db, "CREATE TABLE sessions (project TEXT NOT NULL)");
    exec(&db, "INSERT INTO observations VALUES ('src-proj')");
    exec(&db, "INSERT INTO sessions VALUES ('src-proj')");
    exec(
        &db,
        "CREATE TRIGGER fail_update BEFORE UPDATE ON sessions \
         BEGIN SELECT RAISE(ABORT, 'forced failure'); END",
    );

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_ne!(code, 0, "global DB error must produce a non-zero exit");
    assert_eq!(
        scalar(&db, "SELECT project FROM observations"),
        "src-proj",
        "earlier updates must be rolled back"
    );
    assert!(tmp.path().join("projects/src-proj").is_dir());
}

#[test]
fn merge_into_active_target_succeeds_and_deletes_source() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    std::fs::create_dir_all(tmp.path().join("projects/dst-proj")).unwrap();
    let db = tmp.path().join("harness.db");
    exec(
        &db,
        "CREATE TABLE metrics_state (key TEXT NOT NULL, value TEXT NOT NULL, \
         project TEXT NOT NULL DEFAULT '', PRIMARY KEY (key, project))",
    );
    exec(
        &db,
        "CREATE TABLE skill_attribution (skill_name TEXT NOT NULL, \
         project TEXT NOT NULL DEFAULT '', PRIMARY KEY (skill_name, project))",
    );
    exec(
        &db,
        "INSERT INTO metrics_state VALUES \
         ('trend', 'improving', 'dst-proj'), ('trend', 'declining', 'src-proj'), \
         ('best_session', 'only-src', 'src-proj')",
    );
    exec(
        &db,
        "INSERT INTO skill_attribution VALUES \
         ('shared', 'dst-proj'), ('shared', 'src-proj'), ('only-src', 'src-proj')",
    );

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_eq!(code, 0);
    assert!(!tmp.path().join("projects/src-proj").exists());
    assert!(
        tmp.path()
            .join("projects/dst-proj/obs/session_a.jsonl")
            .exists()
    );
    // Conflicting key keeps the target value; source-only keys are re-labelled.
    assert_eq!(
        scalar(
            &db,
            "SELECT value FROM metrics_state WHERE key = 'trend' AND project = 'dst-proj'"
        ),
        "improving"
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT CAST(COUNT(*) AS TEXT) FROM metrics_state WHERE project = 'dst-proj'"
        ),
        "2"
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT CAST(COUNT(*) AS TEXT) FROM skill_attribution WHERE project = 'dst-proj'"
        ),
        "2"
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT CAST(COUNT(*) AS TEXT) FROM skill_attribution WHERE project = 'src-proj'"
        ),
        "0"
    );
}

#[test]
fn slug_alias_with_separator_is_rejected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "proj");

    let code = run_merge("proj/", "proj", false, true);

    assert_ne!(code, 0, "alias of the same slug must be refused");
    assert!(
        tmp.path()
            .join("projects/proj/obs/session_a.jsonl")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn symlink_alias_of_target_is_rejected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "proj");
    let projects = tmp.path().join("projects");
    std::os::unix::fs::symlink(projects.join("proj"), projects.join("alias")).unwrap();

    let code = run_merge("alias", "proj", false, true);

    assert_ne!(code, 0, "symlink to the target must be refused");
    assert!(projects.join("proj/obs/session_a.jsonl").exists());
}

#[test]
fn traversal_slug_is_rejected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "other");
    std::fs::write(tmp.path().join("keep.txt"), "x").unwrap();

    for (from, to) in [
        ("..", "other"),
        (".", "other"),
        ("", "other"),
        ("other", ".."),
    ] {
        let code = run_merge(from, to, false, true);
        assert_ne!(code, 0, "slug {from:?} -> {to:?} must be refused");
    }

    assert!(tmp.path().join("keep.txt").exists());
    assert!(
        tmp.path()
            .join("projects/other/obs/session_a.jsonl")
            .exists()
    );
}

#[test]
fn unhandled_source_entries_block_delete() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let src = tmp.path().join("projects/src-proj");
    std::fs::create_dir_all(src.join("team")).unwrap();
    std::fs::write(src.join("team/notes.md"), "x").unwrap();
    std::fs::write(src.join("metrics.json"), "{}").unwrap();
    std::fs::write(src.join("unknown.bin"), "x").unwrap();

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_ne!(code, 0, "unhandled entries must block --delete-source");
    assert!(src.join("team/notes.md").exists());
    assert!(src.join("metrics.json").exists());
    assert!(src.join("unknown.bin").exists());
    assert!(
        tmp.path()
            .join("projects/dst-proj/obs/session_a.jsonl")
            .exists(),
        "handled entries are still merged"
    );
}

fn project(root: &Path, slug: &str) -> std::path::PathBuf {
    root.join("projects").join(slug)
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Sorted `name=content` of every file directly under `dir`.
fn dir_files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            format!(
                "{}={}",
                e.file_name().to_string_lossy(),
                std::fs::read_to_string(e.path()).unwrap().trim()
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn promotion_counters_unique_to_source_are_kept() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    std::fs::create_dir_all(project(tmp.path(), "dst-proj")).unwrap();
    let db = tmp.path().join("harness.db");
    exec(
        &db,
        "CREATE TABLE promotion_counters (pattern_key TEXT NOT NULL, \
         project TEXT NOT NULL DEFAULT '', count INTEGER NOT NULL DEFAULT 0, \
         PRIMARY KEY (pattern_key, project))",
    );
    exec(
        &db,
        "INSERT INTO promotion_counters VALUES ('unique', 'src-proj', 7), \
         ('shared', 'src-proj', 2), ('shared', 'dst-proj', 3), ('other', 'elsewhere', 11)",
    );

    let code = run_merge("src-proj", "dst-proj", false, false);

    assert_eq!(code, 0);
    let rows = scalar(
        &db,
        "SELECT group_concat(pattern_key || ':' || project || ':' || count, ' ') FROM \
         (SELECT * FROM promotion_counters ORDER BY pattern_key, project)",
    );
    assert_eq!(
        rows,
        "other:elsewhere:11 shared:dst-proj:5 unique:dst-proj:7"
    );
}

#[test]
fn evolution_records_sharing_a_prefix_are_not_deduplicated() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    let prefix = "x".repeat(100);
    let first = format!("{{\"p\":\"{prefix}\",\"analysis_summary\":\"first\"}}");
    let second = format!("{{\"p\":\"{prefix}\",\"analysis_summary\":\"second\"}}");
    write(
        &project(tmp.path(), "src-proj").join("evolution.jsonl"),
        &format!("{first}\n{second}\n{second}\n"),
    );
    let target = project(tmp.path(), "dst-proj").join("evolution.jsonl");
    write(&target, &format!("{first}\n"));

    assert_eq!(run_merge("src-proj", "dst-proj", false, false), 0);
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        format!("{first}\n{second}\n"),
        "distinct record kept, exact duplicates dropped"
    );

    assert_eq!(run_merge("src-proj", "dst-proj", false, false), 0);
    assert_eq!(
        std::fs::read_to_string(&target).unwrap().lines().count(),
        2,
        "a retried merge appends nothing"
    );
}

#[test]
fn same_name_file_with_different_content_is_kept_under_a_suffix() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    let (src, dst) = (
        project(tmp.path(), "src-proj"),
        project(tmp.path(), "dst-proj"),
    );
    write(&src.join("obs/session_a.jsonl"), "from-src\n");
    write(&src.join("obs/session_b.jsonl"), "only-src\n");
    write(&src.join("obs/session_c.jsonl"), "same\n");
    write(&dst.join("obs/session_a.jsonl"), "from-dst\n");
    write(&dst.join("obs/session_c.jsonl"), "same\n");
    let expected = [
        "session_a.from-src-proj.jsonl=from-src",
        "session_a.jsonl=from-dst",
        "session_b.jsonl=only-src",
        "session_c.jsonl=same",
    ];

    assert_eq!(run_merge("src-proj", "dst-proj", false, false), 0);
    assert_eq!(dir_files(&dst.join("obs")), expected);

    assert_eq!(run_merge("src-proj", "dst-proj", false, false), 0);
    assert_eq!(
        dir_files(&dst.join("obs")),
        expected,
        "a retried merge must not add more copies"
    );

    assert_eq!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(!src.exists(), "every source file now lives in the target");
    assert_eq!(dir_files(&dst.join("obs")), expected);
}

#[test]
fn nested_directory_in_a_merged_dir_blocks_delete() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let src = project(tmp.path(), "src-proj");
    write(&src.join("obs/archive/old.jsonl"), "x\n");

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_ne!(code, 0, "a skipped nested entry must block --delete-source");
    assert!(src.join("obs/archive/old.jsonl").exists());
}

#[test]
fn differing_evolved_skill_blocks_delete_but_identical_does_not() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let (src, dst) = (
        project(tmp.path(), "src-proj"),
        project(tmp.path(), "dst-proj"),
    );
    write(&src.join("evolved/same/SKILL.md"), "one\n");
    write(&dst.join("evolved/same/SKILL.md"), "one\n");
    write(&src.join("evolved/differs/SKILL.md"), "src\n");
    write(&dst.join("evolved/differs/SKILL.md"), "dst\n");

    assert_ne!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(src.join("evolved/differs/SKILL.md").exists());
    assert_eq!(
        std::fs::read_to_string(dst.join("evolved/differs/SKILL.md")).unwrap(),
        "dst\n",
        "the target skill is never overwritten"
    );

    std::fs::remove_dir_all(src.join("evolved/differs")).unwrap();
    assert_eq!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(!src.exists());
}

/// Run the binary so stdout can be inspected; the home dir is a temp dir too.
fn run_cli(root: &Path, args: &[&str]) -> std::process::Output {
    let home = root.join("home");
    write(&home.join(".config/epic-harness/telemetry-consent"), "off");
    std::process::Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("merge-project")
        .args(args)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("HARNESS_DIR", root)
        .output()
        .unwrap()
}

#[test]
fn dry_run_does_not_promise_a_delete_the_real_run_refuses() {
    let tmp = tempfile::tempdir().unwrap();
    seed_project(tmp.path(), "src-proj");
    let src = project(tmp.path(), "src-proj");
    let args = [
        "--from",
        "src-proj",
        "--to",
        "dst-proj",
        "--dry-run",
        "--delete-source",
    ];

    // Control: nothing blocks the delete, so the dry run says it would delete.
    let out = run_cli(tmp.path(), &args);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("would delete"));

    write(&src.join("obs/archive/old.jsonl"), "x\n");
    let out = run_cli(tmp.path(), &args);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("would delete"),
        "dry run must not promise a delete the real run refuses"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("NOT deleted"));
    assert!(src.join("obs/archive/old.jsonl").exists());
}

#[test]
fn slug_outside_the_allowlist_is_rejected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "other");

    for bad in ["a:b", "a?b", "a b", "a*b", "a\\b", "a/b", "a\0b", "a\tb"] {
        assert_ne!(
            run_merge(bad, "other", false, true),
            0,
            "slug {bad:?} must be refused as source"
        );
        assert_ne!(
            run_merge("other", bad, false, true),
            0,
            "slug {bad:?} must be refused as target"
        );
    }
    assert!(
        tmp.path()
            .join("projects/other/obs/session_a.jsonl")
            .exists()
    );

    // Control: dots, underscores, hyphens and non-ASCII letters (as project_slug() makes) are allowed.
    for good in ["my.proj_1-x", "한글-프로젝트"] {
        seed_project(tmp.path(), good);
        assert_eq!(run_merge(good, "other", false, true), 0, "slug {good:?}");
    }
}

#[test]
fn failed_attached_table_copy_rolls_back_and_keeps_source() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let src_db = project(tmp.path(), "src-proj").join("harness.db");
    exec(
        &src_db,
        "CREATE TABLE observations (id INTEGER PRIMARY KEY AUTOINCREMENT, \
         timestamp TEXT NOT NULL, session_id TEXT NOT NULL, tool TEXT NOT NULL, \
         tool_category TEXT NOT NULL)",
    );
    exec(
        &src_db,
        "INSERT INTO observations (timestamp, session_id, tool, tool_category) \
         VALUES ('t', 's', 'Bash', 'shell')",
    );
    // An agent whose run exists nowhere: the target's foreign key rejects the copy.
    exec(
        &src_db,
        "CREATE TABLE orch_agents (id TEXT PRIMARY KEY, run_id TEXT NOT NULL)",
    );
    exec(&src_db, "INSERT INTO orch_agents VALUES ('a', 'ghost')");

    let code = run_merge("src-proj", "dst-proj", false, true);

    assert_ne!(code, 0, "a failed table copy must fail the merge");
    assert!(src_db.exists(), "source must survive a failed copy");
    let dst_db = project(tmp.path(), "dst-proj").join("harness.db");
    assert_eq!(
        scalar(&dst_db, "SELECT CAST(COUNT(*) AS TEXT) FROM observations"),
        "0",
        "the earlier table copy must be rolled back"
    );
}

#[test]
fn differing_evolved_skill_row_blocks_delete_but_identical_does_not() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let src = project(tmp.path(), "src-proj");
    let src_db = src.join("harness.db");
    let dst_db = project(tmp.path(), "dst-proj").join("harness.db");
    for db in [&src_db, &dst_db] {
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        exec(
            db,
            "CREATE TABLE evolved_skills (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL UNIQUE, origin TEXT NOT NULL DEFAULT '', \
             confidence REAL NOT NULL DEFAULT 0.5, project TEXT NOT NULL, \
             skill_md TEXT NOT NULL DEFAULT '', active INTEGER NOT NULL DEFAULT 1, \
             created TEXT NOT NULL, updated TEXT NOT NULL)",
        );
    }
    exec(
        &src_db,
        "INSERT INTO evolved_skills (name, project, skill_md, created, updated) VALUES \
         ('same', 'src-proj', 'one', 't', 't'), ('differs', 'src-proj', 'src', 't', 't')",
    );
    exec(
        &dst_db,
        "INSERT INTO evolved_skills (name, project, skill_md, created, updated) VALUES \
         ('same', 'dst-proj', 'one', 't', 't'), ('differs', 'dst-proj', 'dst', 't', 't')",
    );

    let dry = run_cli(
        tmp.path(),
        &[
            "--from",
            "src-proj",
            "--to",
            "dst-proj",
            "--dry-run",
            "--delete-source",
        ],
    );
    assert!(
        !String::from_utf8_lossy(&dry.stdout).contains("would delete"),
        "dry run must not promise a delete the real run refuses"
    );

    assert_ne!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(src_db.exists(), "a differing row blocks the delete");
    assert_eq!(
        scalar(
            &dst_db,
            "SELECT skill_md FROM evolved_skills WHERE name = 'differs'"
        ),
        "dst",
        "the target row is never overwritten"
    );

    exec(&src_db, "DELETE FROM evolved_skills WHERE name = 'differs'");
    assert_eq!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(!src.exists(), "identical rows do not block the delete");
}

#[test]
fn differing_keyed_row_blocks_delete_but_identical_does_not() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    set_root(tmp.path());
    seed_project(tmp.path(), "src-proj");
    let src = project(tmp.path(), "src-proj");
    let src_db = src.join("harness.db");
    let dst_db = project(tmp.path(), "dst-proj").join("harness.db");
    for db in [&src_db, &dst_db] {
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        exec(
            db,
            "CREATE TABLE metrics_state (key TEXT NOT NULL, value TEXT NOT NULL, \
             project TEXT NOT NULL DEFAULT '', PRIMARY KEY (key, project))",
        );
    }
    exec(
        &src_db,
        "INSERT INTO metrics_state VALUES ('same', 'v', 'src-proj'), ('differs', 'src', 'src-proj')",
    );
    exec(
        &dst_db,
        "INSERT INTO metrics_state VALUES ('same', 'v', 'dst-proj'), ('differs', 'dst', 'dst-proj')",
    );

    let dry = run_cli(
        tmp.path(),
        &[
            "--from",
            "src-proj",
            "--to",
            "dst-proj",
            "--dry-run",
            "--delete-source",
        ],
    );
    assert!(
        !String::from_utf8_lossy(&dry.stdout).contains("would delete"),
        "dry run must not promise a delete the real run refuses"
    );

    assert_ne!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(src_db.exists(), "a differing row blocks the delete");
    assert_eq!(
        scalar(
            &dst_db,
            "SELECT value FROM metrics_state WHERE key = 'differs'"
        ),
        "dst",
        "the target row is never overwritten"
    );

    exec(&src_db, "DELETE FROM metrics_state WHERE key = 'differs'");
    assert_eq!(run_merge("src-proj", "dst-proj", false, true), 0);
    assert!(!src.exists(), "identical rows do not block the delete");
}

#[test]
fn to_global_keeps_promotion_counters_unsummed_and_source_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let global = tmp.path().join("harness.db");
    let src_db = project(tmp.path(), "alpha").join("harness.db");
    std::fs::create_dir_all(src_db.parent().unwrap()).unwrap();
    exec(
        &global,
        "CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    );
    for db in [&global, &src_db] {
        exec(
            db,
            "CREATE TABLE promotion_counters (pattern_key TEXT NOT NULL, \
             project TEXT NOT NULL DEFAULT '', count INTEGER NOT NULL DEFAULT 0, \
             PRIMARY KEY (pattern_key, project))",
        );
    }
    exec(
        &global,
        "INSERT INTO promotion_counters VALUES ('shared', 'alpha', 3)",
    );
    exec(
        &src_db,
        "INSERT INTO promotion_counters VALUES ('shared', 'alpha', 2), ('only', 'alpha', 7)",
    );
    let home = tmp.path().join("home");
    write(&home.join(".config/epic-harness/telemetry-consent"), "off");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["migrate", "--to-global"])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("HARNESS_DIR", tmp.path())
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rows = "SELECT group_concat(pattern_key || ':' || count, ' ') FROM \
                (SELECT * FROM promotion_counters ORDER BY pattern_key)";
    assert_eq!(scalar(&global, rows), "only:7 shared:3", "no summing");
    assert_eq!(
        scalar(&src_db, rows),
        "only:7 shared:2",
        "source left in place"
    );
}
