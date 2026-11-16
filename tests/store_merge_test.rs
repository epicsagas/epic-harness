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
