//! migrate.rs — Legacy JSONL/JSON → SQLite import
//!
//! Invoked explicitly via `epic-harness migrate [--dry-run]`.
//! Original files are NOT deleted after import.

use rusqlite::Connection;
use std::io::{self, BufRead};

use super::{ImmediateTx, store_err};

/// Entry point for the `migrate` subcommand.
///
/// `dry_run = true` scans files and reports what would be imported without writing anything.
/// Returns an exit code (0 = success, 1 = error).
pub fn run_subcommand(dry_run: bool) -> i32 {
    let conn = match super::open_harness_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[migrate] failed to open harness.db: {e}");
            return 1;
        }
    };

    let already_migrated: bool = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");

    if already_migrated {
        println!("already migrated — nothing to do");
        return 0;
    }

    if dry_run {
        return run_dry(&conn);
    }

    match do_migrate(&conn) {
        Ok(stats) => {
            let error_pct = if stats.total_lines > 0 {
                stats.errors as f64 / stats.total_lines as f64 * 100.0
            } else {
                0.0
            };
            if error_pct > 10.0 {
                eprintln!(
                    "[migrate] WARNING: high error rate {:.1}% ({} errors / {} lines) — \
                     some legacy data may not have been imported",
                    error_pct, stats.errors, stats.total_lines
                );
            }
            println!(
                "migration complete: {} obs, {} sessions, {} evo records imported ({} errors, {:.1}%)",
                stats.obs_imported,
                stats.sess_imported,
                stats.evo_imported,
                stats.errors,
                error_pct,
            );
            0
        }
        Err(e) => {
            eprintln!("[migrate] import failed: {e}");
            1
        }
    }
}

/// Scan legacy files and report counts without importing.
fn run_dry(conn: &Connection) -> i32 {
    let harness_dir = crate::shared::paths::harness_dir();

    let obs_count = count_jsonl_lines(&harness_dir.join("obs"));
    let sess_count = count_json_files(&harness_dir.join("sessions"));
    let evo_count = count_jsonl_lines_single(&harness_dir.join("evolution.jsonl"));
    let _ = conn; // ensure conn is used (schema already initialised)

    println!(
        "dry-run: would import ~{obs_count} obs lines, ~{sess_count} session files, ~{evo_count} evo records"
    );
    println!("run without --dry-run to perform the import");
    0
}

fn count_jsonl_lines(dir: &std::path::Path) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .map(|e| {
            std::fs::File::open(e.path())
                .map(|f| std::io::BufReader::new(f).lines().count())
                .unwrap_or(0)
        })
        .sum()
}

fn count_json_files(dir: &std::path::Path) -> usize {
    if !dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count()
}

fn count_jsonl_lines_single(path: &std::path::Path) -> usize {
    std::fs::File::open(path)
        .map(|f| std::io::BufReader::new(f).lines().count())
        .unwrap_or(0)
}

/// Migration statistics for diagnostics.
pub(crate) struct MigrationStats {
    obs_imported: usize,
    sess_imported: usize,
    evo_imported: usize,
    errors: usize,
    /// Total lines/files attempted (for error rate calculation).
    total_lines: usize,
}

pub(crate) fn do_migrate(conn: &Connection) -> io::Result<MigrationStats> {
    let harness_dir = crate::shared::paths::harness_dir();
    let mut stats = MigrationStats {
        obs_imported: 0,
        sess_imported: 0,
        evo_imported: 0,
        errors: 0,
        total_lines: 0,
    };

    // BEGIN IMMEDIATE acquires a write lock upfront, serializing concurrent migration
    // attempts. ImmediateTx provides RAII rollback on drop for safety.
    let tx = ImmediateTx::begin(conn)?;

    // Re-check flag inside the write lock — a concurrent opener may have set it
    // between our outer read above and this BEGIN IMMEDIATE.
    let already: bool = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");
    if already {
        // tx drops → auto-ROLLBACK via ImmediateTx
        return Ok(stats);
    }

    // Import observations from obs/*.jsonl
    import_observations(conn, &harness_dir, &mut stats)?;
    // Import sessions from sessions/*.json
    import_sessions(conn, &harness_dir, &mut stats)?;
    // Import evolution from evolution.jsonl
    import_evolution(conn, &harness_dir, &mut stats)?;
    // Import metrics from metrics.json
    import_metrics(conn, &harness_dir, &mut stats)?;

    // Mark as migrated and commit atomically
    store_err(conn.execute(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('legacy_migrated', '1')",
        [],
    ))?;
    tx.commit()?;

    Ok(stats)
}

fn import_observations(
    conn: &Connection,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let obs_dir = harness_dir.join("obs");
    if !obs_dir.is_dir() {
        return Ok(());
    }

    // Prepare INSERT statement once for all observation rows.
    let mut insert_stmt = store_err(conn.prepare(
        "INSERT INTO observations
         (timestamp, session_id, tool, tool_category, action, result, score,
          dim_success, dim_quality, dim_cost, failure_category, error_snippet,
          file_ext, sequence_id, pipeline_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
    ))?;

    let entries = std::fs::read_dir(&obs_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        // Extract session_id from filename: session_{YYYYMMDD}_{PID}.jsonl → {YYYYMMDD}_{PID}
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let session_id = filename
            .strip_prefix("session_")
            .unwrap_or(filename)
            .to_string();

        // Stream lines via BufReader instead of loading entire file into memory.
        // The outer do_migrate() already holds a BEGIN IMMEDIATE transaction.
        let file = std::fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);

        for line in reader.lines() {
            stats.total_lines += 1;
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[migrate] read error in {}: {e}", path.display());
                    stats.errors += 1;
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<crate::shared::obs::ObsRecord>(&line) {
                Ok(rec) => {
                    let (dim_s, dim_q, dim_c) = match &rec.dimensions {
                        Some(d) => (
                            Some(d.tool_success),
                            Some(d.output_quality),
                            Some(d.execution_cost),
                        ),
                        None => (None, None, None),
                    };
                    let result = insert_stmt.execute(rusqlite::params![
                        rec.timestamp,
                        session_id,
                        rec.tool,
                        rec.tool_category,
                        rec.action,
                        rec.result,
                        rec.score,
                        dim_s,
                        dim_q,
                        dim_c,
                        rec.failure_category,
                        rec.error_snippet,
                        rec.file_ext,
                        rec.sequence_id.map(super::u64_to_i64),
                        rec.pipeline_id,
                    ]);
                    if let Err(e) = result {
                        eprintln!("[migrate] insert obs error: {e}");
                        stats.errors += 1;
                    } else {
                        stats.obs_imported += 1;
                    }
                }
                Err(e) => {
                    eprintln!("[migrate] parse obs error: {e}");
                    stats.errors += 1;
                }
            }
        }
    }
    Ok(())
}

fn import_sessions(
    conn: &Connection,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let sessions_dir = harness_dir.join("sessions");
    if !sessions_dir.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(&sessions_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Extract millis from filename: snapshot_{millis}.json
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let millis: i64 = match filename
            .strip_prefix("snapshot_")
            .and_then(|s| s.parse().ok())
        {
            Some(ms) => ms,
            None => {
                eprintln!(
                    "[migrate] cannot parse millis from session filename '{}' — skipping",
                    path.display()
                );
                stats.errors += 1;
                continue;
            }
        };

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<crate::shared::types::SessionSnapshot>(&content) {
                    Ok(snap) => {
                        if let Err(e) = super::sessions::insert_snapshot_conn(conn, &snap, millis) {
                            eprintln!("[migrate] insert session error: {e}");
                            stats.errors += 1;
                        } else {
                            stats.sess_imported += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("[migrate] parse session error in {}: {e}", path.display());
                        stats.errors += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("[migrate] read session error {}: {e}", path.display());
                stats.errors += 1;
            }
        }
    }
    Ok(())
}

fn import_evolution(
    conn: &Connection,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let evo_file = harness_dir.join("evolution.jsonl");
    if !evo_file.exists() {
        return Ok(());
    }

    // Stream via BufReader
    let file = std::fs::File::open(&evo_file)?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines() {
        stats.total_lines += 1;
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[migrate] read evo error: {e}");
                stats.errors += 1;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<crate::shared::evolution::EvolutionRecord>(&line) {
            Ok(rec) => {
                if let Err(e) = super::evolution::insert_record_conn(conn, &rec) {
                    eprintln!("[migrate] insert evo error: {e}");
                    stats.errors += 1;
                } else {
                    stats.evo_imported += 1;
                }
            }
            Err(e) => {
                eprintln!("[migrate] parse evo error: {e}");
                stats.errors += 1;
            }
        }
    }
    Ok(())
}

fn import_metrics(
    conn: &Connection,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let metrics_file = harness_dir.join("metrics.json");
    if !metrics_file.exists() {
        return Ok(());
    }

    match std::fs::read_to_string(&metrics_file) {
        Ok(content) => {
            let metrics = serde_json::from_str::<crate::shared::evolution::Metrics>(&content)
                .unwrap_or_else(|e| {
                    eprintln!("[migrate] parse metrics error: {e}");
                    crate::shared::evolution::default_metrics()
                });
            if let Err(e) = super::metrics::save_metrics_conn(conn, &metrics) {
                eprintln!("[migrate] save metrics error: {e}");
                stats.errors += 1;
            }
        }
        Err(e) => {
            eprintln!("[migrate] read metrics error: {e}");
            stats.errors += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn do_migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();

        // First run marks legacy_migrated=1
        do_migrate(&conn).unwrap();
        // Second run must see the flag and be a no-op (returns empty stats)
        let stats = do_migrate(&conn).unwrap();
        assert_eq!(stats.obs_imported, 0);
        assert_eq!(stats.sess_imported, 0);

        let migrated: String = conn
            .query_row(
                "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migrated, "1");
    }
}
