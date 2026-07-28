//! schema.rs — Harness operational database schema
//!
//! Creates all tables for observations, sessions, evolution, metrics,
//! orchestrator, orbit pipelines, and global patterns.

use sqlx::AnyPool;
use std::io;

/// Current schema version. Bump when DDL changes.
pub(crate) const SCHEMA_VERSION: u32 = 6;

async fn stored_schema_version(pool: &AnyPool) -> io::Result<Option<u32>> {
    match sqlx::query_scalar::<_, String>(
        "SELECT value FROM _harness_meta WHERE key = 'schema_version'",
    )
    .fetch_optional(pool)
    .await
    {
        Ok(version) => Ok(version.and_then(|value| value.parse().ok())),
        Err(error) => {
            let missing_meta = error.as_database_error().is_some_and(|database_error| {
                let message = database_error.message();
                message.contains("no such table: _harness_meta")
                    || (message.contains("_harness_meta") && message.contains("does not exist"))
            });
            if missing_meta {
                Ok(None)
            } else {
                Err(super::sqlx_err(error))
            }
        }
    }
}

async fn stamp_schema_version(pool: &AnyPool) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO _harness_meta (key, value) VALUES ('schema_version', ?) \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(SCHEMA_VERSION.to_string())
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Apply the full operational schema to a pool.
/// Safe to call multiple times (uses `IF NOT EXISTS`).
pub async fn init_schema_pool(pool: &AnyPool) -> io::Result<()> {
    if stored_schema_version(pool).await? == Some(SCHEMA_VERSION) {
        return Ok(());
    }

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
    ensure_column(pool, "evolution_records", "session_id", "TEXT").await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_evo_project_session
         ON evolution_records(project, session_id)",
    )
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    ensure_column(pool, "observations", "tool_use_id", "TEXT").await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_obs_tool_use_id ON observations(tool_use_id)")
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    ensure_column(
        pool,
        "global_patterns",
        "session_id",
        "TEXT NOT NULL DEFAULT ''",
    )
    .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_global_project_session
         ON global_patterns(project, session_id)
         WHERE session_id <> ''",
    )
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;

    // Rebuild metrics_state PK (key) -> (key, project) for legacy databases.
    // New databases already get the composite PK from DDL_SQLITE above.
    migrate_metrics_state_pk(pool).await?;
    migrate_skill_attribution_pk(pool).await?;
    migrate_orbit_pipeline_pk(pool).await?;

    // Holdout A/B counter (added with attribution holdout rotation). Runs
    // after the PK rebuild so legacy-rebuilt tables also gain the column.
    ensure_column(
        pool,
        "skill_attribution",
        "sessions_holdout",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;

    // A current marker proves that every DDL and migration above completed.
    // Never stamp earlier: a partial initialization must retry on the next open.
    stamp_schema_version(pool).await?;

    Ok(())
}

/// Migrate `orbit_pipelines` primary key from `(id)` to `(project, id)`.
///
/// SQLite cannot alter a primary key in place. Legacy tables are rebuilt in
/// one transaction, preserving every row. Primary-key inspection makes this
/// safe to retry and avoids a separate migration marker.
async fn migrate_orbit_pipeline_pk(pool: &AnyPool) -> io::Result<()> {
    let pk_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('orbit_pipelines')
         WHERE pk > 0 ORDER BY pk",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    if pk_columns == ["project", "id"] {
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;
    sqlx::query("DROP TABLE IF EXISTS orbit_pipelines_new")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query(
        "CREATE TABLE orbit_pipelines_new (
            id          TEXT NOT NULL,
            project     TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'running',
            phase       TEXT,
            mode        TEXT,
            state_json  TEXT NOT NULL DEFAULT '{}',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (project, id)
        )",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;
    sqlx::query(
        "INSERT INTO orbit_pipelines_new
            (id, project, status, phase, mode, state_json, created_at, updated_at)
         SELECT id, project, status, phase, mode, state_json, created_at, updated_at
         FROM orbit_pipelines",
    )
    .execute(&mut *tx)
    .await
    .map_err(super::sqlx_err)?;
    sqlx::query("DROP TABLE orbit_pipelines")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("ALTER TABLE orbit_pipelines_new RENAME TO orbit_pipelines")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("CREATE INDEX idx_orbit_status ON orbit_pipelines(status)")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("CREATE INDEX idx_orbit_project ON orbit_pipelines(project)")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;
    tx.commit().await.map_err(super::sqlx_err)?;
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
        "INSERT OR REPLACE INTO skill_attribution_new \
            (skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, project) \
         SELECT skill_name, sessions_active, avg_score_with, avg_score_without, first_seen, '' \
         FROM skill_attribution",
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
        "INSERT OR REPLACE INTO metrics_state_new (key, value, project) \
         SELECT key, value, COALESCE(project, '') FROM metrics_state",
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
        tool_use_id      TEXT,
        project          TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_obs_ts         ON observations(timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_session    ON observations(session_id);
    CREATE INDEX IF NOT EXISTS idx_obs_tool       ON observations(tool_category);
    CREATE INDEX IF NOT EXISTS idx_obs_sess_ts    ON observations(session_id, timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_proj_ts    ON observations(project, timestamp);
    CREATE INDEX IF NOT EXISTS idx_obs_sess_proj_id ON observations(session_id, project, id DESC);
    CREATE INDEX IF NOT EXISTS idx_obs_proj_ts_id ON observations(project, timestamp DESC, id DESC);
    CREATE INDEX IF NOT EXISTS idx_obs_ts_id      ON observations(timestamp DESC, id DESC);

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
        session_id        TEXT,
        project           TEXT NOT NULL DEFAULT ''
    );

    CREATE INDEX IF NOT EXISTS idx_evo_ts ON evolution_records(timestamp DESC);
    CREATE INDEX IF NOT EXISTS idx_evo_project_id ON evolution_records(project, id DESC);

    CREATE TABLE IF NOT EXISTS reflection_sessions (
        session_id   TEXT NOT NULL,
        project      TEXT NOT NULL,
        completed_at TEXT NOT NULL,
        PRIMARY KEY (session_id, project)
    );

    CREATE TABLE IF NOT EXISTS reflection_metrics (
        session_id TEXT NOT NULL,
        project    TEXT NOT NULL,
        PRIMARY KEY (session_id, project)
    );

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

    CREATE INDEX IF NOT EXISTS idx_score_history_project_id
        ON score_history(project, id DESC);

    CREATE TABLE IF NOT EXISTS skill_attribution (
        skill_name        TEXT NOT NULL,
        sessions_active   INTEGER NOT NULL DEFAULT 0,
        avg_score_with    REAL NOT NULL DEFAULT 0.0,
        avg_score_without REAL NOT NULL DEFAULT 0.0,
        first_seen        TEXT NOT NULL,
        project           TEXT NOT NULL DEFAULT '',
        sessions_holdout  INTEGER NOT NULL DEFAULT 0,
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
        id          TEXT NOT NULL,
        project     TEXT NOT NULL,
        status      TEXT NOT NULL DEFAULT 'running',
        phase       TEXT,
        mode        TEXT,
        state_json  TEXT NOT NULL DEFAULT '{}',
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        PRIMARY KEY (project, id)
    );

    CREATE INDEX IF NOT EXISTS idx_orbit_status  ON orbit_pipelines(status);
    CREATE INDEX IF NOT EXISTS idx_orbit_project ON orbit_pipelines(project);

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
        session_id        TEXT NOT NULL DEFAULT '',
        success_rate     REAL NOT NULL DEFAULT 0.0,
        avg_score        REAL NOT NULL DEFAULT 0.0,
        per_error_stats  TEXT NOT NULL DEFAULT '{}',
        failure_patterns TEXT NOT NULL DEFAULT '[]',
        weak_tools       TEXT NOT NULL DEFAULT '[]'
    );

    CREATE INDEX IF NOT EXISTS idx_global_ts      ON global_patterns(timestamp DESC);
    CREATE INDEX IF NOT EXISTS idx_global_project  ON global_patterns(project);
"#;

#[cfg(test)]
mod tests {
    use super::SCHEMA_VERSION;

    #[tokio::test]
    async fn current_schema_version_skips_full_initialization() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO _harness_meta (key, value) VALUES ('schema_version', ?)")
            .bind(SCHEMA_VERSION.to_string())
            .execute(&pool)
            .await
            .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let observations: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'observations'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(observations, 0, "current schema must skip full DDL");
    }

    #[tokio::test]
    async fn outdated_schema_version_runs_upgrade_and_stamps_current() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO _harness_meta (key, value) VALUES ('schema_version', '0')")
            .execute(&pool)
            .await
            .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let observations: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'observations'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let version: String =
            sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(observations, 1);
        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[tokio::test]
    async fn v3_schema_upgrades_to_durable_reflection_sessions() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO _harness_meta (key, value) VALUES ('schema_version', '3')")
            .execute(&pool)
            .await
            .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let reflection_sessions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'reflection_sessions'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(reflection_sessions, 1);
    }

    #[tokio::test]
    async fn failed_initialization_does_not_stamp_schema_version() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE VIEW observations AS SELECT 1 AS id")
            .execute(&pool)
            .await
            .unwrap();

        assert!(super::init_schema_pool(&pool).await.is_err());

        let version: Option<String> =
            sqlx::query_scalar("SELECT value FROM _harness_meta WHERE key = 'schema_version'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(version, None);
    }

    #[tokio::test]
    async fn reflection_sessions_are_unique_per_project_and_session() {
        let pool = super::super::pool::test_memory_pool().await;
        super::init_schema_pool(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO reflection_sessions (session_id, project, completed_at) VALUES (?, ?, ?)",
        )
        .bind("session-a")
        .bind("project-a")
        .bind("2026-07-28T10:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            sqlx::query(
                "INSERT INTO reflection_sessions (session_id, project, completed_at) VALUES (?, ?, ?)",
            )
            .bind("session-a")
            .bind("project-a")
            .bind("2026-07-28T10:01:00Z")
            .execute(&pool)
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn legacy_observations_gain_tool_identity_before_its_index() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE observations (
                id INTEGER PRIMARY KEY, timestamp TEXT NOT NULL, session_id TEXT NOT NULL,
                tool TEXT NOT NULL, tool_category TEXT NOT NULL, action TEXT, result TEXT,
                score REAL, dim_success REAL, dim_quality REAL, dim_cost REAL,
                failure_category TEXT, error_snippet TEXT, file_ext TEXT,
                sequence_id INTEGER, pipeline_id TEXT, project TEXT)",
        )
        .execute(&pool)
        .await
        .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('observations') WHERE name = 'tool_use_id'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let index: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_obs_tool_use_id'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(columns, 1);
        assert_eq!(index, 1);
    }

    #[tokio::test]
    async fn v5_orbit_identity_migrates_to_project_and_id_without_losing_rows() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO _harness_meta (key, value) VALUES ('schema_version', '5')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE orbit_pipelines (
                id TEXT PRIMARY KEY,
                project TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                phase TEXT,
                mode TEXT,
                state_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO orbit_pipelines
                (id, project, status, state_json, created_at, updated_at)
             VALUES ('PIPELINE-legacy', 'project-a', 'running', '{}', '1', '1')",
        )
        .execute(&pool)
        .await
        .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let pk_columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('orbit_pipelines')
             WHERE pk > 0 ORDER BY pk",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let preserved: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM orbit_pipelines WHERE project='project-a'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pk_columns, vec!["project", "id"]);
        assert_eq!(preserved, 1);
    }

    #[tokio::test]
    async fn v5_global_patterns_gain_nonempty_session_identity_without_losing_legacy_rows() {
        let pool = super::super::pool::test_memory_pool().await;
        sqlx::query("CREATE TABLE _harness_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO _harness_meta (key, value) VALUES ('schema_version', '5')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE global_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                project TEXT NOT NULL,
                success_rate REAL NOT NULL DEFAULT 0.0,
                avg_score REAL NOT NULL DEFAULT 0.0,
                per_error_stats TEXT NOT NULL DEFAULT '{}',
                failure_patterns TEXT NOT NULL DEFAULT '[]',
                weak_tools TEXT NOT NULL DEFAULT '[]'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        for timestamp in ["1", "2"] {
            sqlx::query(
                "INSERT INTO global_patterns (timestamp, project)
                 VALUES (?, 'project-a')",
            )
            .bind(timestamp)
            .execute(&pool)
            .await
            .unwrap();
        }

        super::init_schema_pool(&pool).await.unwrap();

        let legacy_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM global_patterns
             WHERE project = 'project-a' AND session_id = ''",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_rows, 2);

        sqlx::query(
            "INSERT INTO global_patterns (timestamp, project, session_id)
             VALUES ('3', 'project-a', 'session-a')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let duplicate = sqlx::query(
            "INSERT INTO global_patterns (timestamp, project, session_id)
             VALUES ('4', 'project-a', 'session-a')",
        )
        .execute(&pool)
        .await;
        assert!(duplicate.is_err());
    }

    #[tokio::test]
    async fn migrate_metrics_state_legacy_single_pk_to_composite() {
        // Regression: a legacy single-PK metrics_state must migrate to the
        // composite (key, project) PK without error. Previously the rebuild
        // used `INSERT ... SELECT ... ON CONFLICT (key, project) DO UPDATE`,
        // which sqlx's Any driver rejected as `near "DO"` (a constraint-match
        // failure surfaced as a syntax error), leaving metrics_state stuck on
        // the single PK and breaking every reflect SQLite read.
        let pool = super::super::pool::test_memory_pool().await;

        sqlx::query(
            "CREATE TABLE metrics_state \
             (key TEXT PRIMARY KEY, value TEXT NOT NULL, project TEXT NOT NULL DEFAULT '')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO metrics_state (key, value, project) \
             VALUES ('total_sessions', '5', 'demo')",
        )
        .execute(&pool)
        .await
        .unwrap();

        super::init_schema_pool(&pool).await.unwrap();

        let pk_cols: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('metrics_state') WHERE pk > 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            pk_cols, 2,
            "metrics_state PK must be composite after migration"
        );

        let v: String = sqlx::query_scalar(
            "SELECT value FROM metrics_state WHERE key='total_sessions' AND project='demo'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(v, "5", "row preserved across the rebuild");
    }
}
