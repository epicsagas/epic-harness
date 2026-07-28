//! orbit_store.rs — Orbit pipeline SQLite I/O
#![allow(dead_code)]

use sqlx::any::AnyRow;
use sqlx::{AnyPool, Row};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn require_identity(project: &str, pipeline_id: &str) -> io::Result<()> {
    if project.is_empty() || project == "__all__" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a concrete Orbit project is required",
        ));
    }
    if pipeline_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orbit pipeline ID must not be empty",
        ));
    }
    Ok(())
}

/// Upsert one project-scoped pipeline state.
pub async fn upsert_pipeline_pool(
    pool: &AnyPool,
    id: &str,
    project: &str,
    status: &str,
    phase: Option<&str>,
    mode: Option<&str>,
    state_json: &str,
) -> io::Result<()> {
    require_identity(project, id)?;
    let now = crate::shared::helpers::now_iso();
    sqlx::query(
        "INSERT INTO orbit_pipelines
         (id, project, status, phase, mode, state_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(project, id) DO UPDATE SET
             status = excluded.status,
             phase = excluded.phase,
             mode = excluded.mode,
             state_json = excluded.state_json,
             updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(project)
    .bind(status)
    .bind(phase)
    .bind(mode)
    .bind(state_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// Find a running pipeline, optionally filtered by project.
pub async fn read_running_pipeline_pool(
    pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<Option<serde_json::Value>> {
    let row = match project {
        Some(p) => {
            sqlx::query(
                "SELECT state_json FROM orbit_pipelines WHERE status = 'running' AND project = ? LIMIT 1",
            )
            .bind(p)
            .fetch_optional(pool)
            .await
        }
        None => {
            sqlx::query(
                "SELECT state_json FROM orbit_pipelines WHERE status = 'running' LIMIT 1",
            )
            .fetch_optional(pool)
            .await
        }
    }
    .map_err(super::sqlx_err)?;

    match row {
        Some(r) => {
            let json_str: String = r.try_get(0).map_err(super::sqlx_err)?;
            let val: serde_json::Value = serde_json::from_str(&json_str)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            Ok(Some(val))
        }
        None => Ok(None),
    }
}

/// List all pipelines across all projects (for dashboard).
pub async fn list_all_pipelines_pool(pool: &AnyPool) -> io::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
         FROM orbit_pipelines ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    pipeline_values(rows)
}

fn pipeline_values(rows: Vec<AnyRow>) -> io::Result<Vec<serde_json::Value>> {
    let mut pipelines = Vec::with_capacity(rows.len());
    for r in rows {
        let id: String = r.try_get(0).map_err(super::sqlx_err)?;
        let project: String = r.try_get(1).map_err(super::sqlx_err)?;
        let status: String = r.try_get(2).map_err(super::sqlx_err)?;
        let phase: Option<String> = r.try_get(3).map_err(super::sqlx_err)?;
        let mode: Option<String> = r.try_get(4).map_err(super::sqlx_err)?;
        let state_json: String = r.try_get(5).map_err(super::sqlx_err)?;
        let created_at: String = r.try_get(6).map_err(super::sqlx_err)?;
        let updated_at: String = r.try_get(7).map_err(super::sqlx_err)?;

        let mut val: serde_json::Value = serde_json::from_str(&state_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let map = val.as_object_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Orbit pipeline {project}/{id} state must be a JSON object"),
            )
        })?;
        map.insert("id".into(), serde_json::Value::String(id));
        map.insert("project".into(), serde_json::Value::String(project));
        map.insert("status".into(), serde_json::Value::String(status));
        if let Some(p) = phase {
            map.insert("phase".into(), serde_json::Value::String(p));
        }
        if let Some(m) = mode {
            map.insert("mode".into(), serde_json::Value::String(m));
        }
        map.insert("started_at".into(), serde_json::Value::String(created_at));
        map.insert("updated_at".into(), serde_json::Value::String(updated_at));
        pipelines.push(val);
    }
    Ok(pipelines)
}

/// List pipelines with a row limit (most recent first).
pub async fn list_all_pipelines_pool_limited(
    pool: &AnyPool,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    list_pipelines_scoped_pool(pool, None, limit).await
}

/// List a bounded set of pipelines, optionally scoped to one project.
pub async fn list_pipelines_scoped_pool(
    pool: &AnyPool,
    project: Option<&str>,
    limit: i64,
) -> io::Result<Vec<serde_json::Value>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let rows = if let Some(project) = project {
        if project.is_empty() || project == "__all__" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a selected project must be a concrete project slug",
            ));
        }
        sqlx::query(
            "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
             FROM orbit_pipelines WHERE project = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(project)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, project, status, phase, mode, state_json, created_at, updated_at
             FROM orbit_pipelines ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .map_err(super::sqlx_err)?;
    pipeline_values(rows)
}

/// Dismiss one project-scoped pipeline row.
pub async fn dismiss_pipeline_pool(
    pool: &AnyPool,
    project: &str,
    pipeline_id: &str,
) -> io::Result<bool> {
    require_identity(project, pipeline_id)?;
    let result = sqlx::query("DELETE FROM orbit_pipelines WHERE project = ? AND id = ?")
        .bind(project)
        .bind(pipeline_id)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

/// Update pipeline status only.
pub async fn update_pipeline_status_pool(
    pool: &AnyPool,
    project: &str,
    pipeline_id: &str,
    status: &str,
) -> io::Result<bool> {
    require_identity(project, pipeline_id)?;
    let now = crate::shared::helpers::now_iso();
    let result = sqlx::query(
        "UPDATE orbit_pipelines SET status = ?, updated_at = ? WHERE project = ? AND id = ?",
    )
    .bind(status)
    .bind(&now)
    .bind(project)
    .bind(pipeline_id)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DismissPipelineResult {
    pub deleted_file: bool,
    pub deleted_row: bool,
}

fn exact_pipeline_file(
    project_harness_dir: &Path,
    pipeline_id: &str,
) -> io::Result<Option<PathBuf>> {
    let orbit_dir = project_harness_dir.join("orbit");
    match fs::symlink_metadata(&orbit_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Orbit directory must not be a symlink: {}",
                    orbit_dir.display()
                ),
            ));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Orbit path is not a directory: {}", orbit_dir.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }

    let mut matched = None;
    for entry in fs::read_dir(&orbit_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("PIPELINE-") || !name.ends_with(".json") {
            continue;
        }

        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Orbit pipeline entry is not a regular file: {name}"),
            ));
        }

        let content = fs::read_to_string(entry.path())?;
        let state: serde_json::Value = serde_json::from_str(&content)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if state.get("id").and_then(serde_json::Value::as_str) != Some(pipeline_id) {
            continue;
        }
        if matched.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("multiple Orbit files have ID {pipeline_id}"),
            ));
        }
        matched = Some(entry.path());
    }
    Ok(matched)
}

/// Dismiss one exact pipeline from its selected project file store and SQLite.
///
/// The operation is safe to retry when either side is already absent.
pub async fn dismiss_pipeline_state_pool(
    pool: &AnyPool,
    project: &str,
    pipeline_id: &str,
    project_harness_dir: &Path,
) -> io::Result<DismissPipelineResult> {
    require_identity(project, pipeline_id)?;
    let pipeline_file = exact_pipeline_file(project_harness_dir, pipeline_id)?;
    let deleted_file = if let Some(path) = pipeline_file {
        fs::remove_file(path)?;
        true
    } else {
        false
    };
    let deleted_row = dismiss_pipeline_pool(pool, project, pipeline_id).await?;
    Ok(DismissPipelineResult {
        deleted_file,
        deleted_row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn upsert_and_read_running() {
        let pool = test_pool().await;
        upsert_pipeline_pool(
            &pool,
            "PIPELINE-20260602-abc",
            "my-project",
            "running",
            Some("go"),
            Some("direct"),
            r#"{"requirement":"fix bug"}"#,
        )
        .await
        .unwrap();

        let result = read_running_pipeline_pool(&pool, Some("my-project"))
            .await
            .unwrap();
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["requirement"], "fix bug");
    }

    #[tokio::test]
    async fn list_all_pipelines() {
        let pool = test_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj-a", "running", None, None, "{}")
            .await
            .unwrap();
        upsert_pipeline_pool(&pool, "PIPELINE-2", "proj-b", "complete", None, None, "{}")
            .await
            .unwrap();

        let pipelines = list_all_pipelines_pool(&pool).await.unwrap();
        assert_eq!(pipelines.len(), 2);
    }

    #[tokio::test]
    async fn same_pipeline_id_coexists_in_two_projects() {
        let pool = test_pool().await;
        upsert_pipeline_pool(
            &pool,
            "PIPELINE-same",
            "proj-a",
            "running",
            None,
            None,
            r#"{"v":"a"}"#,
        )
        .await
        .unwrap();
        upsert_pipeline_pool(
            &pool,
            "PIPELINE-same",
            "proj-b",
            "complete",
            None,
            None,
            r#"{"v":"b"}"#,
        )
        .await
        .unwrap();

        let pipelines = list_all_pipelines_pool(&pool).await.unwrap();
        assert_eq!(pipelines.len(), 2);
        assert!(pipelines.iter().any(|pipeline| {
            pipeline["id"] == "PIPELINE-same"
                && pipeline["project"] == "proj-a"
                && pipeline["v"] == "a"
        }));
        assert!(pipelines.iter().any(|pipeline| {
            pipeline["id"] == "PIPELINE-same"
                && pipeline["project"] == "proj-b"
                && pipeline["v"] == "b"
        }));
    }

    #[tokio::test]
    async fn dismiss_pipeline_is_project_scoped() {
        let pool = test_pool().await;
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj-a", "running", None, None, "{}")
            .await
            .unwrap();
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj-b", "running", None, None, "{}")
            .await
            .unwrap();

        let dismissed = dismiss_pipeline_pool(&pool, "proj-a", "PIPELINE-1")
            .await
            .unwrap();
        assert!(dismissed);

        assert!(
            read_running_pipeline_pool(&pool, Some("proj-a"))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            read_running_pipeline_pool(&pool, Some("proj-b"))
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn dismiss_pipeline_rejects_empty_identity() {
        let pool = test_pool().await;
        assert!(dismiss_pipeline_pool(&pool, "proj", "").await.is_err());
        assert!(
            dismiss_pipeline_pool(&pool, "", "PIPELINE-1")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn dismiss_pipeline_state_matches_exact_id_in_selected_project() {
        let pool = test_pool().await;
        let selected = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::create_dir(selected.path().join("orbit")).unwrap();
        fs::create_dir(other.path().join("orbit")).unwrap();
        fs::write(
            selected.path().join("orbit/PIPELINE-short.json"),
            r#"{"id":"PIPELINE-12"}"#,
        )
        .unwrap();
        fs::write(
            selected.path().join("orbit/PIPELINE-exact.json"),
            r#"{"id":"PIPELINE-123"}"#,
        )
        .unwrap();
        fs::write(
            other.path().join("orbit/PIPELINE-exact.json"),
            r#"{"id":"PIPELINE-123"}"#,
        )
        .unwrap();

        let result = dismiss_pipeline_state_pool(&pool, "proj-a", "PIPELINE-123", selected.path())
            .await
            .unwrap();

        assert_eq!(
            result,
            DismissPipelineResult {
                deleted_file: true,
                deleted_row: false,
            }
        );
        assert!(selected.path().join("orbit/PIPELINE-short.json").exists());
        assert!(!selected.path().join("orbit/PIPELINE-exact.json").exists());
        assert!(other.path().join("orbit/PIPELINE-exact.json").exists());
    }

    #[tokio::test]
    async fn dismiss_pipeline_state_is_retry_safe() {
        let pool = test_pool().await;
        let selected = tempfile::tempdir().unwrap();
        fs::create_dir(selected.path().join("orbit")).unwrap();
        fs::write(
            selected.path().join("orbit/PIPELINE-1.json"),
            r#"{"id":"PIPELINE-1"}"#,
        )
        .unwrap();
        upsert_pipeline_pool(&pool, "PIPELINE-1", "proj", "running", None, None, "{}")
            .await
            .unwrap();

        let first = dismiss_pipeline_state_pool(&pool, "proj", "PIPELINE-1", selected.path())
            .await
            .unwrap();
        let second = dismiss_pipeline_state_pool(&pool, "proj", "PIPELINE-1", selected.path())
            .await
            .unwrap();

        assert_eq!(
            first,
            DismissPipelineResult {
                deleted_file: true,
                deleted_row: true,
            }
        );
        assert_eq!(
            second,
            DismissPipelineResult {
                deleted_file: false,
                deleted_row: false,
            }
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dismiss_pipeline_state_rejects_symlinked_orbit_directory() {
        use std::os::unix::fs::symlink;

        let pool = test_pool().await;
        let selected = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        symlink(target.path(), selected.path().join("orbit")).unwrap();

        assert!(
            dismiss_pipeline_state_pool(&pool, "proj", "PIPELINE-1", selected.path())
                .await
                .is_err()
        );
    }
}
