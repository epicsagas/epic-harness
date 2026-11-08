//! Dashboard projection of recorded coding-session activity and subagent state.
//! Recent tool activity is evidence of activity, not proof of process liveness.
use serde_json::{Value, json};
use sqlx::{AnyPool, Row};
use std::{collections::BTreeMap, io, path::Path};

const SESSION_WINDOW: u64 = 300;

fn valid_project(project: &str) -> bool {
    !project.is_empty() && crate::shared::paths::sanitize_slug_name(project) == project
}

fn timestamp_secs(value: &str) -> Option<u64> {
    if value.len() < 19 || !value.is_ascii() {
        return None;
    }
    let year = value[0..4].parse::<u32>().ok()?;
    let month = value[5..7].parse::<u32>().ok()?;
    let day = value[8..10].parse::<u32>().ok()?;
    let hour = value[11..13].parse::<u32>().ok()?;
    let minute = value[14..16].parse::<u32>().ok()?;
    let second = value[17..19].parse::<u32>().ok()?;
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day)
        || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(crate::mem::store::parse_iso_to_secs(value))
}

fn recent(value: &str, now: u64, window: u64) -> bool {
    timestamp_secs(value).is_some_and(|time| time <= now && now - time <= window)
}

fn decode(raw: String, fallback: Value) -> Value {
    serde_json::from_str(&raw).unwrap_or(fallback)
}

fn contained_file(file: &Path, root: &Path) -> bool {
    file.canonicalize().is_ok_and(|path| path.starts_with(root) && path.is_file())
}

/// Read project-scoped activity without creating state or changing lifecycle data.
/// File and SQLite runs share a normalized `agents` array and status map.
pub async fn snapshot(
    pool: &AnyPool,
    projects_root: &Path,
    project: Option<&str>,
    now: u64,
) -> io::Result<Value> {
    if project.is_some_and(|name| !valid_project(name)) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid project"));
    }
    let scope = project.unwrap_or("");
    let mut runs: BTreeMap<(String, String), Value> = BTreeMap::new();
    let rows = sqlx::query(
        "SELECT id, project, status, agents_json, dep_graph_json, created_at, updated_at
         FROM orch_runs WHERE (? = '' OR project = ?) ORDER BY updated_at DESC LIMIT 100",
    ).bind(scope).bind(scope).fetch_all(pool).await.map_err(crate::store::sqlx_err)?;
    for row in rows {
        let name: String = row.try_get("project").map_err(crate::store::sqlx_err)?;
        let id: String = row.try_get("id").map_err(crate::store::sqlx_err)?;
        let agents = decode(row.try_get("agents_json").map_err(crate::store::sqlx_err)?, json!([]));
        runs.insert((name.clone(), id.clone()), json!({
            "id": id, "project": name,
            "status": row.try_get::<String, _>("status").map_err(crate::store::sqlx_err)?,
            "agents": if agents.is_array() { agents } else { json!([]) },
            "dependency_graph": decode(row.try_get("dep_graph_json").map_err(crate::store::sqlx_err)?, json!({})),
            "created_at": row.try_get::<String, _>("created_at").map_err(crate::store::sqlx_err)?,
            "updated_at": row.try_get::<String, _>("updated_at").map_err(crate::store::sqlx_err)?,
            "agent_statuses": {},
        }));
    }
    // Fetch persisted agent updates together, rather than one request/query per card.
    let rows = sqlx::query(
        "SELECT a.*, r.project FROM orch_agents a JOIN orch_runs r ON r.id = a.run_id
         WHERE (? = '' OR r.project = ?)",
    ).bind(scope).bind(scope).fetch_all(pool).await.map_err(crate::store::sqlx_err)?;
    for row in rows {
        let name: String = row.try_get("project").map_err(crate::store::sqlx_err)?;
        let run_id: String = row.try_get("run_id").map_err(crate::store::sqlx_err)?;
        let Some(run) = runs.get_mut(&(name, run_id)) else { continue };
        let id: String = row.try_get("id").map_err(crate::store::sqlx_err)?;
        let status: String = row.try_get("status").map_err(crate::store::sqlx_err)?;
        let agent = json!({
            "id": id, "status": status,
            "role": row.try_get::<String, _>("role").map_err(crate::store::sqlx_err)?,
            "task": row.try_get::<String, _>("task").map_err(crate::store::sqlx_err)?,
            "satisfies": decode(row.try_get("satisfies_json").map_err(crate::store::sqlx_err)?, json!([])),
            "started_at": row.try_get::<Option<String>, _>("started_at").map_err(crate::store::sqlx_err)?,
            "completed_at": row.try_get::<Option<String>, _>("completed_at").map_err(crate::store::sqlx_err)?,
        });
        let agents = run["agents"].as_array_mut().unwrap();
        if let Some(existing) = agents.iter_mut().find(|agent| agent["id"] == id) { *existing = agent; }
        else { agents.push(agent); }
        run["agent_statuses"][&id] = json!({
            "status": status,
            "phase": row.try_get::<String, _>("phase").map_err(crate::store::sqlx_err)?,
            "progress": row.try_get::<f64, _>("progress").map_err(crate::store::sqlx_err)?,
            "last_heartbeat": row.try_get::<String, _>("last_heartbeat").map_err(crate::store::sqlx_err)?,
        });
    }
    if let (Ok(root), Ok(entries)) = (projects_root.canonicalize(), std::fs::read_dir(projects_root)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !valid_project(&name) || project.is_some_and(|scope| scope != name) { continue; }
            let Ok(directory) = entry.path().canonicalize() else { continue };
            if !directory.starts_with(&root) || !directory.is_dir() { continue; }
            if !contained_file(&directory.join("orchestrator").join("run.json"), &directory) { continue; }
            let Some(run) = crate::orchestrate::state::read_run(&directory) else { continue };
            let key = (name.clone(), run.id.clone());
            if runs.get(&key).is_some_and(|old| old["updated_at"].as_str().unwrap_or("") > run.updated_at.as_str()) { continue; }
            let mut statuses = serde_json::Map::new();
            for agent in &run.agents {
                if crate::orchestrate::state::validate_agent_id(&agent.id)
                    && contained_file(&directory.join("orchestrator").join("agents").join(&agent.id).join("status.json"), &directory)
                    && let Some(status) = crate::orchestrate::state::read_agent_status(&directory, &agent.id) {
                    statuses.insert(agent.id.clone(), json!(status));
                }
            }
            let mut value = json!(run);
            value["project"] = json!(name);
            value["agent_statuses"] = json!(statuses);
            runs.insert(key, value);
        }
    }
    let mut runs: Vec<Value> = runs.into_values().filter(|run| {
        run["agents"].as_array().is_some_and(|agents| !agents.is_empty())
            && (run["status"] == "running" || recent(run["updated_at"].as_str().unwrap_or(""), now, 86400))
    }).collect();
    runs.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
    let rows = sqlx::query(
        "SELECT project, session_id, MAX(timestamp) AS last_seen, COUNT(*) AS calls,
                COALESCE(AVG(score), 0.0) AS avg_score,
                SUM(CASE WHEN result = 'error' THEN 1 ELSE 0 END) AS failures
         FROM observations WHERE (? = '' OR project = ?) AND session_id <> ''
         GROUP BY project, session_id ORDER BY last_seen DESC LIMIT 100",
    ).bind(scope).bind(scope).fetch_all(pool).await.map_err(crate::store::sqlx_err)?;
    let mut sessions = Vec::new();
    let mut history = Vec::new();
    for row in rows {
        let last_seen: String = row.try_get("last_seen").map_err(crate::store::sqlx_err)?;
        let value = json!({
            "session_id": row.try_get::<String, _>("session_id").map_err(crate::store::sqlx_err)?,
            "project": row.try_get::<String, _>("project").map_err(crate::store::sqlx_err)?,
            "date": if timestamp_secs(&last_seen).is_some() { &last_seen[..10] } else { "unknown" },
            "last_seen": last_seen,
            "tool_calls": row.try_get::<i64, _>("calls").map_err(crate::store::sqlx_err)?,
            "avg_score": row.try_get::<f64, _>("avg_score").map_err(crate::store::sqlx_err)?,
            "failures": row.try_get::<i64, _>("failures").map_err(crate::store::sqlx_err)?,
        });
        if recent(&last_seen, now, SESSION_WINDOW) { sessions.push(value.clone()); }
        if history.len() < 20 { history.push(value); }
    }
    Ok(json!({"runs": runs, "sessions": sessions, "recent_sessions": history, "activity_window_seconds": SESSION_WINDOW}))
}

#[cfg(test)]
mod tests;
