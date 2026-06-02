//! migrate.rs — Legacy JSONL/JSON → SQLite import
//!
//! Runs once on first startup after upgrade. Detects existing file-based data
//! and imports it into the new SQLite tables. Original files are NOT deleted.

use rusqlite::Connection;
use std::io;

/// Run legacy migration if not already done.
/// Called from `open_harness_db()` after schema init.
pub fn run(conn: &Connection) {
    let migrated: bool = conn
        .query_row(
            "SELECT value FROM _meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");

    if migrated {
        return;
    }

    let _ = do_migrate(conn);
}

fn do_migrate(conn: &Connection) -> io::Result<()> {
    let harness_dir = crate::shared::paths::harness_dir();

    // Import observations from obs/*.jsonl
    import_observations(conn, &harness_dir)?;

    // Import sessions from sessions/*.json
    import_sessions(conn, &harness_dir)?;

    // Import evolution from evolution.jsonl
    import_evolution(conn, &harness_dir)?;

    // Import metrics from metrics.json
    import_metrics(conn, &harness_dir)?;

    // Mark as migrated
    conn.execute(
        "INSERT OR REPLACE INTO _meta (key, value) VALUES ('legacy_migrated', '1')",
        [],
    )
    .map_err(io::Error::other)?;

    Ok(())
}

fn import_observations(conn: &Connection, harness_dir: &std::path::Path) -> io::Result<()> {
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

        let content = std::fs::read_to_string(&path)?;
        for line in content.lines() {
            if let Ok(rec) = serde_json::from_str::<crate::shared::obs::ObsRecord>(line) {
                let _ = super::observations::insert_observation_conn(conn, &rec, &session_id);
            }
        }
    }
    Ok(())
}

fn import_sessions(conn: &Connection, harness_dir: &std::path::Path) -> io::Result<()> {
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

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(snap) =
                serde_json::from_str::<crate::shared::types::SessionSnapshot>(&content)
            {
                let _ = super::sessions::insert_snapshot_conn(conn, &snap, millis);
            }
        }
    }
    Ok(())
}

fn import_evolution(conn: &Connection, harness_dir: &std::path::Path) -> io::Result<()> {
    let evo_file = harness_dir.join("evolution.jsonl");
    if !evo_file.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&evo_file)?;
    for line in content.lines() {
        if let Ok(rec) = serde_json::from_str::<crate::shared::evolution::EvolutionRecord>(line) {
            let _ = super::evolution::insert_record_conn(conn, &rec);
        }
    }
    Ok(())
}

fn import_metrics(conn: &Connection, harness_dir: &std::path::Path) -> io::Result<()> {
    let metrics_file = harness_dir.join("metrics.json");
    if !metrics_file.exists() {
        return Ok(());
    }

    if let Ok(content) = std::fs::read_to_string(&metrics_file) {
        let metrics = serde_json::from_str::<crate::shared::evolution::Metrics>(&content)
            .unwrap_or_else(|_| crate::shared::evolution::default_metrics());
        let _ = super::metrics::save_metrics_conn(conn, &metrics);
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

        // First run
        run(&conn);
        // Second run should be no-op
        run(&conn);

        let migrated: String = conn
            .query_row(
                "SELECT value FROM _meta WHERE key = 'legacy_migrated'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migrated, "1");
    }
}
