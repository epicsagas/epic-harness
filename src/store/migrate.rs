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
/// `reset = true` clears an `in_progress` marker left by a previously interrupted migration,
/// allowing the import to be retried without manual DB surgery.
/// Returns an exit code (0 = success, 1 = error).
pub fn run_subcommand(dry_run: bool, reset: bool) -> i32 {
    let conn = match super::open_harness_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[migrate] failed to open harness.db: {e}");
            return 1;
        }
    };

    let migration_state: Option<String> = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();

    match migration_state.as_deref() {
        Some("1") => {
            println!("already migrated — nothing to do");
            return 0;
        }
        Some("in_progress") => {
            if reset {
                if let Err(e) = conn.execute(
                    "DELETE FROM _harness_meta WHERE key = 'legacy_migrated'",
                    [],
                ) {
                    eprintln!("[migrate] failed to clear in_progress marker: {e}");
                    return 1;
                }
                println!("[migrate] cleared interrupted migration marker — retrying");
            } else {
                eprintln!(
                    "[migrate] a previous migration was interrupted. \
                     Re-run with --reset to clear the marker and retry."
                );
                return 1;
            }
        }
        _ => {}
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

    // Phase 1: Reserve migration under write lock. ImmediateTx serializes concurrent
    // callers; re-checking the flag inside the lock closes the TOCTOU window between
    // the outer read in run_subcommand and this BEGIN IMMEDIATE.
    {
        let tx = ImmediateTx::begin(conn)?;
        let already: bool = conn
            .query_row(
                "SELECT value FROM _harness_meta WHERE key = 'legacy_migrated'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .is_some_and(|v| v == "1" || v == "in_progress");
        if already {
            return Ok(stats); // tx drops → auto-ROLLBACK
        }
        store_err(conn.execute(
            "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('legacy_migrated', 'in_progress')",
            [],
        ))?;
        tx.commit()?;
    }

    // Phase 2: Per-file imports. Each source file gets its own ImmediateTx so the WAL
    // never grows unboundedly for large legacy datasets. The 'in_progress' marker set
    // above prevents concurrent migration attempts from racing with these small commits.
    import_observations(conn, &harness_dir, &mut stats)?;
    let slug = crate::shared::paths::project_slug();
    import_sessions(conn, &slug, &harness_dir, &mut stats)?;
    import_evolution(conn, &slug, &harness_dir, &mut stats)?;
    import_metrics(conn, &slug, &harness_dir, &mut stats)?;

    // Phase 3: Mark complete.
    {
        let tx = ImmediateTx::begin(conn)?;
        store_err(conn.execute(
            "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('legacy_migrated', '1')",
            [],
        ))?;
        tx.commit()?;
    }

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

        // Each file gets its own ImmediateTx to bound WAL growth.
        // do_migrate() released its outer tx before calling this function.
        let tx = ImmediateTx::begin(conn)?;

        let mut insert_stmt = store_err(conn.prepare(
            "INSERT INTO observations
             (timestamp, session_id, tool, tool_category, action, result, score,
              dim_success, dim_quality, dim_cost, failure_category, error_snippet,
              file_ext, sequence_id, pipeline_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        ))?;

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

        // Explicitly drop the prepared statement before committing —
        // Statement holds a shared borrow of conn but commit() also needs conn.
        drop(insert_stmt);
        tx.commit()?;
    }
    Ok(())
}

fn import_sessions(
    conn: &Connection,
    slug: &str,
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

        // Parse before opening the transaction to avoid holding a write lock
        // while doing I/O — consistent with import_observations per-file pattern.
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[migrate] read session error {}: {e}", path.display());
                stats.errors += 1;
                continue;
            }
        };
        let snap = match serde_json::from_str::<crate::shared::types::SessionSnapshot>(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[migrate] parse session error in {}: {e}", path.display());
                stats.errors += 1;
                continue;
            }
        };

        // Each file gets its own ImmediateTx to bound WAL growth.
        let tx = ImmediateTx::begin(conn)?;
        let pending_json = serde_json::to_string(&snap.pending_tasks).unwrap_or_else(|e| {
            eprintln!("[migrate] pending_tasks serialization failed: {e}");
            "[]".into()
        });
        let pipeline_json = snap.pipeline_state.as_ref().map(|v| {
            serde_json::to_string(v).unwrap_or_else(|e| {
                eprintln!("[migrate] pipeline_state serialization failed: {e}");
                "{}".into()
            })
        });
        let result = store_err(conn.execute(
            "INSERT INTO sessions (timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state, created_at_millis, project) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                snap.timestamp, snap.snap_type, snap.summary, pending_json,
                snap.context_usage, pipeline_json, millis, slug,
            ],
        ));
        match result {
            Ok(_) => {
                stats.sess_imported += 1;
                tx.commit()?;
            }
            Err(e) => {
                eprintln!("[migrate] insert session error: {e}");
                stats.errors += 1;
                // tx drops → ROLLBACK; no partial state committed for this file
            }
        }
    }
    Ok(())
}

fn import_evolution(
    conn: &Connection,
    slug: &str,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let evo_file = harness_dir.join("evolution.jsonl");
    if !evo_file.exists() {
        return Ok(());
    }

    // Stream via BufReader with batched transactions (BATCH_SIZE records per commit)
    // to cap WAL growth. import_observations uses per-file transactions for the same
    // reason; evolution.jsonl is a single large file so we batch by record count.
    const BATCH_SIZE: usize = 500;
    let file = std::fs::File::open(&evo_file)?;
    let reader = std::io::BufReader::new(file);

    let mut records_in_batch: usize = 0;
    let mut current_tx = ImmediateTx::begin(conn)?;

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
                let error_json = serde_json::to_string(&rec.error_patterns).unwrap_or_else(|e| {
                    eprintln!("[migrate] error_patterns serialization failed: {e}");
                    "{}".into()
                });
                let failure_json =
                    serde_json::to_string(&rec.failure_patterns).unwrap_or_else(|e| {
                        eprintln!("[migrate] failure_patterns serialization failed: {e}");
                        "[]".into()
                    });
                let result = store_err(conn.execute(
                    "INSERT INTO evolution_records (timestamp, observations, success_rate, avg_score, error_patterns, failure_patterns, skills_seeded, skills_rolled_back, total_evolved, analysis_summary, project) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    rusqlite::params![
                        rec.timestamp,
                        super::u64_to_i64(rec.observations),
                        rec.success_rate,
                        rec.avg_score,
                        error_json,
                        failure_json,
                        super::u64_to_i64(rec.skills_seeded),
                        super::u64_to_i64(rec.skills_rolled_back),
                        super::u64_to_i64(rec.total_evolved),
                        rec.analysis_summary,
                        slug,
                    ],
                ));
                if let Err(e) = result {
                    eprintln!("[migrate] insert evo error: {e}");
                    stats.errors += 1;
                } else {
                    stats.evo_imported += 1;
                    records_in_batch += 1;
                    if records_in_batch >= BATCH_SIZE {
                        current_tx.commit()?;
                        records_in_batch = 0;
                        current_tx = ImmediateTx::begin(conn)?;
                    }
                }
            }
            Err(e) => {
                eprintln!("[migrate] parse evo error: {e}");
                stats.errors += 1;
            }
        }
    }
    current_tx.commit()?;
    Ok(())
}

fn import_metrics(
    conn: &Connection,
    slug: &str,
    harness_dir: &std::path::Path,
    stats: &mut MigrationStats,
) -> io::Result<()> {
    let metrics_file = harness_dir.join("metrics.json");
    if !metrics_file.exists() {
        return Ok(());
    }

    match std::fs::read_to_string(&metrics_file) {
        Ok(content) => {
            let metrics = match serde_json::from_str::<crate::shared::evolution::Metrics>(&content)
            {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[migrate] parse metrics error: {e}");
                    stats.errors += 1;
                    crate::shared::evolution::default_metrics()
                }
            };
            // Inline key-value metrics_state write (replaces deleted save_metrics_conn).
            let kv = |k: &str, v: &str| -> io::Result<()> {
                store_err(conn.execute(
                    "INSERT OR REPLACE INTO metrics_state (key, value, project) VALUES (?1, ?2, ?3)",
                    rusqlite::params![k, v, slug],
                ))?;
                Ok(())
            };
            let result: io::Result<()> = (|| {
                kv("total_sessions", &metrics.total_sessions.to_string())?;
                kv("avg_success_rate", &metrics.avg_success_rate.to_string())?;
                kv(
                    "total_evolved_skills",
                    &metrics.total_evolved_skills.to_string(),
                )?;
                if let Some(ref v) = metrics.last_session {
                    kv("last_session", v)?;
                }
                if let Some(v) = metrics.best_score {
                    kv("best_score", &v.to_string())?;
                }
                kv("best_session", &metrics.best_session)?;
                kv("trend", &metrics.trend)?;
                kv("stagnation_count", &metrics.stagnation_count.to_string())?;
                if let Some(ref v) = metrics.last_error_context {
                    kv("last_error_context", v)?;
                }
                Ok(())
            })();
            if let Err(e) = result {
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

    #[test]
    fn to_global_merges_per_project_dbs() {
        let global_db = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&global_db).unwrap();

        // Simulate a per-project DB (v3 schema — no project column)
        let project_dir = std::env::temp_dir().join("harness-test-to-global");
        let _ = std::fs::create_dir_all(&project_dir);
        let db_path = project_dir.join("harness.db");
        let src_conn = Connection::open(&db_path).unwrap();
        // Apply a minimal v3-like schema manually
        src_conn
            .execute_batch(
                "CREATE TABLE observations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    tool TEXT NOT NULL,
                    tool_category TEXT NOT NULL,
                    action TEXT, result TEXT, score REAL,
                    dim_success REAL, dim_quality REAL, dim_cost REAL,
                    failure_category TEXT, error_snippet TEXT,
                    file_ext TEXT, sequence_id INTEGER, pipeline_id TEXT
                );
                INSERT INTO observations (timestamp, session_id, tool, tool_category)
                VALUES ('2026-01-01T00:00:00Z', 'sess1', 'Bash', 'shell');
                CREATE TABLE sessions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    snap_type TEXT NOT NULL,
                    summary TEXT,
                    snapshot_json TEXT NOT NULL,
                    millis INTEGER NOT NULL
                );
                CREATE TABLE evolution_records (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL, observations INTEGER NOT NULL DEFAULT 0,
                    success_rate REAL NOT NULL, avg_score REAL NOT NULL,
                    error_patterns TEXT NOT NULL DEFAULT '{}',
                    failure_patterns TEXT NOT NULL DEFAULT '[]',
                    skills_seeded INTEGER NOT NULL DEFAULT 0,
                    skills_rolled_back INTEGER NOT NULL DEFAULT 0,
                    total_evolved INTEGER NOT NULL DEFAULT 0,
                    analysis_summary TEXT NOT NULL DEFAULT ''
                );",
            )
            .unwrap();
        drop(src_conn);

        // Attach and merge
        let slug = "test-project";
        let escaped_path = db_path.display().to_string().replace('\'', "''");
        global_db
            .execute(&format!("ATTACH '{escaped_path}' AS src"), [])
            .unwrap();

        let merged = super::merge_attached_db(&global_db, slug, "src").unwrap();
        global_db.execute("DETACH src", []).unwrap();

        assert_eq!(merged.obs, 1, "should merge 1 observation");
        assert_eq!(merged.sessions, 0, "sessions table was empty");

        // Verify the project column is set
        let count: i64 = global_db
            .query_row(
                "SELECT COUNT(*) FROM observations WHERE project = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let _ = std::fs::remove_dir_all(&project_dir);
    }
}

// ── Per-project DB → Global DB consolidation ────────────

/// Merge a per-project harness.db into the global DB using ATTACH.
///
/// `attach_name` is the SQLite schema name used in ATTACH (e.g., "src").
/// All data from the attached DB is copied with `project` set to `slug`.
/// Tables that already have rows for this project are skipped (INSERT OR IGNORE).
pub fn merge_attached_db(
    conn: &Connection,
    slug: &str,
    attach_name: &str,
) -> io::Result<GlobalMergeStats> {
    let mut stats = GlobalMergeStats::default();

    // Tables with simple columns — direct INSERT with project appended.
    let simple_tables = [
        // (table, columns_without_project, src_columns_without_project)
        (
            "observations",
            "timestamp, session_id, tool, tool_category, action, result, score, \
          dim_success, dim_quality, dim_cost, failure_category, error_snippet, \
          file_ext, sequence_id, pipeline_id, project",
            "timestamp, session_id, tool, tool_category, action, result, score, \
          dim_success, dim_quality, dim_cost, failure_category, error_snippet, \
          file_ext, sequence_id, pipeline_id",
        ),
        (
            "sessions",
            "timestamp, snap_type, summary, snapshot_json, millis, project",
            "timestamp, snap_type, summary, snapshot_json, millis",
        ),
        (
            "evolution_records",
            "timestamp, observations, success_rate, avg_score, error_patterns, \
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved, \
          analysis_summary, project",
            "timestamp, observations, success_rate, avg_score, error_patterns, \
          failure_patterns, skills_seeded, skills_rolled_back, total_evolved, \
          analysis_summary",
        ),
        (
            "orch_runs",
            "id, status, agents_json, dep_graph_json, created_at, updated_at, project",
            "id, status, agents_json, dep_graph_json, created_at, updated_at",
        ),
        (
            "orch_control",
            "action, target, message, generation, project",
            "action, target, message, generation",
        ),
    ];

    for (table, dest_cols, src_cols) in &simple_tables {
        let sql = format!(
            "INSERT OR IGNORE INTO main.{table} ({dest_cols}) \
             SELECT {src_cols}, ?1 FROM {attach_name}.{table}"
        );
        match conn.execute(&sql, rusqlite::params![slug]) {
            Ok(n) => match *table {
                "observations" => stats.obs += n,
                "sessions" => stats.sessions += n,
                "evolution_records" => stats.evo += n,
                _ => {}
            },
            Err(e) => {
                // Table may not exist in the source DB — skip silently.
                if !e.to_string().contains("no such table") {
                    eprintln!("[migrate/to-global] {table}: {e}");
                }
            }
        }
    }

    // metrics_state: key-value with project
    if let Ok(n) = conn.execute(
        &format!(
            "INSERT OR IGNORE INTO main.metrics_state (key, value, project) \
             SELECT key, value, ?1 FROM {attach_name}.metrics_state"
        ),
        rusqlite::params![slug],
    ) {
        stats.metrics += n;
    }

    // score_history: has UNIQUE(timestamp) so INSERT OR IGNORE deduplicates
    if let Ok(n) = conn.execute(
        &format!(
            "INSERT OR IGNORE INTO main.score_history \
             (timestamp, success_rate, avg_score, observations, \
              dim_success, dim_quality, dim_cost, project) \
             SELECT timestamp, success_rate, avg_score, observations, \
                    dim_success, dim_quality, dim_cost, ?1 \
             FROM {attach_name}.score_history"
        ),
        rusqlite::params![slug],
    ) {
        stats.score_history += n;
    }

    // skill_attribution: composite PK (skill_name, project) — safe to INSERT OR IGNORE
    if let Ok(n) = conn.execute(
        &format!(
            "INSERT OR IGNORE INTO main.skill_attribution \
             (skill_name, project, sessions_active, avg_score_with, avg_score_without, first_seen) \
             SELECT skill_name, ?1, sessions_active, avg_score_with, avg_score_without, first_seen \
             FROM {attach_name}.skill_attribution"
        ),
        rusqlite::params![slug],
    ) {
        stats.skill_attr += n;
    }

    // promotion_counters: composite PK (pattern_key, project)
    if let Ok(n) = conn.execute(
        &format!(
            "INSERT OR IGNORE INTO main.promotion_counters (pattern_key, project, count) \
             SELECT pattern_key, ?1, count FROM {attach_name}.promotion_counters"
        ),
        rusqlite::params![slug],
    ) {
        stats.promo += n;
    }

    // orch_agents, orch_agent_events, orch_agent_inbox: FK-linked to orch_runs
    for table in ["orch_agents", "orch_agent_events", "orch_agent_inbox"] {
        let _ = conn.execute(
            &format!("INSERT OR IGNORE INTO main.{table} SELECT * FROM {attach_name}.{table}"),
            [],
        );
    }

    Ok(stats)
}

/// Entry point for `epic-harness migrate --to-global [--dry-run]`.
///
/// Scans `~/.harness/projects/*/harness.db` and merges each into the global DB.
/// Returns an exit code (0 = success, 1 = error).
pub fn run_to_global(dry_run: bool) -> i32 {
    let projects_root = crate::shared::paths::harness_projects_root();
    if !projects_root.is_dir() {
        println!("no per-project directories found — nothing to do");
        return 0;
    }

    // Collect candidate per-project DBs
    let mut candidates: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&projects_root)
        .into_iter()
        .flatten()
        .flatten()
    {
        let slug_dir = entry.path();
        if !slug_dir.is_dir() {
            continue;
        }
        let db_path = slug_dir.join("harness.db");
        if !db_path.is_file() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().into_owned();
        candidates.push((slug, db_path));
    }

    if candidates.is_empty() {
        println!("no per-project harness.db files found — nothing to do");
        return 0;
    }

    if dry_run {
        println!(
            "dry-run: would merge {} per-project DB(s) into global harness.db:",
            candidates.len()
        );
        for (slug, path) in &candidates {
            println!("  {slug} ← {}", path.display());
        }
        println!("run without --dry-run to perform the merge");
        return 0;
    }

    // Open global DB
    let conn = match super::open_harness_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[migrate/to-global] failed to open global harness.db: {e}");
            return 1;
        }
    };

    let mut total = GlobalMergeStats::default();
    for (slug, db_path) in &candidates {
        // ATTACH the per-project DB read-only
        let attach_name = "src";
        let escaped_path = db_path.display().to_string().replace('\'', "''");
        if let Err(e) = conn.execute(&format!("ATTACH '{escaped_path}' AS {attach_name}"), []) {
            eprintln!("[migrate/to-global] ATTACH failed for {slug}: {e}");
            continue;
        }

        match merge_attached_db(&conn, slug, attach_name) {
            Ok(stats) => {
                println!(
                    "  {slug}: {} obs, {} sessions, {} evo, {} metrics, {} score_history",
                    stats.obs, stats.sessions, stats.evo, stats.metrics, stats.score_history
                );
                total += stats;
            }
            Err(e) => {
                eprintln!("[migrate/to-global] merge failed for {slug}: {e}");
            }
        }

        let _ = conn.execute("DETACH src", []);
    }

    // Mark consolidation complete
    if let Err(e) = conn.execute(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('global_consolidated', '1')",
        [],
    ) {
        eprintln!("[migrate/to-global] failed to set consolidation marker: {e}");
    }

    println!(
        "\nconsolidation complete: {} obs, {} sessions, {} evo records across {} project(s)",
        total.obs,
        total.sessions,
        total.evo,
        candidates.len()
    );
    0
}

#[derive(Default)]
/// Statistics for per-project → global DB merge.
pub struct GlobalMergeStats {
    pub obs: usize,
    pub sessions: usize,
    pub evo: usize,
    pub metrics: usize,
    pub score_history: usize,
    pub skill_attr: usize,
    pub promo: usize,
}

impl std::ops::AddAssign for GlobalMergeStats {
    fn add_assign(&mut self, other: Self) {
        self.obs += other.obs;
        self.sessions += other.sessions;
        self.evo += other.evo;
        self.metrics += other.metrics;
        self.score_history += other.score_history;
        self.skill_attr += other.skill_attr;
        self.promo += other.promo;
    }
}

// ── Slug normalization (hash suffix removal) ────────────

/// Detect old-format slugs (`{name}-{6hex}`) and return the name part.
fn strip_hash_suffix(slug: &str) -> Option<&str> {
    if slug.len() > 7 {
        let maybe_sep = &slug[slug.len() - 7..slug.len() - 6];
        let maybe_hex = &slug[slug.len() - 6..];
        if maybe_sep == "-" && maybe_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let name = &slug[..slug.len() - 7];
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Auto-normalize old hashed slugs to name-only format.
///
/// Called from `init_schema()` after migrations. Idempotent — tracked via
/// `_harness_meta.slugs_normalized`.
pub fn normalize_slugs_if_needed(conn: &Connection) -> io::Result<()> {
    // Check if already done.
    let done: bool = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'slugs_normalized'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .is_some_and(|v| v == "1");

    if done {
        return Ok(());
    }

    // Collect all distinct project values that need normalization.
    let slugs = collect_distinct_projects(conn)?;
    let mapping = build_normalization_mapping(&slugs);

    if mapping.is_empty() {
        // No hashed slugs found — just mark as done.
        store_err(conn.execute(
            "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('slugs_normalized', '1')",
            [],
        ))?;
        return Ok(());
    }

    eprintln!(
        "[harness] normalizing {} hashed slug(s) to name-only format",
        mapping.len()
    );

    // Apply within a transaction.
    let tx = super::ImmediateTx::begin(conn)?;
    apply_slug_mapping(conn, &mapping)?;
    store_err(conn.execute(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('slugs_normalized', '1')",
        [],
    ))?;
    tx.commit()?;

    // Rename per-project directories.
    rename_slug_directories(&mapping);

    // Normalize memory.db projects CSV.
    normalize_memory_projects(&mapping);

    Ok(())
}

/// Collect all distinct `project` values across all relevant tables.
fn collect_distinct_projects(conn: &Connection) -> io::Result<Vec<String>> {
    let tables = [
        "observations",
        "sessions",
        "evolution_records",
        "metrics_state",
        "score_history",
        "skill_attribution",
        "promotion_counters",
        "workspace_manifest",
        "orch_runs",
        "orch_control",
        "orbit_pipelines",
        "evolved_skills",
        "global_patterns",
    ];
    let mut seen = std::collections::HashSet::new();
    for table in &tables {
        let rows: Vec<String> = conn
            .prepare(&format!("SELECT DISTINCT project FROM {table}"))
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .query_map([], |row| row.get(0))
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        for slug in rows {
            seen.insert(slug);
        }
    }
    Ok(seen.into_iter().collect())
}

/// Build a mapping of `{old_hashed_slug} → {name_only_slug}`.
fn build_normalization_mapping(slugs: &[String]) -> Vec<(String, String)> {
    slugs
        .iter()
        .filter_map(|s| strip_hash_suffix(s).map(|name| (s.clone(), name.to_string())))
        .collect()
}

/// Tables where `project` is part of a composite PK (need special handling).
const COMPOSITE_PK_TABLES: &[&str] = &["skill_attribution", "promotion_counters"];

/// Tables with simple project column (UPDATE works directly).
const SIMPLE_TABLES: &[&str] = &[
    "observations",
    "sessions",
    "evolution_records",
    "metrics_state",
    "score_history",
    "workspace_manifest",
    "orch_runs",
    "orch_control",
    "orbit_pipelines",
    "evolved_skills",
    "global_patterns",
];

/// Apply slug mapping to all tables in harness.db.
fn apply_slug_mapping(conn: &Connection, mapping: &[(String, String)]) -> io::Result<()> {
    for (old, new) in mapping {
        // Simple tables: direct UPDATE.
        for table in SIMPLE_TABLES {
            store_err(conn.execute(
                &format!("UPDATE {table} SET project = ?1 WHERE project = ?2"),
                rusqlite::params![new, old],
            ))?;
        }

        // Composite PK tables: temp-table approach to handle PK conflicts
        // when two hashed slugs map to the same name-only slug.
        for table in COMPOSITE_PK_TABLES {
            // Step 1: Copy matching rows to temp table.
            store_err(conn.execute(
                &format!("CREATE TEMP TABLE _norm_tmp AS SELECT * FROM {table} WHERE project = ?1"),
                rusqlite::params![old],
            ))?;
            // Step 2: Delete originals.
            store_err(conn.execute(
                &format!("DELETE FROM {table} WHERE project = ?1"),
                rusqlite::params![old],
            ))?;
            // Step 3: Update project in temp.
            store_err(conn.execute("UPDATE _norm_tmp SET project = ?1", rusqlite::params![new]))?;
            // Step 4: Insert back (IGNORE handles conflicts with existing rows).
            store_err(conn.execute_batch(&format!(
                "INSERT OR IGNORE INTO {table} SELECT * FROM _norm_tmp; DROP TABLE _norm_tmp;"
            )))?;
        }
    }
    Ok(())
}

/// Rename per-project directories from hashed to name-only slugs.
fn rename_slug_directories(mapping: &[(String, String)]) {
    let root = crate::shared::paths::harness_projects_root();
    if !root.is_dir() {
        return;
    }

    for (old, new) in mapping {
        let old_dir = root.join(old);
        let new_dir = root.join(new);

        if !old_dir.is_dir() {
            continue;
        }

        if new_dir.is_dir() {
            // Target exists — merge contents.
            if let Ok(entries) = std::fs::read_dir(&old_dir) {
                for entry in entries.flatten() {
                    let dest = new_dir.join(entry.file_name());
                    if dest.exists() {
                        // Merge subdirectory contents recursively.
                        if entry.path().is_dir() {
                            merge_dir_recursive(&entry.path(), &dest);
                        }
                        // Skip existing files (keep both — newer wins is not worth complexity).
                    } else {
                        let _ = std::fs::rename(entry.path(), &dest);
                    }
                }
            }
            // Remove old dir if empty.
            let _ = std::fs::remove_dir(&old_dir);
        } else {
            // Simple rename.
            let _ = std::fs::rename(&old_dir, &new_dir);
        }
    }
}

/// Recursively merge contents of `src` into `dst`.
fn merge_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    let _ = std::fs::create_dir_all(dst);
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let dest = dst.join(entry.file_name());
            if entry.path().is_dir() {
                merge_dir_recursive(&entry.path(), &dest);
            } else if !dest.exists() {
                let _ = std::fs::rename(entry.path(), &dest);
            }
        }
    }
    let _ = std::fs::remove_dir(src);
}

/// Normalize projects CSV in memory.db nodes table.
fn normalize_memory_projects(mapping: &[(String, String)]) {
    if mapping.is_empty() {
        return;
    }
    let db_path = crate::shared::paths::dirs_home()
        .join(".harness")
        .join("memory.db");
    if !db_path.is_file() {
        return;
    }
    let conn =
        match Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE) {
            Ok(c) => c,
            Err(_) => return,
        };

    let mapping_map: std::collections::HashMap<&str, &str> = mapping
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut stmt = match conn.prepare("SELECT id, projects FROM nodes") {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows: Vec<(String, String)> = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => return,
    };

    for (id, csv) in &rows {
        let new_csv = remap_csv(csv, &mapping_map);
        if new_csv != *csv {
            let _ = conn.execute(
                "UPDATE nodes SET projects = ?1 WHERE id = ?2",
                rusqlite::params![new_csv, id],
            );
        }
    }

    // Update FTS.
    let _ = conn.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');");
}

/// Remap slug values in a comma-separated CSV string.
fn remap_csv(csv: &str, mapping: &std::collections::HashMap<&str, &str>) -> String {
    if csv.is_empty() {
        return csv.to_string();
    }
    csv.split(',')
        .map(|s| {
            let trimmed = s.trim();
            mapping
                .get(trimmed)
                .map(|v| v.to_string())
                .unwrap_or_else(|| trimmed.to_string())
        })
        .collect::<Vec<_>>()
        .join(",")
}
