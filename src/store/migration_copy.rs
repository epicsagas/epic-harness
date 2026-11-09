//! Complete, schema-aware transfer of a supported attached operational database.
use super::{GlobalMergeStats, begin_immediate, commit, rollback, sqlx_err};
use sqlx::{Row, SqliteConnection};
use std::collections::HashSet;
use std::io;

// Parents precede their foreign-key children. Schema metadata is destination-owned.
const TABLES: &[&str] = &[
    "observations",
    "sessions",
    "evolution_records",
    "metrics_state",
    "score_history",
    "skill_attribution",
    "orch_runs",
    "orch_agents",
    "orch_agent_events",
    "orch_agent_inbox",
    "orch_control",
    "orbit_pipelines",
    "evolved_skills",
    "promotion_counters",
    "workspace_manifest",
    "global_patterns",
];

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(super) async fn merge(
    conn: &mut SqliteConnection,
    slug: &str,
    source: &str,
) -> io::Result<GlobalMergeStats> {
    if source.is_empty()
        || !source
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid attached schema name",
        ));
    }
    begin_immediate(conn).await?;
    let transfer = async {
        let mut stats = GlobalMergeStats::default();
        for table in TABLES {
            let count = copy_table(conn, slug, source, table).await?;
            match *table {
                "observations" => stats.obs += count,
                "sessions" => stats.sessions += count,
                "evolution_records" => stats.evo += count,
                "metrics_state" => stats.metrics += count,
                "score_history" => stats.score_history += count,
                "skill_attribution" => stats.skill_attr += count,
                "promotion_counters" => stats.promo += count,
                _ => {}
            }
        }
        if stats.obs > 0 {
            // Observation IDs are allocated again at the destination. Replay is
            // safer than treating a cursor from either ID space as authoritative.
            sqlx::query(
                "INSERT INTO metrics_state(key,value,project) VALUES('reflection_cursor','0',?) ON CONFLICT(key,project) DO UPDATE SET value=excluded.value",
            )
            .bind(slug)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
        }
        Ok(stats)
    }
    .await;
    match transfer {
        Ok(stats) => {
            commit(conn).await?;
            Ok(stats)
        }
        Err(error) => {
            rollback(conn).await;
            Err(error)
        }
    }
}

async fn columns(
    conn: &mut SqliteConnection,
    schema: &str,
    table: &str,
) -> io::Result<Vec<String>> {
    let sql = format!("PRAGMA {}.table_info({})", quoted(schema), quoted(table));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(conn)
        .await
        .map_err(sqlx_err)?;
    rows.into_iter()
        .map(|r| r.try_get::<String, _>("name").map_err(sqlx_err))
        .collect()
}

async fn copy_table(
    conn: &mut SqliteConnection,
    slug: &str,
    source: &str,
    table: &str,
) -> io::Result<usize> {
    let source_columns = columns(conn, source, table).await?;
    if source_columns.is_empty() {
        return Ok(0);
    }
    let target_columns = columns(conn, "main", table).await?;
    let source_set: HashSet<_> = source_columns.iter().map(String::as_str).collect();
    if table == "metrics_state" && source_set.contains("key") && source_set.contains("value") {
        let sql = format!(
            "UPDATE main.metrics_state SET value='0' WHERE key='reflection_cursor' AND project=? AND EXISTS(SELECT 1 FROM {}.metrics_state WHERE key='reflection_cursor')",
            quoted(source)
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(slug)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
    }
    if source_columns
        .iter()
        .any(|name| !target_columns.contains(name))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{table}: source columns require a newer destination schema"),
        ));
    }
    let allocate_id = matches!(
        table,
        "observations"
            | "sessions"
            | "evolution_records"
            | "score_history"
            | "orch_agent_events"
            | "orch_agent_inbox"
            | "evolved_skills"
            | "global_patterns"
    );
    let mut dest = Vec::new();
    let mut select = Vec::new();
    let mut project = false;
    for column in target_columns {
        if allocate_id && column == "id" {
            continue;
        }
        if column == "project" {
            dest.push(quoted(&column));
            select.push("?".to_string());
            project = true;
        } else if table == "metrics_state" && column == "value" && source_set.contains("value") {
            dest.push(quoted(&column));
            select.push("CASE WHEN s.key='reflection_cursor' THEN '0' ELSE s.value END".into());
        } else if source_set.contains(column.as_str()) {
            dest.push(quoted(&column));
            select.push(format!("s.{}", quoted(&column)));
        }
        // Missing legacy optional columns intentionally receive destination defaults.
    }
    let dest = dest.join(", ");
    let select = select.join(", ");
    let sql = format!(
        "INSERT OR IGNORE INTO main.{} ({dest}) SELECT {select} FROM {}.{} AS s",
        quoted(table),
        quoted(source),
        quoted(table)
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if project {
        query = query.bind(slug);
    }
    let count = query
        .execute(&mut *conn)
        .await
        .map_err(sqlx_err)?
        .rows_affected() as usize;
    // Ignoring a conflicting natural key is safe only when its complete projected
    // content is already present. Otherwise retain the source and fail atomically.
    let sql = format!(
        "SELECT COUNT(*) FROM (SELECT {select} FROM {}.{} AS s EXCEPT SELECT {dest} FROM main.{})",
        quoted(source),
        quoted(table),
        quoted(table)
    );
    let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql));
    if project {
        query = query.bind(slug);
    }
    let untransferred = query.fetch_one(&mut *conn).await.map_err(sqlx_err)?;
    if untransferred > 0 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{table}: {untransferred} source record(s) conflict with existing destination contents"
            ),
        ));
    }
    Ok(count)
}
