//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, evolved skills, and global patterns.

use rusqlite::Connection;
use std::io;

use super::store_err;

/// Current schema version. Increment when adding migrations in `run_migrations`.
const SCHEMA_VERSION: u32 = 2;

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
            pipeline_id      TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
        CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
        CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
        CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);",
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
            created_at_millis INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_ts ON sessions(timestamp DESC);",
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
            analysis_summary  TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);",
    ))?;

    // ── Metrics (3-table normalized) ──────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics_state (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS score_history (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp    TEXT NOT NULL,
            success_rate REAL NOT NULL,
            avg_score    REAL NOT NULL,
            observations INTEGER NOT NULL DEFAULT 0,
            dim_success  REAL NOT NULL DEFAULT 0.0,
            dim_quality  REAL NOT NULL DEFAULT 0.0,
            dim_cost     REAL NOT NULL DEFAULT 0.0
        );
        CREATE TABLE IF NOT EXISTS skill_attribution (
            skill_name        TEXT PRIMARY KEY,
            sessions_active   INTEGER NOT NULL DEFAULT 0,
            avg_score_with    REAL NOT NULL DEFAULT 0.0,
            avg_score_without REAL NOT NULL DEFAULT 0.0,
            first_seen        TEXT NOT NULL
        );",
    ))?;

    // ── Orchestrator ──────────────────────────────────
    store_err(conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS orch_runs (
            id             TEXT PRIMARY KEY,
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
            id         INTEGER PRIMARY KEY CHECK (id = 1),
            action     TEXT NOT NULL,
            target     TEXT,
            message    TEXT,
            generation INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_orch_events ON orch_agent_events(agent_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_orch_inbox  ON orch_agent_inbox(agent_id, timestamp);",
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
            pattern_key TEXT PRIMARY KEY,
            count       INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS workspace_manifest (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
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

    set_version(conn, to_version)?;
    Ok(())
}
