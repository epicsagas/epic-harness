//! migrate.rs — Legacy JSONL/JSON → SQLite import
//!
//! Runs once on first startup after upgrade. Detects existing file-based data
//! and imports it into the new SQLite tables. Original files are NOT deleted.

use rusqlite::Connection;
use std::io::{self, BufRead};

/// Run legacy migration if not already done.
/// Called from `open_harness_db()` after schema init.
pub fn run(conn: &Connection) {
    let migrated: bool = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");

    if migrated {
        return;
    }

    match do_migrate(conn) {
        Ok(stats) => {
            eprintln!(
                "[migrate] legacy import complete: {} obs, {} sessions, {} evo, {} metrics errors",
                stats.obs_imported, stats.sess_imported, stats.evo_imported, stats.errors
            );
        }
        Err(e) => {
            eprintln!("[migrate] legacy import failed: {e}");
        }
    }
}

/// Migration statistics for diagnostics.
struct MigrationStats {
    obs_imported: usize,
    sess_imported: usize,
    evo_imported: usize,
    errors: usize,
}

fn do_migrate(conn: &Connection) -> io::Result<MigrationStats> {
    let harness_dir = crate::shared::paths::harness_dir();
    let mut stats = MigrationStats {
        obs_imported: 0,
        sess_imported: 0,
        evo_imported: 0,
        errors: 0,
    };

    // Import observations from obs/*.jsonl
    import_observations(conn, &harness_dir, &mut stats)?;

    // Import sessions from sessions/*.json
    import_sessions(conn, &harness_dir, &mut stats)?;

    // Import evolution from evolution.jsonl
    import_evolution(conn, &harness_dir, &mut stats)?;

    // Import metrics from metrics.json
    import_metrics(conn, &harness_dir, &mut stats)?;

    // Mark as migrated
    conn.execute(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('legacy_migrated', '1')",
        [],
    )
    .map_err(io::Error::other)?;

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

        // Stream lines via BufReader instead of loading entire file into memory
        let file = std::fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);

        // Batch insert in a transaction for large files
        let tx = conn.unchecked_transaction().map_err(io::Error::other)?;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[migrate] read error in {}: {e}", path.display());
                    stats.errors += 1;
                    continue;
                }
            };
            match serde_json::from_str::<crate::shared::obs::ObsRecord>(&line) {
                Ok(rec) => {
                    if let Err(e) =
                        super::observations::insert_observation_conn(&tx, &rec, &session_id)
                    {
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
        tx.commit().map_err(io::Error::other)?;
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
        let millis: i64 = filename
            .strip_prefix("snapshot_")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

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
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[migrate] read evo error: {e}");
                stats.errors += 1;
                continue;
            }
        };
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
    fn run_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();

        // First run (no files to import, but marks migrated)
        run(&conn);
        // Second run should be no-op
        run(&conn);

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
