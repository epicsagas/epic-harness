//! Atomic relabelling of project records. Conflicting scalar state is retained.
use sqlx::{Executor, SqliteConnection};
use std::io;

pub(super) async fn transfer(
    conn: &mut SqliteConnection,
    from: &str,
    to: &str,
) -> io::Result<usize> {
    conn.execute("BEGIN IMMEDIATE")
        .await
        .map_err(super::sqlx_err)?;
    let result = transfer_inner(conn, from, to).await;
    match result {
        Ok(count) => {
            conn.execute("COMMIT").await.map_err(super::sqlx_err)?;
            Ok(count)
        }
        Err(error) => {
            let _ = conn.execute("ROLLBACK").await;
            Err(error)
        }
    }
}

async fn transfer_inner(conn: &mut SqliteConnection, from: &str, to: &str) -> io::Result<usize> {
    let mut total = 0;
    // Cursors from separate project streams cannot be combined without skipping
    // pending observations. Reset derived progress, retaining all observations.
    sqlx::query("DELETE FROM metrics_state WHERE key='reflection_cursor' AND project=?")
        .bind(from)
        .execute(&mut *conn)
        .await
        .map_err(super::sqlx_err)?;
    sqlx::query("INSERT INTO metrics_state(key,value,project) VALUES('reflection_cursor','0',?) ON CONFLICT(key,project) DO UPDATE SET value=excluded.value")
        .bind(to)
        .execute(&mut *conn)
        .await
        .map_err(super::sqlx_err)?;
    for table in [
        "observations",
        "sessions",
        "evolution_records",
        "score_history",
        "orch_runs",
        "orch_control",
        "orbit_pipelines",
        "evolved_skills",
        "global_patterns",
        "metrics_state",
        "skill_attribution",
    ] {
        let sql = format!("UPDATE {table} SET project = ? WHERE project = ?");
        total += sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(to)
            .bind(from)
            .execute(&mut *conn)
            .await
            .map_err(super::sqlx_err)?
            .rows_affected() as usize;
    }
    // Counter aggregation and source-only relabelling share the same transaction.
    sqlx::query("UPDATE promotion_counters SET count = count + (SELECT count FROM promotion_counters AS src WHERE src.project = ? AND src.pattern_key = promotion_counters.pattern_key) WHERE project = ? AND pattern_key IN (SELECT pattern_key FROM promotion_counters WHERE project = ?)")
        .bind(from).bind(to).bind(from).execute(&mut *conn).await.map_err(super::sqlx_err)?;
    sqlx::query("DELETE FROM promotion_counters WHERE project = ?")
        .bind(from)
        .execute(&mut *conn)
        .await
        .map_err(super::sqlx_err)?;
    total += sqlx::query("UPDATE promotion_counters SET project = ? WHERE project = ?")
        .bind(to)
        .bind(from)
        .execute(&mut *conn)
        .await
        .map_err(super::sqlx_err)?
        .rows_affected() as usize;
    Ok(total)
}
