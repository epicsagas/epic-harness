//! Data projections used by native commands and headless contract tests.

use epic_harness::{shared, store};
use sqlx::{AnyPool, Row};
use std::io;

pub fn project_scope(project: Option<&str>) -> Option<&str> {
    project.filter(|p| !p.is_empty() && *p != "__all__")
}

pub async fn metrics(
    pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<shared::evolution::Metrics> {
    store::metrics::load_metrics_scoped_pool(pool, project_scope(project)).await
}

pub async fn observations(
    pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<store::observations::ObsStats> {
    store::observations::query_obs_stats_scoped_pool(
        pool,
        "2000-01-01",
        "2099-12-31",
        project_scope(project),
    )
    .await
}

pub async fn history(
    pool: &AnyPool,
    limit: i64,
    project: Option<&str>,
) -> io::Result<Vec<shared::evolution::EvolutionRecord>> {
    store::evolution::query_recent_records_scoped_pool(pool, limit, project_scope(project)).await
}

pub async fn snapshots(
    pool: &AnyPool,
    limit: i64,
    project: Option<&str>,
) -> io::Result<Vec<shared::types::SessionSnapshot>> {
    let Some(project) = project_scope(project) else {
        return store::sessions::list_recent_snapshots_pool(pool, limit).await;
    };
    let rows = sqlx::query("SELECT timestamp, snap_type, summary, pending_tasks, context_usage, pipeline_state FROM sessions WHERE project = ? ORDER BY id DESC LIMIT ?")
        .bind(project).bind(limit).fetch_all(pool).await.map_err(io::Error::other)?;
    rows.into_iter()
        .map(|r| {
            let pending: String = r.try_get("pending_tasks").map_err(io::Error::other)?;
            let pipeline: Option<String> = r.try_get("pipeline_state").map_err(io::Error::other)?;
            Ok(shared::types::SessionSnapshot {
                timestamp: r.try_get("timestamp").map_err(io::Error::other)?,
                snap_type: r.try_get("snap_type").map_err(io::Error::other)?,
                summary: r.try_get("summary").map_err(io::Error::other)?,
                pending_tasks: serde_json::from_str(&pending).map_err(io::Error::other)?,
                context_usage: r.try_get("context_usage").map_err(io::Error::other)?,
                pipeline_state: pipeline
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(io::Error::other)?,
            })
        })
        .collect()
}
