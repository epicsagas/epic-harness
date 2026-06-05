//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, evolved skills, and global patterns.
//! Supports SQLite (default) and PostgreSQL via dual-path DDL.

use sqlx::{AnyPool, Row};
use std::io;

use super::pool::DbType;

/// Current schema version. Increment when adding migrations in `run_migrations_pool`.
pub(crate) const SCHEMA_VERSION: u32 = 4;

/// ── SQLite DDL ──────────────────────────────────────
pub(crate) const DDL_SQLITE: &str = "
    CREATE TABLE IF NOT EXISTS _harness_meta (
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
    CREATE INDEX IF NOT EXISTS idx_evolved_proj_act ON evolved_skills(project, active);
";

/// ── PostgreSQL DDL ──────────────────────────────────
const DDL_POSTGRES: &str = "
    CREATE TABLE IF NOT EXISTS _harness_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS observations (
        id               BIGSERIAL PRIMARY KEY,
        timestamp        TEXT NOT NULL,
        session_id       TEXT NOT NULL,
        tool             TEXT NOT NULL,
        tool_category    TEXT NOT NULL,
        action           TEXT,
        result           TEXT,
        score            DOUBLE PRECISION,
        dim_success      DOUBLE PRECISION,
        dim_quality      DOUBLE PRECISION,
        dim_cost         DOUBLE PRECISION,
        failure_category TEXT,
        error_snippet    TEXT,
        file_ext         TEXT,
        sequence_id      BIGINT,
        pipeline_id      TEXT,
        project          TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
    CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
    CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_project    ON observations(project);

    CREATE TABLE IF NOT EXISTS sessions (
        id                BIGSERIAL PRIMARY KEY,
        timestamp         TEXT NOT NULL,
        snap_type         TEXT NOT NULL DEFAULT 'pre-compact',
        summary           TEXT NOT NULL DEFAULT '',
        pending_tasks     TEXT NOT NULL DEFAULT '[]',
        context_usage     DOUBLE PRECISION,
        pipeline_state    TEXT,
        created_at_millis BIGINT NOT NULL,
        project           TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX IF NOT EXISTS idx_sessions_ts ON sessions(timestamp DESC);
    CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);

    CREATE TABLE IF NOT EXISTS evolution_records (
        id                BIGSERIAL PRIMARY KEY,
        timestamp         TEXT NOT NULL,
        observations      INTEGER NOT NULL DEFAULT 0,
        success_rate      DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        avg_score         DOUBLE PRECISION NOT NULL DEFAULT 0.0,
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
        id           BIGSERIAL PRIMARY KEY,
        timestamp    TEXT NOT NULL,
        success_rate DOUBLE PRECISION NOT NULL,
        avg_score    DOUBLE PRECISION NOT NULL,
        observations INTEGER NOT NULL DEFAULT 0,
        dim_success  DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        dim_quality  DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        dim_cost     DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        project      TEXT NOT NULL DEFAULT '',
        UNIQUE(timestamp, project)
    );
    CREATE TABLE IF NOT EXISTS skill_attribution (
        skill_name        TEXT NOT NULL,
        project           TEXT NOT NULL DEFAULT '',
        sessions_active   INTEGER NOT NULL DEFAULT 0,
        avg_score_with    DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        avg_score_without DOUBLE PRECISION NOT NULL DEFAULT 0.0,
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
        progress       DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        last_heartbeat TEXT NOT NULL DEFAULT '',
        started_at     TEXT,
        completed_at   TEXT
    );
    CREATE TABLE IF NOT EXISTS orch_agent_events (
        id          BIGSERIAL PRIMARY KEY,
        agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
        timestamp   TEXT NOT NULL,
        event_type  TEXT NOT NULL,
        data_json   TEXT NOT NULL DEFAULT '{}'
    );
    CREATE TABLE IF NOT EXISTS orch_agent_inbox (
        id          BIGSERIAL PRIMARY KEY,
        agent_id    TEXT NOT NULL REFERENCES orch_agents(id),
        from_agent  TEXT NOT NULL,
        timestamp   TEXT NOT NULL,
        message     TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS orch_control (
        id         BIGSERIAL PRIMARY KEY,
        project    TEXT NOT NULL DEFAULT '',
        action     TEXT,
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
        id          BIGSERIAL PRIMARY KEY,
        name        TEXT NOT NULL UNIQUE,
        origin      TEXT NOT NULL DEFAULT '',
        confidence  DOUBLE PRECISION NOT NULL DEFAULT 0.5,
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
        id          BIGSERIAL PRIMARY KEY,
        project     TEXT NOT NULL DEFAULT '',
        version     TEXT NOT NULL DEFAULT '1.0',
        updated     TEXT NOT NULL,
        skills_json TEXT NOT NULL DEFAULT '[]'
    );

    CREATE TABLE IF NOT EXISTS global_patterns (
        id               BIGSERIAL PRIMARY KEY,
        timestamp        TEXT NOT NULL,
        project          TEXT NOT NULL,
        success_rate     DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        avg_score        DOUBLE PRECISION NOT NULL DEFAULT 0.0,
        per_error_stats  TEXT NOT NULL DEFAULT '{}',
        failure_patterns TEXT NOT NULL DEFAULT '[]',
        weak_tools       TEXT NOT NULL DEFAULT '[]'
    );
    CREATE INDEX IF NOT EXISTS idx_global_ts      ON global_patterns(timestamp DESC);
    CREATE INDEX IF NOT EXISTS idx_global_project  ON global_patterns(project);

    CREATE INDEX IF NOT EXISTS idx_orch_agents_run ON orch_agents(run_id);
    CREATE INDEX IF NOT EXISTS idx_obs_tool_ts     ON observations(tool, timestamp);
    CREATE INDEX IF NOT EXISTS idx_evolved_proj_act ON evolved_skills(project, active);
";

// ── Schema initialization ─────────────────────────────

/// Apply the full operational schema to an `AnyPool`.
///
/// Dispatches to the appropriate DDL based on detected database type.
/// Called once during pool creation in `pool::harness_pool()`.
pub(crate) async fn init_schema_pool(pool: &AnyPool) -> io::Result<()> {
    use sqlx::Executor;

    let db_type = super::pool::harness_db_type();

    // SQLite-specific PRAGMAs
    if db_type == DbType::Sqlite {
        pool.execute(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA wal_autocheckpoint=100;
             PRAGMA busy_timeout=5000;",
        )
        .await
        .map_err(super::sqlx_err)?;
    }

    // DDL — select by backend
    let ddl = match db_type {
        DbType::Sqlite => DDL_SQLITE,
        DbType::Postgres => DDL_POSTGRES,
        DbType::Mysql => DDL_SQLITE, // MySQL deferred — feature flag only for now
    };
    sqlx::raw_sql(ddl)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;

    // Run pending migrations before setting version (SQLite only for now).
    if db_type == DbType::Sqlite {
        run_migrations_pool(pool).await?;
    }

    // Set schema version.
    set_version_pool(pool, SCHEMA_VERSION).await?;

    Ok(())
}

/// Check schema version in the pool and run pending migrations.
async fn run_migrations_pool(pool: &AnyPool) -> io::Result<()> {
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

/// Write the current schema version to _harness_meta.
async fn set_version_pool(pool: &AnyPool, version: u32) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO _harness_meta (key, value) VALUES ('schema_version', $1)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(version.to_string())
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Async v3->v4 migration: add project column to tables that don't have it.
async fn migrate_v3_to_v4_pool(pool: &AnyPool) -> io::Result<()> {
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

    // Composite-PK table recreations
    let recreations: &[(&str, &str)] = &[
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
async fn table_has_single_pk(pool: &AnyPool, table: &str) -> io::Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify DDL const works through the AnyPool path (init_schema_pool).
    #[tokio::test]
    async fn ddl_const_creates_all_tables_via_sqlx() {
        let pool = crate::store::pool::test_memory_pool().await;
        init_schema_pool(&pool).await.unwrap();

        let row = sqlx::query("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let count: i64 = row.try_get(0).unwrap();
        // 17 application tables + sqlite_sequence (auto-created by AUTOINCREMENT)
        assert_eq!(count, 18, "expected 18 tables via sqlx path, got {count}");

        // Verify version was set
        let v: String =
            sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(v, "4");
    }
}
