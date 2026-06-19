//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, evolved skills, and global patterns.

use sqlx::AnyPool;
use std::io;

/// Current schema version. Bump when DDL changes.
#[cfg(test)]
pub(crate) const SCHEMA_VERSION: u32 = 1;

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

    // Idempotent column migrations for pre-existing databases.
    // SQLite has no ADD COLUMN IF NOT EXISTS, so guard with pragma_table_info.
    ensure_column(
        pool,
        "evolution_records",
        "edit_type",
        "TEXT NOT NULL DEFAULT 'add_skill'",
    )
    .await?;

    // Rebuild metrics_state PK (key) -> (key, project) for legacy databases.
    // New databases already get the composite PK from DDL_SQLITE above.
    migrate_metrics_state_pk(pool).await?;
    migrate_skill_attribution_pk(pool).await?;

    Ok(())
}

/// Add a column to a table if it does not already exist.
///
/// SQLite lacks `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`, so this inspects
/// `pragma_table_info` and only issues the `ALTER TABLE` when the column is
/// absent. Safe to call on every schema init.
///
/// `table`, `column`, and `type_clause` are always compile-time string
/// literals passed by the caller (never user input), so the interpolated DDL
/// is wrapped in `AssertSqlSafe` per the codebase convention for identifier DDL.
async fn ensure_column(
    pool: &AnyPool,
    table: &str,
    column: &str,
    type_clause: &str,
) -> io::Result<()> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?")
            .bind(table)
            .bind(column)
            .fetch_one(pool)
            .await
            .map_err(super::sqlx_err)?;

    if exists == 0 {
        let ddl = format!("ALTER TABLE {table} ADD COLUMN {column} {type_clause}");
        sqlx::query(sqlx::AssertSqlSafe(ddl))
            .execute(pool)
            .await
            .map_err(super::sqlx_err)?;
    }
    Ok(())
}

/// Migrate `metrics_state` primary key from `(key)` to `(key, project)`.
///
/// SQLite cannot ALTER an existing table's primary key, so for legacy
/// databases we rebuild the table (create-copy-drop-rename) inside a
/// transaction. New databases already get the composite PK from `DDL_SQLITE`.
/// Idempotent: guarded by `_harness_meta.metrics_pk_v2` and a
/// `pragma_table_info` PK-column count check.
/// Migrate `skill_attribution` PK `(skill_name)` → `(skill_name, project)`.
///
/// Legacy rows are attributed to `project=''` (single-project assumption —
/// the writer had no project binding before this change). A multi-project
/// legacy DB would collapse pre-existing rows to one `(skill_name, '')`, which
/// is acceptable since pre-#92 data was CWD-only.
async fn migrate_skill_attribution_pk(pool: &AnyPool) -> io::Result<()> {
    let done: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM _harness_meta WHERE key = 'skill_attribution_pk_v2' AND value = '1'",
    )
    .fetch_one(pool)
    .await
    .map_err(super::sqlx_err)?;
    if done > 0 {
        return Ok(());
    }

    let pk_cols: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('skill_attribution') WHERE pk > 0",
    )
    .fetch_one(pool)
    .await
    .map_err(super::sqlx_err)?;

    if pk_cols >= 2 {
        sqlx::query(
            "INSERT INTO _harness_meta (key, value) VALUES ('skill_attribution_pk_v2', '1') \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
        return Ok(());
    }

    // Legacy single-PK table: rebuild to composite PK in one transaction.
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    sqlx::query(
        "CREATE TABLE skill_attribution_new (\
            skill_name TEXT NOT NULL, sessions_active INTEGER NOT NULL DEFAULT 0, \
            avg_score_with REAL NOT NULL DEFAULT 0.0, avg_score_without REAL NOT NULL DEFAULT 0.0, \
            first_seen TEXT NOT NULL, project TEXT NOT NULL DEFAULT '', \
            PRIMARY KEY (skill_name, project))",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    sqlx::query(
        "INSERT INTO skill_attribution_new \
            (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, project) \
         SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, '' \
         FROM skill_attribution \
         ON CONFLICT (skill_name, project) DO UPDATE SET \
            sessions_active = excluded.sessions_active, \
            avg_score_with = excluded.avg_score_with, \
            avg_score_without = excluded.avg_score_without, \
            first_seen = excluded.first_seen",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    sqlx::query("DROP TABLE skill_attribution")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("ALTER TABLE skill_attribution_new RENAME TO skill_attribution")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    sqlx::query(
        "INSERT INTO _harness_meta (key, value) VALUES ('skill_attribution_pk_v2', '1') \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(())
}

async fn migrate_metrics_state_pk(pool: &AnyPool) -> io::Result<()> {
    let done: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM _harness_meta WHERE key = 'metrics_pk_v2' AND value = '1'",
    )
    .fetch_one(pool)
    .await
    .map_err(super::sqlx_err)?;

    if done > 0 {
        return Ok(());
    }

    // Number of columns participating in the PK: (key) = 1, (key, project) = 2.
    let pk_cols: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('metrics_state') WHERE pk > 0")
            .fetch_one(pool)
            .await
            .map_err(super::sqlx_err)?;

    if pk_cols >= 2 {
        // Already composite (new DB) — just stamp the guard.
        sqlx::query(
            "INSERT INTO _harness_meta (key, value) VALUES ('metrics_pk_v2', '1') \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
        return Ok(());
    }

    // Legacy single-PK table: rebuild to composite PK in one transaction.
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    sqlx::query(
        "CREATE TABLE metrics_state_new (\
            key TEXT NOT NULL, value TEXT NOT NULL, \
            project TEXT NOT NULL DEFAULT '', \
            PRIMARY KEY (key, project))",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    sqlx::query(
        "INSERT INTO metrics_state_new (key, value, project) \
         SELECT key, value, COALESCE(project, '') FROM metrics_state \
         ON CONFLICT (key, project) DO UPDATE SET value = excluded.value",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    sqlx::query("DROP TABLE metrics_state")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    sqlx::query("ALTER TABLE metrics_state_new RENAME TO metrics_state")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    sqlx::query(
        "INSERT INTO _harness_meta (key, value) VALUES ('metrics_pk_v2', '1') \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;

    tx.commit().await.map_err(super::sqlx_err)?;
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
        pipeline_id      TEXT,
        project          TEXT
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
        edit_type         TEXT NOT NULL DEFAULT 'add_skill',
        project           TEXT NOT NULL DEFAULT ''
    );

    CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);

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
        project      TEXT NOT NULL DEFAULT ''
    );

    CREATE TABLE IF NOT EXISTS skill_attribution (
        skill_name        TEXT NOT NULL,
        sessions_active   INTEGER NOT NULL DEFAULT 0,
        avg_score_with    REAL NOT NULL DEFAULT 0.0,
        avg_score_without REAL NOT NULL DEFAULT 0.0,
        first_seen        TEXT NOT NULL,
        project           TEXT NOT NULL DEFAULT '',
        PRIMARY KEY (skill_name, project)
    );

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
        project    TEXT NOT NULL DEFAULT '',
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
        pattern_key TEXT NOT NULL,
        project     TEXT NOT NULL DEFAULT '',
        count       INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (pattern_key, project)
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
