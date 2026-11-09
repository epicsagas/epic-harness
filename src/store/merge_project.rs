//! merge_project.rs — Consolidate two project slugs into one.
//!
//! `epic-harness merge-project --from <slug> --to <slug> [--dry-run] [--delete-source]`
//!
//! Three-layer merge:
//! 1. Global harness.db (`~/.harness/harness.db`): UPDATE project column from→to.
//! 2. Per-project harness.db: ATTACH source DB, INSERT OR IGNORE into target.
//! 3. File-based: obs/*.jsonl, sessions/*.json, evolved/, evolution.jsonl, orbit/.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{ConnectOptions, Executor};
use std::io;
use std::path::Path;
use std::str::FromStr;

use super::sqlx_err;

#[path = "global_transfer.rs"]
mod global_transfer;
use crate::shared::paths::{global_harness_db_path, harness_projects_root};

// ── Public entry point ───────────────────────────────────────────────────────

pub fn run_merge(from: &str, to: &str, dry_run: bool, delete_source: bool) -> i32 {
    super::runtime::block_on(run_merge_async(from, to, dry_run, delete_source))
}

// ── Core async impl ──────────────────────────────────────────────────────────

async fn run_merge_async(from: &str, to: &str, dry_run: bool, delete_source: bool) -> i32 {
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
        match merge_per_project_dbs(&from_db, &to_db, to, dry_run).await {
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
    let obs_files = merge_file_count(
        copy_dir_files(&from_dir.join("obs"), &to_dir.join("obs"), dry_run),
        "observations",
        &mut exit,
    );
    let session_files = merge_file_count(
        copy_dir_files(
            &from_dir.join("sessions"),
            &to_dir.join("sessions"),
            dry_run,
        ),
        "sessions",
        &mut exit,
    );
    let orbit_files = merge_file_count(
        copy_dir_files(&from_dir.join("orbit"), &to_dir.join("orbit"), dry_run),
        "orbit",
        &mut exit,
    );
    let evolved_dirs = merge_file_count(
        copy_evolved_dir(&from_dir.join("evolved"), &to_dir.join("evolved"), dry_run),
        "evolved skills",
        &mut exit,
    );
    let evo_lines = merge_file_count(
        append_evolution_jsonl(&from_dir, &to_dir, dry_run),
        "evolution history",
        &mut exit,
    );

    // ── Summary ──────────────────────────────────────────────────────────────
    println!("\n[merge-project] ─── summary ───────────────────────────");
    println!("  global DB rows updated   : {global_rows}");
    println!("  per-project DB merged    : obs={db_obs} sessions={db_sessions} evo={db_evo}");
    println!("  obs files copied         : {obs_files}");
    println!("  session files copied     : {session_files}");
    println!("  evolved dirs copied      : {evolved_dirs}");
    println!("  evolution.jsonl lines    : {evo_lines}");
    println!("  orbit files copied       : {orbit_files}");

    if delete_source && exit != 0 {
        eprintln!("[merge-project] source retained because at least one transfer failed");
    } else if delete_source {
        if dry_run {
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

fn merge_file_count(result: io::Result<usize>, label: &str, exit: &mut i32) -> usize {
    match result {
        Ok(count) => count,
        Err(error) => {
            eprintln!("[merge-project] {label} transfer failed: {error}");
            *exit = 1;
            0
        }
    }
}

// ── Global DB: UPDATE project column ────────────────────────────────────────

async fn merge_global_db(db_path: &Path, from: &str, to: &str, dry_run: bool) -> io::Result<usize> {
    let url = format!("sqlite:{}", db_path.display());
    let mut conn = SqliteConnectOptions::from_str(&url)
        .map_err(sqlx_err)?
        .journal_mode(SqliteJournalMode::Wal)
        .connect()
        .await
        .map_err(sqlx_err)?;

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

    global_transfer::transfer(&mut conn, from, to).await
}

// ── Per-project harness.db: ATTACH + INSERT OR IGNORE ───────────────────────

async fn merge_per_project_dbs(
    from_db: &Path,
    to_db: &Path,
    to_slug: &str,
    dry_run: bool,
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

    let escaped = from_db.to_string_lossy().replace('\'', "''");
    conn.execute(sqlx::AssertSqlSafe(format!(
        "ATTACH DATABASE '{escaped}' AS src"
    )))
    .await
    .map_err(sqlx_err)?;

    let stats = super::migrate::merge_attached_db_async(&mut conn, to_slug, "src").await?;

    conn.execute("DETACH DATABASE src")
        .await
        .map_err(sqlx_err)?;

    Ok((stats.obs, stats.sessions, stats.evo))
}

// ── File-level helpers ───────────────────────────────────────────────────────

/// Copy files from `src_dir` to `dst_dir`, skipping files that already exist.
/// Returns the number of files copied (or would copy in dry-run).
fn copy_dir_files(src_dir: &Path, dst_dir: &Path, dry_run: bool) -> io::Result<usize> {
    if !src_dir.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(src_dir)?;
    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let name = entry.file_name();
        let dst = dst_dir.join(&name);
        if dst.exists() {
            continue;
        }
        if dry_run {
            count += 1;
        } else {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dst)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Copy evolved skill subdirectories that don't already exist in target.
fn copy_evolved_dir(src_evolved: &Path, dst_evolved: &Path, dry_run: bool) -> io::Result<usize> {
    if !src_evolved.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(src_evolved)?;
    let mut count = 0;
    for entry in entries {
        let entry = entry?;
        let src = entry.path();
        if !src.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let dst = dst_evolved.join(&name);
        if dst.exists() {
            continue;
        }
        if !dry_run {
            copy_dir_recursive(&src, &dst)?;
        }
        count += 1;
    }
    Ok(count)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
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

/// Append lines from source evolution.jsonl that don't exist in target (dedup by content hash).
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

    // Build set of existing lines (by first 80 chars as a cheap dedup key).
    let existing: std::collections::HashSet<String> = if dst.exists() {
        std::fs::read_to_string(&dst)?
            .lines()
            .map(|l| l.chars().take(80).collect())
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let new_lines: Vec<&str> = src_lines
        .iter()
        .filter(|l| {
            let key: String = l.chars().take(80).collect();
            !existing.contains(&key)
        })
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

#[cfg(test)]
#[path = "regression_183.rs"]
mod regression_183;

#[cfg(test)]
#[path = "regression_global_merge.rs"]
mod regression_global_merge;
