//! merge_project.rs — Consolidate two project slugs into one.
//!
//! `epic-harness merge-project --from <slug> --to <slug> [--dry-run] [--delete-source]`
//!
//! Three-layer merge:
//! 1. Global harness.db (`~/.harness/harness.db`): UPDATE project column from→to.
//! 2. Per-project harness.db: ATTACH source DB, INSERT OR IGNORE into target. Surrogate ids
//!    are re-assigned, promotion counters are summed, and a source row that differs from the
//!    target row with the same key blocks `--delete-source`.
//! 3. File-based: obs/*.jsonl, sessions/*.json, evolved/, evolution.jsonl, orbit/.
//!
//! Close sessions in the source project before merging: a session still writing to it
//! can add rows or files after the merge has read them, and `--delete-source` then removes
//! them.

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode, SqliteQueryResult};
use sqlx::{ConnectOptions, Executor};
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::sqlx_err;
use crate::shared::paths::{global_harness_db_path, harness_projects_root};

// ── Public entry point ───────────────────────────────────────────────────────

pub fn run_merge(from: &str, to: &str, dry_run: bool, delete_source: bool) -> i32 {
    super::runtime::block_on(run_merge_async(from, to, dry_run, delete_source))
}

// ── Core async impl ──────────────────────────────────────────────────────────

/// A slug names one directory directly under the projects root: letters and digits (any
/// script, as `project_slug()` produces), `.`, `_` and `-`, minus `.` and `..`. This keeps
/// `:` and `?` out, which break Windows path joins and the sqlite URL.
fn is_valid_slug(slug: &str) -> bool {
    slug != "."
        && slug != ".."
        && !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Top-level entries of a project dir that a merge step handles.
const MERGED_ENTRIES: &[&str] = &[
    "harness.db",
    "harness.db-wal",
    "harness.db-shm",
    "obs",
    "sessions",
    "orbit",
    "evolved",
    "evolution.jsonl",
];

/// Names of top-level entries in `dir` that the merge does not handle.
/// An unreadable dir is reported as one entry so the caller refuses to delete it.
fn unmerged_entries(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec!["<unreadable>".into()];
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !MERGED_ENTRIES.contains(&n.as_str()))
        .collect();
    names.sort();
    names
}

async fn run_merge_async(from: &str, to: &str, dry_run: bool, delete_source: bool) -> i32 {
    for slug in [from, to] {
        if !is_valid_slug(slug) {
            eprintln!("[merge-project] invalid slug: {slug:?}");
            return 1;
        }
    }
    let projects_root = harness_projects_root();
    let from_dir = projects_root.join(from);
    let to_dir = projects_root.join(to);

    if !from_dir.is_dir() {
        eprintln!("[merge-project] source slug not found: {from}");
        eprintln!("  expected: {}", from_dir.display());
        return 1;
    }
    if from == to {
        eprintln!("[merge-project] --from and --to must be different slugs");
        return 1;
    }
    if let (Ok(f), Ok(t)) = (from_dir.canonicalize(), to_dir.canonicalize())
        && (f.starts_with(&t) || t.starts_with(&f))
    {
        eprintln!("[merge-project] source and target resolve to the same or nested directories");
        return 1;
    }

    println!("[merge-project] from: {from}  →  to: {to}");
    if dry_run {
        println!("[merge-project] DRY RUN — no changes will be made\n");
    }

    if !to_dir.is_dir() {
        if dry_run {
            println!("  [dry-run] would create {}", to_dir.display());
        } else if let Err(e) = std::fs::create_dir_all(&to_dir) {
            eprintln!("[merge-project] cannot create target dir: {e}");
            return 1;
        }
    }

    let mut exit = 0;
    // Source entries the merge leaves behind (nested files, differing skills); they block
    // --delete-source.
    let mut skipped: Vec<String> = Vec::new();
    let mut global_rows = 0usize;
    let mut db_obs = 0usize;
    let mut db_sessions = 0usize;
    let mut db_evo = 0usize;

    // ── 1. Global harness.db ─────────────────────────────────────────────────
    let global_db = global_harness_db_path();
    if global_db.exists() {
        match merge_global_db(&global_db, from, to, dry_run).await {
            Ok(n) => global_rows = n,
            Err(e) => {
                eprintln!("[merge-project] global DB error: {e}");
                exit = 1;
            }
        }
    }

    // ── 2. Per-project harness.db ────────────────────────────────────────────
    let from_db = from_dir.join("harness.db");
    let to_db = to_dir.join("harness.db");
    if from_db.exists() {
        match merge_per_project_dbs(&from_db, &to_db, to, dry_run, &mut skipped).await {
            Ok((o, s, e)) => {
                db_obs = o;
                db_sessions = s;
                db_evo = e;
            }
            Err(e) => {
                eprintln!("[merge-project] per-project DB error: {e}");
                exit = 1;
            }
        }
    }

    // ── 3. File-based data ───────────────────────────────────────────────────
    let mut file_errors = 0usize;
    let (obs_files, e) = copy_dir_files(
        &from_dir.join("obs"),
        &to_dir.join("obs"),
        from,
        dry_run,
        &mut skipped,
    );
    file_errors += e;
    let (session_files, e) = copy_dir_files(
        &from_dir.join("sessions"),
        &to_dir.join("sessions"),
        from,
        dry_run,
        &mut skipped,
    );
    file_errors += e;
    let (orbit_files, e) = copy_dir_files(
        &from_dir.join("orbit"),
        &to_dir.join("orbit"),
        from,
        dry_run,
        &mut skipped,
    );
    file_errors += e;
    let (evolved_dirs, e) = copy_evolved_dir(
        &from_dir.join("evolved"),
        &to_dir.join("evolved"),
        dry_run,
        &mut skipped,
    );
    file_errors += e;
    let evo_lines = match append_evolution_jsonl(&from_dir, &to_dir, dry_run) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("[merge-project] evolution.jsonl error: {e}");
            file_errors += 1;
            0
        }
    };
    if file_errors > 0 {
        eprintln!("[merge-project] {file_errors} file transfer error(s)");
        exit = 1;
    }

    // ── Summary ──────────────────────────────────────────────────────────────
    println!("\n[merge-project] ─── summary ───────────────────────────");
    println!("  global DB rows updated   : {global_rows}");
    println!("  per-project DB merged    : obs={db_obs} sessions={db_sessions} evo={db_evo}");
    println!("  obs files copied         : {obs_files}");
    println!("  session files copied     : {session_files}");
    println!("  evolved dirs copied      : {evolved_dirs}");
    println!("  evolution.jsonl lines    : {evo_lines}");
    println!("  orbit files copied       : {orbit_files}");

    if delete_source {
        // Same gate for dry-run and real run, so a dry run never promises a delete
        // the real run would refuse.
        let mut leftover = unmerged_entries(&from_dir);
        leftover.extend(skipped);
        leftover.sort();
        if exit != 0 {
            eprintln!("[merge-project] source NOT deleted: merge had errors (see above)");
        } else if !leftover.is_empty() {
            eprintln!(
                "[merge-project] source NOT deleted: entries not handled by merge: {}",
                leftover.join(", ")
            );
            exit = 1;
        } else if dry_run {
            println!("  [dry-run] would delete   : {}", from_dir.display());
        } else {
            match std::fs::remove_dir_all(&from_dir) {
                Ok(_) => println!("  source dir deleted       : {}", from_dir.display()),
                Err(e) => {
                    eprintln!("[merge-project] could not delete source: {e}");
                    exit = 1;
                }
            }
        }
    }

    println!("─────────────────────────────────────────────────────────");
    if dry_run {
        println!("[merge-project] dry run complete — omit --dry-run to apply");
    } else {
        println!("[merge-project] done");
    }

    exit
}

// ── Global DB: UPDATE project column ────────────────────────────────────────

// Tables where project is a plain (non-PK) column — safe to UPDATE directly.
const SIMPLE: &[&str] = &[
    "observations",
    "sessions",
    "evolution_records",
    "score_history",
    "orch_runs",
    "orch_control",
    "orbit_pipelines",
    "evolved_skills",
    "global_patterns",
];

/// Rows affected by a statement; a missing table counts as 0, any other error propagates.
fn rows_or_missing(r: Result<SqliteQueryResult, sqlx::Error>) -> io::Result<usize> {
    match r {
        Ok(r) => Ok(r.rows_affected() as usize),
        Err(e) if e.to_string().contains("no such") => Ok(0),
        Err(e) => Err(sqlx_err(e)),
    }
}

async fn merge_global_db(db_path: &Path, from: &str, to: &str, dry_run: bool) -> io::Result<usize> {
    let url = format!("sqlite:{}", db_path.display());
    let mut conn = SqliteConnectOptions::from_str(&url)
        .map_err(sqlx_err)?
        .journal_mode(SqliteJournalMode::Wal)
        .connect()
        .await
        .map_err(sqlx_err)?;

    let mut total = 0usize;

    if dry_run {
        for table in SIMPLE {
            let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM {table} WHERE project = ?"
            )))
            .bind(from)
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
            if n > 0 {
                println!("  [dry-run] {table}: {n} rows would be re-labelled");
                total += n as usize;
            }
        }
        for table in &["metrics_state", "skill_attribution", "promotion_counters"] {
            let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM {table} WHERE project = ?"
            )))
            .bind(from)
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
            if n > 0 {
                println!("  [dry-run] {table}: {n} rows would be merged/re-labelled");
                total += n as usize;
            }
        }
        return Ok(total);
    }

    conn.execute("BEGIN IMMEDIATE").await.map_err(sqlx_err)?;

    let res = match apply_global_updates(&mut conn, from, to).await {
        Ok(total) => conn
            .execute("COMMIT")
            .await
            .map(|_| total)
            .map_err(sqlx_err),
        Err(e) => Err(e),
    };
    if res.is_err() {
        let _ = conn.execute("ROLLBACK").await;
    }
    res
}

/// Runs inside the caller's transaction; any error other than a missing table aborts the merge.
async fn apply_global_updates(
    conn: &mut SqliteConnection,
    from: &str,
    to: &str,
) -> io::Result<usize> {
    let mut total = 0usize;

    for table in SIMPLE {
        total += rows_or_missing(
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {table} SET project = ? WHERE project = ?"
            )))
            .bind(to)
            .bind(from)
            .execute(&mut *conn)
            .await,
        )?;
    }

    // metrics_state: composite PK (key, project) — drop conflicting source rows, then rename.
    rows_or_missing(
        sqlx::query(
            "DELETE FROM metrics_state WHERE project = ? \
             AND key IN (SELECT key FROM metrics_state WHERE project = ?)",
        )
        .bind(from)
        .bind(to)
        .execute(&mut *conn)
        .await,
    )?;
    total += rows_or_missing(
        sqlx::query("UPDATE metrics_state SET project = ? WHERE project = ?")
            .bind(to)
            .bind(from)
            .execute(&mut *conn)
            .await,
    )?;

    // skill_attribution: composite PK (skill_name, project) — keep target rows on conflict.
    rows_or_missing(
        sqlx::query(
            "DELETE FROM skill_attribution WHERE project = ? \
             AND skill_name IN (SELECT skill_name FROM skill_attribution WHERE project = ?)",
        )
        .bind(from)
        .bind(to)
        .execute(&mut *conn)
        .await,
    )?;
    total += rows_or_missing(
        sqlx::query("UPDATE skill_attribution SET project = ? WHERE project = ?")
            .bind(to)
            .bind(from)
            .execute(&mut *conn)
            .await,
    )?;

    // promotion_counters: composite PK (pattern_key, project) — sum counts on conflict.
    rows_or_missing(
        sqlx::query(
            "UPDATE promotion_counters \
             SET count = count + (
                 SELECT count FROM promotion_counters AS src
                 WHERE src.project = ? AND src.pattern_key = promotion_counters.pattern_key
             ) \
             WHERE project = ? \
             AND pattern_key IN (SELECT pattern_key FROM promotion_counters WHERE project = ?)",
        )
        .bind(from)
        .bind(to)
        .bind(from)
        .execute(&mut *conn)
        .await,
    )?;
    // Delete only the source keys summed above; source-only keys are re-labelled below.
    rows_or_missing(
        sqlx::query(
            "DELETE FROM promotion_counters WHERE project = ? \
             AND pattern_key IN (SELECT pattern_key FROM promotion_counters WHERE project = ?)",
        )
        .bind(from)
        .bind(to)
        .execute(&mut *conn)
        .await,
    )?;
    total += rows_or_missing(
        sqlx::query("UPDATE promotion_counters SET project = ? WHERE project = ?")
            .bind(to)
            .bind(from)
            .execute(&mut *conn)
            .await,
    )?;

    Ok(total)
}

// ── Per-project harness.db: ATTACH + INSERT OR IGNORE ───────────────────────

async fn merge_per_project_dbs(
    from_db: &Path,
    to_db: &Path,
    to_slug: &str,
    dry_run: bool,
    skipped: &mut Vec<String>,
) -> io::Result<(usize, usize, usize)> {
    if dry_run {
        let url = format!("sqlite:{}", from_db.display());
        let mut conn = SqliteConnectOptions::from_str(&url)
            .map_err(sqlx_err)?
            .journal_mode(SqliteJournalMode::Wal)
            .read_only(true)
            .connect()
            .await
            .map_err(sqlx_err)?;
        let obs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM observations")
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
        let sess: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
        let evo: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evolution_records")
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
        println!("  [dry-run] per-project DB would merge: obs={obs} sessions={sess} evo={evo}");
        if to_db.is_file() {
            let url = format!("sqlite:{}", to_db.display());
            let mut target = SqliteConnectOptions::from_str(&url)
                .map_err(sqlx_err)?
                .journal_mode(SqliteJournalMode::Wal)
                .connect()
                .await
                .map_err(sqlx_err)?;
            attach_source(&mut target, from_db).await?;
            skipped.extend(super::migrate::attached_conflicts(&mut target, to_slug, "src").await?);
        }
        return Ok((obs as usize, sess as usize, evo as usize));
    }

    if let Some(parent) = to_db.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("sqlite:{}", to_db.display());
    let mut conn = SqliteConnectOptions::from_str(&url)
        .map_err(sqlx_err)?
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true)
        .connect()
        .await
        .map_err(sqlx_err)?;

    // Init schema if this is a brand-new target DB.
    let table_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
            .fetch_one(&mut conn)
            .await
            .unwrap_or(0);
    if table_count == 0 {
        conn.execute(super::schema::DDL_SQLITE)
            .await
            .map_err(sqlx_err)?;
    }

    attach_source(&mut conn, from_db).await?;
    skipped.extend(super::migrate::attached_conflicts(&mut conn, to_slug, "src").await?);

    let stats = super::migrate::merge_attached_db_async(&mut conn, to_slug, "src", true).await?;

    conn.execute("DETACH DATABASE src")
        .await
        .map_err(sqlx_err)?;

    Ok((stats.obs, stats.sessions, stats.evo))
}

async fn attach_source(conn: &mut SqliteConnection, from_db: &Path) -> io::Result<()> {
    let escaped = from_db.to_string_lossy().replace('\'', "''");
    conn.execute(sqlx::AssertSqlSafe(format!(
        "ATTACH DATABASE '{escaped}' AS src"
    )))
    .await
    .map_err(sqlx_err)?;
    Ok(())
}

// ── File-level helpers ───────────────────────────────────────────────────────

/// True when every file under `src` exists under `dst` with equal bytes.
fn is_covered(src: &Path, dst: &Path) -> bool {
    if src.is_dir() {
        return std::fs::read_dir(src).is_ok_and(|rd| {
            rd.flatten()
                .all(|e| is_covered(&e.path(), &dst.join(e.file_name())))
        });
    }
    matches!((std::fs::read(src), std::fs::read(dst)), (Ok(a), Ok(b)) if a == b)
}

/// Destination for the file `src` in `dst_dir`: its own name, or `<stem>.from-<tag><ext>`
/// (then `-2`, `-3`, ...) when a different file already holds the name. `None` when an
/// identical copy is already in place, so a retried merge adds nothing.
fn free_dest(src: &Path, dst_dir: &Path, tag: &str) -> Option<PathBuf> {
    let stem = src.file_stem()?.to_string_lossy();
    let ext = src
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut dst = dst_dir.join(src.file_name()?);
    let mut n = 0;
    while dst.exists() {
        if is_covered(src, &dst) {
            return None;
        }
        n += 1;
        let sep = if n == 1 {
            String::new()
        } else {
            format!("-{n}")
        };
        dst = dst_dir.join(format!("{stem}.from-{tag}{sep}{ext}"));
    }
    Some(dst)
}

/// Copy files from `src_dir` to `dst_dir`. A same-name file with different content is
/// kept under a suffixed name, never overwritten. Entries that are not files are pushed to
/// `skipped`. Returns the number of files copied (or would copy in dry-run) and the number
/// of failures.
fn copy_dir_files(
    src_dir: &Path,
    dst_dir: &Path,
    tag: &str,
    dry_run: bool,
    skipped: &mut Vec<String>,
) -> (usize, usize) {
    let entries = match std::fs::read_dir(src_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (0, 0),
        Err(e) => {
            eprintln!("[merge-project] cannot read {}: {e}", src_dir.display());
            return (0, 1);
        }
    };
    let mut count = 0;
    let mut errors = 0;
    for entry in entries.flatten() {
        let src = entry.path();
        if !src.is_file() {
            skipped.push(src.display().to_string());
            continue;
        }
        let Some(dst) = free_dest(&src, dst_dir, tag) else {
            continue;
        };
        if dry_run {
            count += 1;
        } else {
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::copy(&src, &dst) {
                Ok(_) => count += 1,
                Err(e) => {
                    eprintln!("[merge-project] copy {} failed: {e}", src.display());
                    errors += 1;
                }
            }
        }
    }
    (count, errors)
}

/// Copy evolved skill subdirectories that don't already exist in target. A skill whose
/// target copy differs is kept as is (target wins) and pushed to `skipped`, as are
/// non-directory entries.
fn copy_evolved_dir(
    src_evolved: &Path,
    dst_evolved: &Path,
    dry_run: bool,
    skipped: &mut Vec<String>,
) -> (usize, usize) {
    let entries = match std::fs::read_dir(src_evolved) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (0, 0),
        Err(e) => {
            eprintln!("[merge-project] cannot read {}: {e}", src_evolved.display());
            return (0, 1);
        }
    };
    let mut count = 0;
    let mut errors = 0;
    for entry in entries.flatten() {
        let src = entry.path();
        if !src.is_dir() {
            skipped.push(src.display().to_string());
            continue;
        }
        let name = entry.file_name();
        let dst = dst_evolved.join(&name);
        if dst.exists() {
            if !is_covered(&src, &dst) {
                skipped.push(src.display().to_string());
            }
            continue;
        }
        if !dry_run && let Err(e) = copy_dir_recursive(&src, &dst) {
            eprintln!("[merge-project] copy {} failed: {e}", src.display());
            errors += 1;
            continue;
        }
        count += 1;
    }
    (count, errors)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let s = entry.path();
        let d = dst.join(entry.file_name());
        if s.is_dir() {
            copy_dir_recursive(&s, &d)?;
        } else {
            std::fs::copy(&s, &d)?;
        }
    }
    Ok(())
}

/// Append lines from source evolution.jsonl that don't exist in target (dedup by whole line).
fn append_evolution_jsonl(from_dir: &Path, to_dir: &Path, dry_run: bool) -> io::Result<usize> {
    let src = from_dir.join("evolution.jsonl");
    let dst = to_dir.join("evolution.jsonl");
    if !src.exists() {
        return Ok(0);
    }

    let src_lines = std::fs::read_to_string(&src)?;
    let src_lines: Vec<&str> = src_lines.lines().filter(|l| !l.trim().is_empty()).collect();
    if src_lines.is_empty() {
        return Ok(0);
    }

    // Only whole equal records are duplicates; distinct records often share a long prefix.
    let mut existing: std::collections::HashSet<String> = if dst.exists() {
        std::fs::read_to_string(&dst)?
            .lines()
            .map(String::from)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let new_lines: Vec<&str> = src_lines
        .iter()
        .filter(|l| existing.insert((**l).to_string()))
        .copied()
        .collect();

    if new_lines.is_empty() {
        return Ok(0);
    }

    if !dry_run {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&dst)?;
        for line in &new_lines {
            writeln!(f, "{line}")?;
        }
    }

    Ok(new_lines.len())
}
