//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, evolved skills, and global patterns.

use sqlx::AnyPool;
use std::io;

/// Apply the full operational schema to a pool.
/// Safe to call multiple times (uses `IF NOT EXISTS`).
pub async fn init_schema_pool(pool: &AnyPool) -> io::Result<()> {
    // PRAGMAs are set in pool.rs build_sqlite_pool() — no need to set here.

    for ddl in DDL_SQLITE.split(';') {
        let trimmed = ddl.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(pool)
            .await
            .map_err(super::sqlx_err)?;
    }

    Ok(())
}

/// Full DDL as a single string. Split on `;` and execute each statement.
pub(crate) const DDL_SQLITE: &str = r#"
    CREATE TABLE IF NOT EXISTS _harness_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    INSERT OR IGNORE INTO _harness_meta (key, value) VALUES ('schema_version', '1');

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
        pipeline_id      TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
    CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
    CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);

    CREATE TABLE IF NOT EXISTS sessions (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp         TEXT NOT NULL,
        snap_type         TEXT NOT NULL DEFAULT 'pre-compact',
        summary           TEXT NOT NULL DEFAULT '',
        pending_tasks     TEXT NOT NULL DEFAULT '[]',
        context_usage     REAL,
        pipeline_state    TEXT,
        created_at_millis INTEGER NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_ts ON sessions(timestamp DESC);

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
        analysis_summary  TEXT NOT NULL DEFAULT ''
    );

    CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);

    CREATE TABLE IF NOT EXISTS metrics_state (
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
    );

    CREATE TABLE IF NOT EXISTS orch_runs (
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
        agent_id    TEXT NOT NULL,
        timestamp   TEXT NOT NULL,
        event_type  TEXT NOT NULL,
        data_json   TEXT NOT NULL DEFAULT '{}'
    );

    CREATE TABLE IF NOT EXISTS orch_agent_inbox (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        agent_id    TEXT NOT NULL,
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
    CREATE INDEX IF NOT EXISTS idx_orch_inbox  ON orch_agent_inbox(agent_id, timestamp);

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

    CREATE INDEX IF NOT EXISTS idx_orbit_status  ON orbit_pipelines(status);
    CREATE INDEX IF NOT EXISTS idx_orbit_project ON orbit_pipelines(project);

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
        pattern_key TEXT PRIMARY KEY,
        count       INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS workspace_manifest (
        id          INTEGER PRIMARY KEY CHECK (id = 1),
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
"#;
