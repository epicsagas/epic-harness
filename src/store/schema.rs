//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, evolved skills, and global patterns.

use rusqlite::Connection;
use sqlx::{Row, SqlitePool};
use std::io;

use super::store_err;

/// Current schema version. Increment when adding migrations in `run_migrations`.
const SCHEMA_VERSION: u32 = 4;

/// Apply the full operational schema to an open connection.
///
/// On first run (no `_harness_meta` table): applies all DDL + PRAGMA.
/// On subsequent runs: skips PRAGMA/DDL if schema version matches, only runs
/// pending migrations for version bumps.
pub(crate) fn init_schema(conn: &Connection) -> io::Result<()> {
    // Check if schema already initialised by probing for the meta table.
    let existing_version: Option<u32> = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok());

    if let Some(version) = existing_version {
        // Schema exists — just ensure PRAGMA are set and run pending migrations.
        apply_pragma(conn)?;
        if version < SCHEMA_VERSION {
            run_migrations(conn, version, SCHEMA_VERSION)?;
        }
        // After migrations, apply DDL to create any tables/indexes added in newer
        // versions that the migration itself didn't create (e.g., because a table
        // was added in v4 but the DB started at v2 with a minimal schema subset).
        // IF NOT EXISTS makes this a no-op for tables that already exist.
        apply_ddl(conn)?;
        // Normalize old hashed slugs to name-only format (idempotent).
        if let Err(e) = super::migrate::normalize_slugs_if_needed(conn) {
            eprintln!("[harness] slug normalization failed: {e}");
        }
        return Ok(());
    }

    // First run: apply everything.
    apply_pragma(conn)?;
    apply_ddl(conn)?;
    set_version(conn, SCHEMA_VERSION)?;

    Ok(())
}

/// Apply WAL, FK, and autocheckpoint PRAGMA.
fn apply_pragma(conn: &Connection) -> io::Result<()> {
    store_err(conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         PRAGMA wal_autocheckpoint=100;
         PRAGMA busy_timeout=5000;",
    ))
}

/// Apply the full DDL (tables + indexes). Uses IF NOT EXISTS throughout.
///
/// **Keep in sync with `init_schema_pool`** at the bottom of this file —
/// any DDL change here must be mirrored there.
fn apply_ddl(conn: &Connection) -> io::Result<()> {
    // Schema version tracking (distinct from memory.db's _meta)
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _harness_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    ))?;

    // ── Observations ──────────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS observations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp        TEXT NOT NULL,
            session_id       TEXT NOT NULL,
            tool             TEXT NOT NULL,
            tool_category    TEXT NOT NULL,
            action           TEXT,
            result           TEXT,
            score            REAL,
            dim_success      REAL,
            dim_quality      REAL,
            dim_cost         REAL,
            failure_category TEXT,
            error_snippet    TEXT,
            file_ext         TEXT,
            sequence_id      INTEGER,
            pipeline_id      TEXT,
            project          TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
        CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
        CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
        CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_obs_project    ON observations(project);",
    ))?;

    // ── Sessions ──────────────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp         TEXT NOT NULL,
            snap_type         TEXT NOT NULL DEFAULT 'pre-compact',
            summary           TEXT NOT NULL DEFAULT '',
            pending_tasks     TEXT NOT NULL DEFAULT '[]',
            context_usage     REAL,
            pipeline_state    TEXT,
            created_at_millis INTEGER NOT NULL,
            project           TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_ts ON sessions(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);",
    ))?;

    // ── Evolution Records ─────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evolution_records (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp         TEXT NOT NULL,
            observations      INTEGER NOT NULL DEFAULT 0,
            success_rate      REAL NOT NULL DEFAULT 0.0,
            avg_score         REAL NOT NULL DEFAULT 0.0,
            error_patterns    TEXT NOT NULL DEFAULT '{}',
            failure_patterns  TEXT NOT NULL DEFAULT '[]',
            skills_seeded     INTEGER NOT NULL DEFAULT 0,
            skills_rolled_back INTEGER NOT NULL DEFAULT 0,
            total_evolved     INTEGER NOT NULL DEFAULT 0,
            analysis_summary  TEXT NOT NULL DEFAULT '',
            project           TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_evo_project ON evolution_records(project);",
    ))?;

    // ── Metrics (3-table normalized) ──────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics_state (
            key     TEXT NOT NULL,
            value   TEXT NOT NULL,
            project TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (key, project)
        );
        CREATE TABLE IF NOT EXISTS score_history (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp    TEXT NOT NULL,
            success_rate REAL NOT NULL,
            avg_score    REAL NOT NULL,
            observations INTEGER NOT NULL DEFAULT 0,
            dim_success  REAL NOT NULL DEFAULT 0.0,
            dim_quality  REAL NOT NULL DEFAULT 0.0,
            dim_cost     REAL NOT NULL DEFAULT 0.0,
            project      TEXT NOT NULL DEFAULT '',
            UNIQUE(timestamp, project)
        );
        CREATE TABLE IF NOT EXISTS skill_attribution (
            skill_name        TEXT NOT NULL,
            project           TEXT NOT NULL DEFAULT '',
            sessions_active   INTEGER NOT NULL DEFAULT 0,
            avg_score_with    REAL NOT NULL DEFAULT 0.0,
            avg_score_without REAL NOT NULL DEFAULT 0.0,
            first_seen        TEXT NOT NULL,
            PRIMARY KEY (skill_name, project)
        );
        CREATE INDEX IF NOT EXISTS idx_score_hist_project ON score_history(project);
        CREATE INDEX IF NOT EXISTS idx_metrics_state_project ON metrics_state(project);",
    ))?;

    // ── Orchestrator ──────────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS orch_runs (
            id             TEXT PRIMARY KEY,
            project        TEXT NOT NULL DEFAULT '',
            status         TEXT NOT NULL DEFAULT 'running',
            agents_json    TEXT NOT NULL DEFAULT '[]',
            dep_graph_json TEXT NOT NULL DEFAULT '{}',
            created_at     TEXT NOT NULL,
            updated_at     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS orch_agents (
            id             TEXT PRIMARY KEY,
            run_id         TEXT NOT NULL REFERENCES orch_runs(id),
            role           TEXT NOT NULL DEFAULT '',
            task           TEXT NOT NULL DEFAULT '',
            satisfies_json TEXT NOT NULL DEFAULT '[]',
            status         TEXT NOT NULL DEFAULT 'pending',
            phase          TEXT NOT NULL DEFAULT '',
            progress       REAL NOT NULL DEFAULT 0.0,
            last_heartbeat TEXT NOT NULL DEFAULT '',
            started_at     TEXT,
            completed_at   TEXT
        );
        CREATE TABLE IF NOT EXISTS orch_agent_events (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
            timestamp   TEXT NOT NULL,
            event_type  TEXT NOT NULL,
            data_json   TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS orch_agent_inbox (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
            from_agent  TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            message     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS orch_control (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            project    TEXT NOT NULL DEFAULT '',
            action     TEXT NOT NULL,
            target     TEXT,
            message    TEXT,
            generation INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_orch_events ON orch_agent_events(agent_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_orch_inbox  ON orch_agent_inbox(agent_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_orch_runs_project ON orch_runs(project);",
    ))?;

    // ── Orbit Pipelines ───────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS orbit_pipelines (
            id          TEXT PRIMARY KEY,
            project     TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'running',
            phase       TEXT,
            mode        TEXT,
            state_json  TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_orbit_status   ON orbit_pipelines(status);
        CREATE INDEX IF NOT EXISTS idx_orbit_project  ON orbit_pipelines(project);
        CREATE INDEX IF NOT EXISTS idx_orbit_created  ON orbit_pipelines(created_at DESC);",
    ))?;

    // ── Evolved Skills ────────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evolved_skills (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            origin      TEXT NOT NULL DEFAULT '',
            confidence  REAL NOT NULL DEFAULT 0.5,
            project     TEXT NOT NULL,
            skill_md    TEXT NOT NULL DEFAULT '',
            active      INTEGER NOT NULL DEFAULT 1,
            created     TEXT NOT NULL,
            updated     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS promotion_counters (
            pattern_key TEXT NOT NULL,
            project     TEXT NOT NULL DEFAULT '',
            count       INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (pattern_key, project)
        );
        CREATE TABLE IF NOT EXISTS workspace_manifest (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            project     TEXT NOT NULL DEFAULT '',
            version     TEXT NOT NULL DEFAULT '1.0',
            updated     TEXT NOT NULL,
            skills_json TEXT NOT NULL DEFAULT '[]'
        );",
    ))?;

    // ── Global Patterns ───────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS global_patterns (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp        TEXT NOT NULL,
            project          TEXT NOT NULL,
            success_rate     REAL NOT NULL DEFAULT 0.0,
            avg_score        REAL NOT NULL DEFAULT 0.0,
            per_error_stats  TEXT NOT NULL DEFAULT '{}',
            failure_patterns TEXT NOT NULL DEFAULT '[]',
            weak_tools       TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_global_ts      ON global_patterns(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_global_project  ON global_patterns(project);",
    ))?;

    // ── Additional indexes for query performance ─────────
    store_err(conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_orch_agents_run ON orch_agents(run_id);
         CREATE INDEX IF NOT EXISTS idx_obs_tool_ts     ON observations(tool, timestamp);
         CREATE INDEX IF NOT EXISTS idx_evolved_proj_act ON evolved_skills(project, active);",
    ))?;

    Ok(())
}

/// Write the current schema version to _harness_meta.
fn set_version(conn: &Connection, version: u32) -> io::Result<()> {
    store_err(conn.execute(
        "INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    ))?;
    Ok(())
}

/// Run schema migrations from `from_version` (exclusive) to `to_version` (inclusive).
///
/// Add migration blocks here when bumping `SCHEMA_VERSION`:
///
/// ```ignore
/// if to_version >= 2 && from_version < 2 {
///     conn.execute_batch("ALTER TABLE …")?;
/// }
/// ```
fn run_migrations(conn: &Connection, from_version: u32, to_version: u32) -> io::Result<()> {
    // v1→v2: Add missing FK indexes and performance indexes for existing DBs.
    // SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so FK references are
    // enforced via the index + application logic for pre-existing data.
    if to_version >= 2 && from_version < 2 {
        store_err(conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_orch_events_agent ON orch_agent_events(agent_id);
             CREATE INDEX IF NOT EXISTS idx_orch_inbox_agent  ON orch_agent_inbox(agent_id);
             CREATE INDEX IF NOT EXISTS idx_obs_tool_ts       ON observations(tool, timestamp);
             CREATE INDEX IF NOT EXISTS idx_evolved_proj_act  ON evolved_skills(project, active);",
        ))?;
    }

    // v2→v3: Add UNIQUE constraint on score_history.timestamp.
    // SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so we recreate the table.
    //
    // Concurrency: ImmediateTx acquires a write lock before the version re-check,
    // closing the TOCTOU window where two processes could both read version=1 in
    // init_schema() and both enter run_migrations(1, 3). The re-read inside the lock
    // ensures the second process skips the migration once the first has committed.
    //
    // Crash recovery: if the process dies after ImmediateTx::begin but before commit(),
    // the RAII guard issues ROLLBACK automatically. If it dies after commit(), the
    // schema_version=3 row is durable and the re-check skips the migration on restart.
    if to_version >= 3 && from_version < 3 {
        let tx = super::ImmediateTx::begin(conn)?;
        // Re-read version inside the lock to guard against concurrent migration.
        let current_v: Option<u32> = conn
            .query_row(
                "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());
        if current_v.map(|v| v >= 3).unwrap_or(false) {
            // Already migrated by a concurrent process — drop tx (auto-ROLLBACK).
            // Fall through so set_version() at the bottom of run_migrations is reached.
            // Skip v3→v4 check below since DB is already at v3+.
            set_version(conn, to_version)?;
            return Ok(());
        } else {
            store_err(conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS score_history_v3 (
                     id           INTEGER PRIMARY KEY AUTOINCREMENT,
                     timestamp    TEXT NOT NULL UNIQUE,
                     success_rate REAL NOT NULL,
                     avg_score    REAL NOT NULL,
                     observations INTEGER NOT NULL DEFAULT 0,
                     dim_success  REAL NOT NULL DEFAULT 0.0,
                     dim_quality  REAL NOT NULL DEFAULT 0.0,
                     dim_cost     REAL NOT NULL DEFAULT 0.0
                 );
                 INSERT OR IGNORE INTO score_history_v3
                     SELECT id, timestamp, success_rate, avg_score, observations,
                            dim_success, dim_quality, dim_cost FROM score_history;
                 DROP TABLE score_history;
                 ALTER TABLE score_history_v3 RENAME TO score_history;",
            ))?;
            tx.commit()?;
        }
        // Fall through — do NOT return early, so subsequent migrations (v3→v4, etc.)
        // can run in the same init_schema() call.
    }

    // v3→v4: Add project column to all tables for global DB.
    if to_version >= 4 && from_version < 4 {
        migrate_v3_to_v4(conn)?;
        // Fall through — do NOT return early, so set_version() is always reached.
    }

    set_version(conn, to_version)?;
    Ok(())
}

/// v3→v4: Add `project` column to all tables for global DB support.
///
/// Tables that already have `project` (orbit_pipelines, evolved_skills,
/// global_patterns) are skipped. Tables with single-row constraints
/// (promotion_counters, skill_attribution, workspace_manifest) are recreated
/// with composite primary keys. All others get a simple ALTER TABLE ADD COLUMN.
fn migrate_v3_to_v4(conn: &Connection) -> io::Result<()> {
    let tx = super::ImmediateTx::begin(conn)?;
    // Re-read version inside the lock.
    let current_v: Option<u32> = conn
        .query_row(
            "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok());
    if current_v.map(|v| v >= 4).unwrap_or(false) {
        return Ok(());
    }

    // Add project column to tables that exist but don't already have it.
    // Tables that don't exist yet will be created by apply_ddl() after migrations.
    let tables_needing_project = [
        "observations",
        "sessions",
        "evolution_records",
        "metrics_state",
        "score_history",
        "orch_runs",
        "orch_control",
    ];
    for table in &tables_needing_project {
        if table_exists(conn, table) && !has_column(conn, table, "project") {
            store_err(conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN project TEXT NOT NULL DEFAULT ''"),
                [],
            ))?;
        }
    }

    // Tables that need PK or UNIQUE constraint changes — recreate with data.
    // Only migrate if the original table exists; otherwise apply_ddl() will create
    // the correct v4 schema from scratch after this migration.

    // metrics_state: TEXT PK → (key, project) composite PK
    if table_exists(conn, "metrics_state") {
        store_err(conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS metrics_state_v4 (
                 key     TEXT NOT NULL,
                 value   TEXT NOT NULL,
                 project TEXT NOT NULL DEFAULT '',
                 PRIMARY KEY (key, project)
             );
             INSERT OR IGNORE INTO metrics_state_v4
                 SELECT key, value, project FROM metrics_state;
             DROP TABLE metrics_state;
             ALTER TABLE metrics_state_v4 RENAME TO metrics_state;",
        ))?;
    }

    // score_history: UNIQUE(timestamp) → UNIQUE(timestamp, project)
    if table_exists(conn, "score_history") {
        store_err(conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS score_history_v4 (
                 id           INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp    TEXT NOT NULL,
                 success_rate REAL NOT NULL,
                 avg_score    REAL NOT NULL,
                 observations INTEGER NOT NULL DEFAULT 0,
                 dim_success  REAL NOT NULL DEFAULT 0.0,
                 dim_quality  REAL NOT NULL DEFAULT 0.0,
                 dim_cost     REAL NOT NULL DEFAULT 0.0,
                 project      TEXT NOT NULL DEFAULT '',
                 UNIQUE(timestamp, project)
             );
             INSERT OR IGNORE INTO score_history_v4
                 SELECT id, timestamp, success_rate, avg_score, observations,
                        dim_success, dim_quality, dim_cost, project FROM score_history;
             DROP TABLE score_history;
             ALTER TABLE score_history_v4 RENAME TO score_history;",
        ))?;
    }

    if table_exists(conn, "skill_attribution") {
        // skill_attribution: TEXT PK → (skill_name, project) composite PK
        store_err(conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS skill_attribution_v4 (
                 skill_name        TEXT NOT NULL,
                 project           TEXT NOT NULL DEFAULT '',
                 sessions_active   INTEGER NOT NULL DEFAULT 0,
                 avg_score_with    REAL NOT NULL DEFAULT 0.0,
                 avg_score_without REAL NOT NULL DEFAULT 0.0,
                 first_seen        TEXT NOT NULL,
                 PRIMARY KEY (skill_name, project)
             );
             INSERT OR IGNORE INTO skill_attribution_v4
                 SELECT skill_name, '', sessions_active, avg_score_with,
                        avg_score_without, first_seen FROM skill_attribution;
             DROP TABLE skill_attribution;
             ALTER TABLE skill_attribution_v4 RENAME TO skill_attribution;",
        ))?;
    }

    if table_exists(conn, "promotion_counters") {
        // promotion_counters: TEXT PK → (pattern_key, project) composite PK
        store_err(conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS promotion_counters_v4 (
                 pattern_key TEXT NOT NULL,
                 project     TEXT NOT NULL DEFAULT '',
                 count       INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (pattern_key, project)
             );
             INSERT OR IGNORE INTO promotion_counters_v4
                 SELECT pattern_key, '', count FROM promotion_counters;
             DROP TABLE promotion_counters;
             ALTER TABLE promotion_counters_v4 RENAME TO promotion_counters;",
        ))?;
    }

    if table_exists(conn, "workspace_manifest") {
        // workspace_manifest: id=1 CHECK → AUTOINCREMENT + project column
        store_err(conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspace_manifest_v4 (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 project     TEXT NOT NULL DEFAULT '',
                 version     TEXT NOT NULL DEFAULT '1.0',
                 updated     TEXT NOT NULL,
                 skills_json TEXT NOT NULL DEFAULT '[]'
             );
             INSERT OR IGNORE INTO workspace_manifest_v4
                 SELECT id, '', version, updated, skills_json FROM workspace_manifest;
             DROP TABLE workspace_manifest;
             ALTER TABLE workspace_manifest_v4 RENAME TO workspace_manifest;",
        ))?;
    }

    // Add project indexes for tables that have the project column.
    // Tables not yet created will get their indexes from apply_ddl().
    let project_indexes = [
        ("idx_obs_project", "observations"),
        ("idx_sessions_project", "sessions"),
        ("idx_evo_project", "evolution_records"),
        ("idx_score_hist_project", "score_history"),
        ("idx_metrics_state_project", "metrics_state"),
        ("idx_orch_runs_project", "orch_runs"),
    ];
    for (idx, tbl) in &project_indexes {
        if has_column(conn, tbl, "project") {
            store_err(conn.execute(
                &format!("CREATE INDEX IF NOT EXISTS {idx} ON {tbl}(project)"),
                [],
            ))?;
        }
    }

    tx.commit()?;
    Ok(())
}

/// Check whether a table exists in the database.
fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table],
        |_| Ok(true),
    )
    .unwrap_or(false)
}

/// Check whether a table has a specific column.
fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({table})")) else {
        return false;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    rows.filter_map(|r| r.ok()).any(|name| name == column)
}

// ── Async (sqlx) schema initialization ────────────

/// Apply the full operational schema to a `SqlitePool`.
///
/// Async equivalent of `init_schema()` for use with sqlx pools.
/// Called once during pool creation in `pool::harness_pool()`.
///
/// **Keep in sync with `apply_ddl`** above — any DDL change here must be
/// mirrored in the rusqlite counterpart.
pub(crate) async fn init_schema_pool(pool: &SqlitePool) -> io::Result<()> {
    use sqlx::Executor;

    // PRAGMA
    pool.execute(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         PRAGMA wal_autocheckpoint=100;
         PRAGMA busy_timeout=5000;",
    )
    .await
    .map_err(super::sqlx_err)?;

    // DDL — same statements as apply_ddl(), using sqlx::raw_sql for multi-statement batches.
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _harness_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS observations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp        TEXT NOT NULL,
            session_id       TEXT NOT NULL,
            tool             TEXT NOT NULL,
            tool_category    TEXT NOT NULL,
            action           TEXT,
            result           TEXT,
            score            REAL,
            dim_success      REAL,
            dim_quality      REAL,
            dim_cost         REAL,
            failure_category TEXT,
            error_snippet    TEXT,
            file_ext         TEXT,
            sequence_id      INTEGER,
            pipeline_id      TEXT,
            project          TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
        CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
        CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
        CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_obs_project    ON observations(project);

        CREATE TABLE IF NOT EXISTS sessions (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp         TEXT NOT NULL,
            snap_type         TEXT NOT NULL DEFAULT 'pre-compact',
            summary           TEXT NOT NULL DEFAULT '',
            pending_tasks     TEXT NOT NULL DEFAULT '[]',
            context_usage     REAL,
            pipeline_state    TEXT,
            created_at_millis INTEGER NOT NULL,
            project           TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_ts ON sessions(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);

        CREATE TABLE IF NOT EXISTS evolution_records (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp         TEXT NOT NULL,
            observations      INTEGER NOT NULL DEFAULT 0,
            success_rate      REAL NOT NULL DEFAULT 0.0,
            avg_score         REAL NOT NULL DEFAULT 0.0,
            error_patterns    TEXT NOT NULL DEFAULT '{}',
            failure_patterns  TEXT NOT NULL DEFAULT '[]',
            skills_seeded     INTEGER NOT NULL DEFAULT 0,
            skills_rolled_back INTEGER NOT NULL DEFAULT 0,
            total_evolved     INTEGER NOT NULL DEFAULT 0,
            analysis_summary  TEXT NOT NULL DEFAULT '',
            project           TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_evo_project ON evolution_records(project);

        CREATE TABLE IF NOT EXISTS metrics_state (
            key     TEXT NOT NULL,
            value   TEXT NOT NULL,
            project TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (key, project)
        );
        CREATE TABLE IF NOT EXISTS score_history (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp    TEXT NOT NULL,
            success_rate REAL NOT NULL,
            avg_score    REAL NOT NULL,
            observations INTEGER NOT NULL DEFAULT 0,
            dim_success  REAL NOT NULL DEFAULT 0.0,
            dim_quality  REAL NOT NULL DEFAULT 0.0,
            dim_cost     REAL NOT NULL DEFAULT 0.0,
            project      TEXT NOT NULL DEFAULT '',
            UNIQUE(timestamp, project)
        );
        CREATE TABLE IF NOT EXISTS skill_attribution (
            skill_name        TEXT NOT NULL,
            project           TEXT NOT NULL DEFAULT '',
            sessions_active   INTEGER NOT NULL DEFAULT 0,
            avg_score_with    REAL NOT NULL DEFAULT 0.0,
            avg_score_without REAL NOT NULL DEFAULT 0.0,
            first_seen        TEXT NOT NULL,
            PRIMARY KEY (skill_name, project)
        );
        CREATE INDEX IF NOT EXISTS idx_score_hist_project ON score_history(project);
        CREATE INDEX IF NOT EXISTS idx_metrics_state_project ON metrics_state(project);

        CREATE TABLE IF NOT EXISTS orch_runs (
            id             TEXT PRIMARY KEY,
            project        TEXT NOT NULL DEFAULT '',
            status         TEXT NOT NULL DEFAULT 'running',
            agents_json    TEXT NOT NULL DEFAULT '[]',
            dep_graph_json TEXT NOT NULL DEFAULT '{}',
            created_at     TEXT NOT NULL,
            updated_at     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS orch_agents (
            id             TEXT PRIMARY KEY,
            run_id         TEXT NOT NULL REFERENCES orch_runs(id),
            role           TEXT NOT NULL DEFAULT '',
            task           TEXT NOT NULL DEFAULT '',
            satisfies_json TEXT NOT NULL DEFAULT '[]',
            status         TEXT NOT NULL DEFAULT 'pending',
            phase          TEXT NOT NULL DEFAULT '',
            progress       REAL NOT NULL DEFAULT 0.0,
            last_heartbeat TEXT NOT NULL DEFAULT '',
            started_at     TEXT,
            completed_at   TEXT
        );
        CREATE TABLE IF NOT EXISTS orch_agent_events (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
            timestamp   TEXT NOT NULL,
            event_type  TEXT NOT NULL,
            data_json   TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS orch_agent_inbox (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
            from_agent  TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            message     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS orch_control (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            project    TEXT NOT NULL DEFAULT '',
            action     TEXT NOT NULL,
            target     TEXT,
            message    TEXT,
            generation INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_orch_events ON orch_agent_events(agent_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_orch_inbox  ON orch_agent_inbox(agent_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_orch_runs_project ON orch_runs(project);

        CREATE TABLE IF NOT EXISTS orbit_pipelines (
            id          TEXT PRIMARY KEY,
            project     TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'running',
            phase       TEXT,
            mode        TEXT,
            state_json  TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_orbit_status   ON orbit_pipelines(status);
        CREATE INDEX IF NOT EXISTS idx_orbit_project  ON orbit_pipelines(project);
        CREATE INDEX IF NOT EXISTS idx_orbit_created  ON orbit_pipelines(created_at DESC);

        CREATE TABLE IF NOT EXISTS evolved_skills (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            origin      TEXT NOT NULL DEFAULT '',
            confidence  REAL NOT NULL DEFAULT 0.5,
            project     TEXT NOT NULL,
            skill_md    TEXT NOT NULL DEFAULT '',
            active      INTEGER NOT NULL DEFAULT 1,
            created     TEXT NOT NULL,
            updated     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS promotion_counters (
            pattern_key TEXT NOT NULL,
            project     TEXT NOT NULL DEFAULT '',
            count       INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (pattern_key, project)
        );
        CREATE TABLE IF NOT EXISTS workspace_manifest (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            project     TEXT NOT NULL DEFAULT '',
            version     TEXT NOT NULL DEFAULT '1.0',
            updated     TEXT NOT NULL,
            skills_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS global_patterns (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp        TEXT NOT NULL,
            project          TEXT NOT NULL,
            success_rate     REAL NOT NULL DEFAULT 0.0,
            avg_score        REAL NOT NULL DEFAULT 0.0,
            per_error_stats  TEXT NOT NULL DEFAULT '{}',
            failure_patterns TEXT NOT NULL DEFAULT '[]',
            weak_tools       TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_global_ts      ON global_patterns(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_global_project  ON global_patterns(project);

        CREATE INDEX IF NOT EXISTS idx_orch_agents_run ON orch_agents(run_id);
        CREATE INDEX IF NOT EXISTS idx_obs_tool_ts     ON observations(tool, timestamp);
        CREATE INDEX IF NOT EXISTS idx_evolved_proj_act ON evolved_skills(project, active);",
    )
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    // Run pending migrations before setting version.
    run_migrations_pool(pool).await?;

    // Set schema version.
    set_version_pool(pool, SCHEMA_VERSION).await?;

    Ok(())
}

/// Check schema version in the pool and run pending migrations.
/// Async equivalent of the rusqlite-based `run_migrations`.
async fn run_migrations_pool(pool: &SqlitePool) -> io::Result<()> {
    // Read current version — None means fresh DB (no rows yet).
    let current: u32 = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
    )
    .fetch_optional(pool)
    .await
    .map_err(super::sqlx_err)?
    .and_then(|(v,)| v.parse().ok())
    .unwrap_or(SCHEMA_VERSION); // Fresh DB: DDL already created v4 schema.

    if current < 4 {
        migrate_v3_to_v4_pool(pool).await?;
    }

    Ok(())
}

/// Write the current schema version to _harness_meta (sqlx variant).
async fn set_version_pool(pool: &SqlitePool, version: u32) -> io::Result<()> {
    sqlx::query("INSERT OR REPLACE INTO _harness_meta (key, value) VALUES ('schema_version', ?1)")
        .bind(version.to_string())
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(())
}

/// Async v3->v4 migration: add project column to tables that don't have it.
async fn migrate_v3_to_v4_pool(pool: &SqlitePool) -> io::Result<()> {
    // Check if already migrated (project column exists in observations).
    let cols = sqlx::query("PRAGMA table_info(observations)")
        .fetch_all(pool)
        .await
        .map_err(super::sqlx_err)?;

    let has_project = cols.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|n| n == "project")
            .unwrap_or(false)
    });

    if has_project {
        return Ok(());
    }

    // Add project column to tables that need it.
    let tables = [
        "observations",
        "sessions",
        "evolution_records",
        "metrics_state",
        "score_history",
        "orch_runs",
        "orch_control",
    ];

    for table in &tables {
        let cols = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(pool)
            .await
            .map_err(super::sqlx_err)?;

        let has_col = cols.iter().any(|r| {
            r.try_get::<String, _>("name")
                .map(|n| n == "project")
                .unwrap_or(false)
        });

        if !has_col && !cols.is_empty() {
            sqlx::query(&format!(
                "ALTER TABLE {table} ADD COLUMN project TEXT NOT NULL DEFAULT ''"
            ))
            .execute(pool)
            .await
            .map_err(super::sqlx_err)?;
        }
    }

    // Composite-PK table recreations — same logic as the rusqlite path in
    // migrate_v3_to_v4(). Tables that need PK changes are recreated with the
    // correct v4 composite primary keys.
    let recreations: &[(&str, &str)] = &[
        // (original_table, create_v4 + copy + drop + rename)
        (
            "metrics_state",
            "CREATE TABLE IF NOT EXISTS metrics_state_v4 (\
                 key TEXT NOT NULL, value TEXT NOT NULL, \
                 project TEXT NOT NULL DEFAULT '', \
                 PRIMARY KEY (key, project));\
             INSERT OR IGNORE INTO metrics_state_v4 \
                 SELECT key, value, project FROM metrics_state;\
             DROP TABLE metrics_state;\
             ALTER TABLE metrics_state_v4 RENAME TO metrics_state;",
        ),
        (
            "score_history",
            "CREATE TABLE IF NOT EXISTS score_history_v4 (\
                 id INTEGER PRIMARY KEY AUTOINCREMENT, \
                 timestamp TEXT NOT NULL, success_rate REAL NOT NULL, \
                 avg_score REAL NOT NULL, observations INTEGER NOT NULL DEFAULT 0, \
                 dim_success REAL NOT NULL DEFAULT 0.0, \
                 dim_quality REAL NOT NULL DEFAULT 0.0, \
                 dim_cost REAL NOT NULL DEFAULT 0.0, \
                 project TEXT NOT NULL DEFAULT '', \
                 UNIQUE(timestamp, project));\
             INSERT OR IGNORE INTO score_history_v4 \
                 SELECT id, timestamp, success_rate, avg_score, observations, \
                        dim_success, dim_quality, dim_cost, project FROM score_history;\
             DROP TABLE score_history;\
             ALTER TABLE score_history_v4 RENAME TO score_history;",
        ),
        (
            "skill_attribution",
            "CREATE TABLE IF NOT EXISTS skill_attribution_v4 (\
                 skill_name TEXT NOT NULL, project TEXT NOT NULL DEFAULT '', \
                 sessions_active INTEGER NOT NULL DEFAULT 0, \
                 avg_score_with REAL NOT NULL DEFAULT 0.0, \
                 avg_score_without REAL NOT NULL DEFAULT 0.0, \
                 first_seen TEXT NOT NULL, \
                 PRIMARY KEY (skill_name, project));\
             INSERT OR IGNORE INTO skill_attribution_v4 \
                 SELECT skill_name, '', sessions_active, avg_score_with, \
                        avg_score_without, first_seen FROM skill_attribution;\
             DROP TABLE skill_attribution;\
             ALTER TABLE skill_attribution_v4 RENAME TO skill_attribution;",
        ),
        (
            "promotion_counters",
            "CREATE TABLE IF NOT EXISTS promotion_counters_v4 (\
                 pattern_key TEXT NOT NULL, project TEXT NOT NULL DEFAULT '', \
                 count INTEGER NOT NULL DEFAULT 0, \
                 PRIMARY KEY (pattern_key, project));\
             INSERT OR IGNORE INTO promotion_counters_v4 \
                 SELECT pattern_key, '', count FROM promotion_counters;\
             DROP TABLE promotion_counters;\
             ALTER TABLE promotion_counters_v4 RENAME TO promotion_counters;",
        ),
        (
            "workspace_manifest",
            "CREATE TABLE IF NOT EXISTS workspace_manifest_v4 (\
                 id INTEGER PRIMARY KEY AUTOINCREMENT, \
                 project TEXT NOT NULL DEFAULT '', \
                 version TEXT NOT NULL DEFAULT '1.0', \
                 updated TEXT NOT NULL, \
                 skills_json TEXT NOT NULL DEFAULT '[]');\
             INSERT OR IGNORE INTO workspace_manifest_v4 \
                 SELECT id, '', version, updated, skills_json FROM workspace_manifest;\
             DROP TABLE workspace_manifest;\
             ALTER TABLE workspace_manifest_v4 RENAME TO workspace_manifest;",
        ),
    ];

    for (table, sql) in recreations {
        // Only recreate if the table exists and still has a single-column PK
        // (i.e., hasn't been migrated yet).
        if table_has_single_pk(pool, table).await? {
            sqlx::raw_sql(sql)
                .execute(pool)
                .await
                .map_err(super::sqlx_err)?;
        }
    }

    Ok(())
}

/// Check if a table exists and has a single-column primary key (not yet migrated to composite).
async fn table_has_single_pk(pool: &SqlitePool, table: &str) -> io::Result<bool> {
    // PRAGMA table_info returns one row per column; pk > 0 marks PK columns.
    let cols = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .map_err(super::sqlx_err)?;
    if cols.is_empty() {
        return Ok(false); // table doesn't exist
    }
    let pk_count = cols
        .iter()
        .filter(|r| r.try_get::<i32, _>("pk").unwrap_or(0) > 0)
        .count();
    Ok(pk_count == 1)
}
