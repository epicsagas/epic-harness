//! sessions.rs — Session snapshot SQLite I/O

use rusqlite::Connection;
use std::io;

use crate::shared::types::SessionSnapshot;

use super::store_err;

/// Insert a session snapshot.
pub fn insert_snapshot_conn(
    conn: &Connection,
    snap: &SessionSnapshot,
    created_at_millis: i64,
) -> io::Result<i64> {
    let pending_json = serde_json::to_string(&snap.pending_tasks).unwrap_or_else(|_| "[]".into());
    let pipeline_json = snap
        .pipeline_state
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));

    store_err(conn.execute(
        "INSERT INTO sessions
         (timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state, created_at_millis)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            snap.timestamp,
            snap.snap_type,
            snap.summary,
            pending_json,
            snap.context_usage,
            pipeline_json,
            created_at_millis,
        ],
    ))?;
    Ok(conn.last_insert_rowid())
}

/// Map a session row to a [`SessionSnapshot`].
fn map_session_row(row: &rusqlite::Row<'_>) -> Result<SessionSnapshot, rusqlite::Error> {
    let pending_json: String = row.get(3)?;
    let pending_tasks: Vec<String> = serde_json::from_str(&pending_json).unwrap_or_default();
    let pipeline_json: Option<String> = row.get(5)?;
    let pipeline_state: Option<serde_json::Value> = pipeline_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    Ok(SessionSnapshot {
        timestamp: row.get(0)?,
        snap_type: row.get(1)?,
        summary: row.get(2)?,
        pending_tasks,
        context_usage: row.get(4)?,
        pipeline_state,
    })
}

/// Get the most recent session snapshot.
pub fn get_latest_snapshot_conn(conn: &Connection) -> io::Result<Option<SessionSnapshot>> {
    let result = conn.query_row(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state
         FROM sessions ORDER BY id DESC LIMIT 1",
        [],
        map_session_row,
    );

    match result {
        Ok(snap) => Ok(Some(snap)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

/// List the N most recent session snapshots.
pub fn list_recent_snapshots_conn(
    conn: &Connection,
    limit: usize,
) -> io::Result<Vec<SessionSnapshot>> {
    let mut stmt = store_err(conn.prepare(
        "SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state
             FROM sessions ORDER BY id DESC LIMIT ?1",
    ))?;

    let rows = store_err(stmt.query_map(rusqlite::params![limit as i64], map_session_row))?;

    let mut snaps = Vec::new();
    for r in rows {
        snaps.push(store_err(r)?);
    }
    Ok(snaps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        super::super::tests::in_memory_db()
    }

    #[test]
    fn insert_and_get_latest() {
        let conn = in_memory_db();
        let snap = SessionSnapshot {
            timestamp: "2026-06-02T10:00:00Z".into(),
            snap_type: "pre-compact".into(),
            summary: "Test summary".into(),
            pending_tasks: vec!["task1".into(), "task2".into()],
            context_usage: Some(0.75),
            pipeline_state: None,
        };

        insert_snapshot_conn(&conn, &snap, 1000).unwrap();

        let latest = get_latest_snapshot_conn(&conn).unwrap();
        assert!(latest.is_some());
        let s = latest.unwrap();
        assert_eq!(s.summary, "Test summary");
        assert_eq!(s.pending_tasks.len(), 2);
    }

    #[test]
    fn get_latest_when_empty() {
        let conn = in_memory_db();
        let result = get_latest_snapshot_conn(&conn).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn list_recent_snapshots() {
        let conn = in_memory_db();

        for i in 0..5 {
            let snap = SessionSnapshot {
                timestamp: format!("2026-06-02T10:0{}:00Z", i),
                snap_type: "pre-compact".into(),
                summary: format!("Session {}", i),
                pending_tasks: vec![],
                context_usage: None,
                pipeline_state: None,
            };
            insert_snapshot_conn(&conn, &snap, 1000 + i as i64).unwrap();
        }

        let recent = list_recent_snapshots_conn(&conn, 3).unwrap();
        assert_eq!(recent.len(), 3);
        // Most recent first (id DESC)
        assert_eq!(recent[0].summary, "Session 4");
    }
}
