//! Native IPC projections for the same SQLite/file state used by hooks.

use super::native_files;
use epic_harness::{orchestrate::state::validate_agent_id, store::orchestrator};
use serde_json::{Value, json};
use sqlx::{AnyPool, Row};
use std::path::Path;

fn agent_path(agent_id: &str, file: &str) -> Result<String, String> {
    if !validate_agent_id(agent_id) {
        return Err("invalid agent id".into());
    }
    Ok(format!("orchestrator/agents/{agent_id}/{file}"))
}

pub async fn run(pool: &AnyPool, project: &Path, scoped: bool) -> Result<Value, String> {
    let file = native_files::read_json(project, "orchestrator/run.json", Value::Null)?;
    if !file.is_null() || scoped {
        return Ok(file);
    }
    let Some(run) = orchestrator::read_run_pool(pool)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(Value::Null);
    };
    Ok(json!({"id":run.id,"status":run.status,
        "agents":serde_json::from_str::<Value>(&run.agents_json).map_err(|e| e.to_string())?,
        "dependency_graph":serde_json::from_str::<Value>(&run.dep_graph_json).map_err(|e| e.to_string())?,
        "created_at":run.created_at,"updated_at":run.updated_at}))
}

async fn visible_agent(
    pool: &AnyPool,
    project: &Path,
    scoped: bool,
    id: &str,
) -> Result<Option<orchestrator::OrchAgent>, String> {
    if !validate_agent_id(id) {
        return Err("invalid agent id".into());
    }
    let row = orchestrator::read_agent_pool(pool, id)
        .await
        .map_err(|e| e.to_string())?;
    if !scoped {
        return Ok(row);
    }
    let run = native_files::read_json(project, "orchestrator/run.json", Value::Null)?;
    Ok(row.filter(|agent| run["id"].as_str() == Some(agent.run_id.as_str())))
}

pub async fn status(
    pool: &AnyPool,
    project: &Path,
    scoped: bool,
    id: &str,
) -> Result<Value, String> {
    let file = native_files::read_json(project, &agent_path(id, "status.json")?, Value::Null)?;
    if !file.is_null() {
        return Ok(file);
    }
    Ok(match visible_agent(pool, project, scoped, id).await? {
        Some(a) => {
            json!({"agent_id":a.id,"phase":a.phase,"progress":a.progress,"last_heartbeat":a.last_heartbeat,"status":a.status})
        }
        None => Value::Null,
    })
}

pub async fn events(
    pool: &AnyPool,
    project: &Path,
    scoped: bool,
    id: &str,
) -> Result<Vec<Value>, String> {
    let file = native_files::read_jsonl(project, &agent_path(id, "stream.jsonl")?, 100)?;
    if !file.is_empty() {
        return Ok(file);
    }
    if visible_agent(pool, project, scoped, id).await?.is_none() {
        return Ok(vec![]);
    }
    let rows=sqlx::query("SELECT timestamp,event_type,data_json FROM orch_agent_events WHERE agent_id=? ORDER BY id DESC LIMIT 100").bind(id).fetch_all(pool).await.map_err(|e|e.to_string())?;
    rows.into_iter().map(|r|{let data:String=r.try_get("data_json").map_err(|e|e.to_string())?;Ok(json!({"timestamp":r.try_get::<String,_>("timestamp").map_err(|e|e.to_string())?,"event_type":r.try_get::<String,_>("event_type").map_err(|e|e.to_string())?,"data":serde_json::from_str::<Value>(&data).map_err(|e|e.to_string())?}))}).collect()
}

pub async fn inbox(
    pool: &AnyPool,
    project: &Path,
    scoped: bool,
    id: &str,
) -> Result<Vec<Value>, String> {
    let file = native_files::read_jsonl(project, &agent_path(id, "inbox.jsonl")?, 100)?;
    if !file.is_empty() {
        return Ok(file);
    }
    if visible_agent(pool, project, scoped, id).await?.is_none() {
        return Ok(vec![]);
    }
    let rows=sqlx::query("SELECT from_agent,timestamp,message FROM orch_agent_inbox WHERE agent_id=? ORDER BY id DESC LIMIT 100").bind(id).fetch_all(pool).await.map_err(|e|e.to_string())?;
    rows.into_iter().map(|r|Ok(json!({"to":id,"from":r.try_get::<String,_>("from_agent").map_err(|e|e.to_string())?,"timestamp":r.try_get::<String,_>("timestamp").map_err(|e|e.to_string())?,"message":r.try_get::<String,_>("message").map_err(|e|e.to_string())?}))).collect()
}
