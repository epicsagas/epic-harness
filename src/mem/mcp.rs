//! mcp.rs — Stdio JSON-RPC 2.0 MCP server for the unified memory system
//!
//! Memory features require the `epic-harness` binary — no Node.js runtime needed.
//! Usage: `epic-harness mem mcp`
//!
//! Implements MCP protocol version 2024-11-05 over stdin/stdout.
//! Tools: mem_add, mem_query, mem_search, mem_related, mem_context

use serde_json::{Value, json};
use sqlx::AnyPool;
use std::io::{self, BufRead, Write};

use super::graph::{graph_neighbors_pool, related_nodes_pool};
use super::store::{
    Node, NodeFrontmatter, importance_for_type, new_uuid, now_iso, query_nodes_pool,
    read_node_pool, read_nodes_pool, search_nodes_pool, smart_recall_pool, touch_nodes_pool,
    validate_uuid, write_node_dedup_pool,
};
use crate::store::runtime;

// ── Tool definitions ───────────────────────────────────────────────────────────

fn tool_definitions() -> Value {
    json!([
        {
            "name": "mem_add",
            "description": "Add a new memory node to the unified knowledge graph. Use for architectural decisions, patterns, recurring errors, or project-specific knowledge.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title":   { "type": "string", "description": "Short descriptive title" },
                    "type": {
                        "type": "string",
                        "enum": ["concept", "pattern", "project", "decision", "error", "session", "resolution"],
                        "description": "Node type"
                    },
                    "body":    { "type": "string", "description": "Markdown content (the actual knowledge)" },
                    "tags":    { "type": "array", "items": { "type": "string" }, "description": "Tags for filtering" },
                    "project": { "type": "string", "description": "Project slug (optional)" },
                    "importance": { "type": "number", "description": "Importance score 0.0-1.0 (auto-set by type if omitted: decision=0.9, resolution=0.8, concept=0.7, pattern=0.5, error=0.4, session=0.2)" }
                },
                "required": ["title", "type", "body"]
            }
        },
        {
            "name": "mem_query",
            "description": "Query memory nodes by filter. Returns relevant memories for the current context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tag":     { "type": "string" },
                    "type": {
                        "type": "string",
                        "enum": ["concept", "pattern", "project", "decision", "error", "session", "resolution"]
                    },
                    "project": { "type": "string" },
                    "limit":   { "type": "number", "default": 10 }
                }
            }
        },
        {
            "name": "mem_search",
            "description": "Full-text search across all memory nodes. Use when you need to find specific knowledge by keyword. Results ranked by importance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search keyword or phrase" },
                    "limit": { "type": "number", "default": 20, "description": "Max results" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "mem_related",
            "description": "Find nodes related to a given node via the knowledge graph edges.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":    { "type": "string", "description": "Node ID" },
                    "depth": { "type": "number", "default": 2, "description": "Graph traversal depth" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "mem_context",
            "description": "Get relevant memory context for a project. Call at session start to load project-specific knowledge. Uses smart ranking (importance + recency + access frequency).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project slug" },
                    "limit":   { "type": "number", "default": 5 }
                }
            }
        },
        {
            "name": "mem_recall",
            "description": "Smart contextual recall. Finds the most relevant memories for your current task by combining full-text search, importance scoring, recency, access frequency, and graph connectivity. Use this PROACTIVELY when starting a task, debugging, or making architectural decisions — it surfaces past decisions, patterns, and resolutions that are relevant to your current work.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "hint":    { "type": "string", "description": "Describe what you're working on (e.g. 'authentication refactor', 'database migration', 'CI pipeline fix'). Used for semantic matching." },
                    "project": { "type": "string", "description": "Project slug to scope results" },
                    "limit":   { "type": "number", "default": 10, "description": "Max nodes to return" },
                    "include_neighbors": { "type": "boolean", "default": true, "description": "Also return 1-hop graph neighbors of top results" }
                },
                "required": ["hint"]
            }
        }
    ])
}

// ── Tool implementations ───────────────────────────────────────────────────────

fn tool_mem_add(pool: &AnyPool, args: &Value) -> Value {
    let title = match args["title"].as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return json!({ "error": "mem_add requires title, type, and body" }),
    };
    let node_type = match args["type"].as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return json!({ "error": "mem_add requires title, type, and body" }),
    };
    let body = match args["body"].as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return json!({ "error": "mem_add requires title, type, and body" }),
    };

    let tags: Vec<String> = args["tags"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let projects: Vec<String> = args["project"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| vec![s.to_string()])
        .unwrap_or_default();

    let importance = args["importance"]
        .as_f64()
        .unwrap_or_else(|| importance_for_type(&node_type))
        .clamp(0.0, 1.0);

    let id = new_uuid();
    let now = now_iso();

    let node = Node {
        frontmatter: NodeFrontmatter {
            id: id.clone(),
            node_type,
            title,
            tags,
            projects,
            agents: vec![],
            created: now.clone(),
            updated: now.clone(),
            importance,
            access_count: 0,
            accessed_at: String::new(),
        },
        body,
    };

    match runtime::block_on(write_node_dedup_pool(pool, &node, 24)) {
        Ok((existing_id, true)) => json!({ "id": existing_id, "deduplicated": true }),
        Ok((_, false)) => json!({ "id": id, "created": now }),
        Err(e) => json!({ "error": format!("write failed: {e}") }),
    }
}

fn tool_mem_query(pool: &AnyPool, args: &Value) -> Value {
    let tag = args["tag"].as_str();
    let type_filter = args["type"].as_str();
    let project = args["project"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;

    let nodes = runtime::block_on(query_nodes_pool(pool, tag, type_filter, project, limit))
        .unwrap_or_default();
    let results: Vec<Value> = nodes
        .iter()
        .map(|node| {
            let fm = &node.frontmatter;
            json!({
                "id":           fm.id,
                "title":        fm.title,
                "type":         fm.node_type,
                "tags":         fm.tags,
                "updated":      fm.updated,
                "projects":     fm.projects,
                "importance":   fm.importance,
                "access_count": fm.access_count,
                "body":         node.body.chars().take(200).collect::<String>()
            })
        })
        .collect();

    json!(results)
}

fn tool_mem_search(pool: &AnyPool, args: &Value) -> Value {
    let query = match args["query"].as_str() {
        Some(s) if !s.is_empty() => s,
        _ => return json!({ "error": "mem_search requires query" }),
    };
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    let nodes = runtime::block_on(search_nodes_pool(pool, query, limit as i64)).unwrap_or_default();

    // Touch retrieved nodes
    let ids: Vec<String> = nodes.iter().map(|n| n.frontmatter.id.clone()).collect();
    touch_nodes_pool(pool, &ids);

    let results: Vec<Value> = nodes
        .iter()
        .map(|node| {
            let snippet: String = node
                .body
                .chars()
                .take(200)
                .collect::<String>()
                .replace('\n', " ");
            json!({
                "id":         node.frontmatter.id,
                "title":      node.frontmatter.title,
                "type":       node.frontmatter.node_type,
                "importance": node.frontmatter.importance,
                "snippet":    snippet
            })
        })
        .collect();

    let count = results.len();
    let mut resp = json!({ "count": count, "results": results });
    if count == 0 {
        resp["note"] = json!("no results — check FTS5 query syntax if using special operators");
    }
    resp
}

fn tool_mem_related(pool: &AnyPool, args: &Value) -> Value {
    let id = match args["id"].as_str() {
        Some(s) if !s.is_empty() => s,
        _ => return json!({ "error": "mem_related requires id" }),
    };
    if !validate_uuid(id) {
        return json!({ "error": "invalid node id" });
    }

    let depth = args["depth"].as_u64().unwrap_or(2) as usize;
    let related_ids = runtime::block_on(related_nodes_pool(pool, id, depth));

    let results: Vec<Value> = related_ids
        .iter()
        .filter_map(|rid| {
            runtime::block_on(read_node_pool(pool, rid))
                .ok()
                .map(|node| {
                    json!({
                        "id":    node.frontmatter.id,
                        "title": node.frontmatter.title,
                        "type":  node.frontmatter.node_type
                    })
                })
        })
        .collect();

    json!(results)
}

fn tool_mem_context(pool: &AnyPool, args: &Value) -> Value {
    let project = args["project"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(5) as usize;

    // Use smart_recall for importance-weighted context
    let scored = match runtime::block_on(smart_recall_pool(pool, project, None, limit)) {
        Ok(s) => s,
        Err(e) => return json!({"error": format!("recall failed: {e}")}),
    };

    let results: Vec<Value> = scored
        .iter()
        .map(|sn| {
            json!({
                "id":         sn.node.frontmatter.id,
                "title":      sn.node.frontmatter.title,
                "type":       sn.node.frontmatter.node_type,
                "tags":       sn.node.frontmatter.tags,
                "updated":    sn.node.frontmatter.updated,
                "importance": sn.node.frontmatter.importance,
                "score":      (sn.score * 1000.0).round() / 1000.0,
                "summary":    sn.node.body.chars().take(300).collect::<String>()
            })
        })
        .collect();

    json!(results)
}

fn tool_mem_recall(pool: &AnyPool, args: &Value) -> Value {
    let hint = match args["hint"].as_str() {
        Some(s) if !s.is_empty() => s,
        _ => return json!({ "error": "mem_recall requires hint" }),
    };
    let project = args["project"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;
    let include_neighbors = args["include_neighbors"].as_bool().unwrap_or(true);

    // Phase 1: Smart recall with composite scoring
    let scored = match runtime::block_on(smart_recall_pool(pool, project, Some(hint), limit)) {
        Ok(s) => s,
        Err(e) => return json!({"error": format!("recall failed: {e}")}),
    };

    let mut results: Vec<Value> = scored
        .iter()
        .map(|sn| {
            json!({
                "id":         sn.node.frontmatter.id,
                "title":      sn.node.frontmatter.title,
                "type":       sn.node.frontmatter.node_type,
                "tags":       sn.node.frontmatter.tags,
                "importance": sn.node.frontmatter.importance,
                "score":      (sn.score * 1000.0).round() / 1000.0,
                "body":       sn.node.body.chars().take(400).collect::<String>()
            })
        })
        .collect();

    // Phase 2: Graph-augmented — include 1-hop neighbors of top results
    if include_neighbors && !scored.is_empty() {
        let seed_ids: Vec<String> = scored
            .iter()
            .map(|sn| sn.node.frontmatter.id.clone())
            .collect();
        let neighbors = runtime::block_on(graph_neighbors_pool(pool, &seed_ids));

        // Add up to 5 graph neighbors not already in results — batch-read in one query.
        let existing_ids: std::collections::HashSet<&str> = scored
            .iter()
            .map(|sn| sn.node.frontmatter.id.as_str())
            .collect();

        // Collect candidate (id, weight) pairs first so we can batch-read nodes.
        let candidates: Vec<(&str, f64)> = neighbors
            .iter()
            .filter(|(nid, _)| !existing_ids.contains(nid.as_str()))
            .take(5)
            .map(|(nid, w)| (nid.as_str(), *w))
            .collect();

        if !candidates.is_empty() {
            let candidate_ids: Vec<&str> = candidates.iter().map(|(id, _)| *id).collect();
            let weight_by_id: std::collections::HashMap<&str, f64> =
                candidates.iter().cloned().collect();

            let neighbor_results: Vec<Value> =
                runtime::block_on(read_nodes_pool(pool, &candidate_ids))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|node| {
                        let edge_weight = weight_by_id
                            .get(node.frontmatter.id.as_str())
                            .copied()
                            .unwrap_or(0.0);
                        json!({
                            "id":          node.frontmatter.id,
                            "title":       node.frontmatter.title,
                            "type":        node.frontmatter.node_type,
                            "tags":        node.frontmatter.tags,
                            "importance":  node.frontmatter.importance,
                            "score":       (edge_weight * 100.0).round() / 100.0,
                            "body":        node.body.chars().take(200).collect::<String>(),
                            "via_graph":   true,
                            "connections": (edge_weight * 100.0).round() / 100.0
                        })
                    })
                    .collect();

            results.extend(neighbor_results);
        }
    }

    json!({
        "count": results.len(),
        "hint": hint,
        "nodes": results
    })
}

fn call_tool(pool: &AnyPool, name: &str, args: &Value) -> Value {
    let result = match name {
        "mem_add" => tool_mem_add(pool, args),
        "mem_query" => tool_mem_query(pool, args),
        "mem_search" => tool_mem_search(pool, args),
        "mem_related" => tool_mem_related(pool, args),
        "mem_context" => tool_mem_context(pool, args),
        "mem_recall" => tool_mem_recall(pool, args),
        _ => json!({ "error": format!("Unknown tool: {name}") }),
    };
    json!({ "content": [{ "type": "text", "text": result.to_string() }] })
}

// ── JSON-RPC dispatch ──────────────────────────────────────────────────────────

fn send(obj: &Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{}", obj);
    let _ = out.flush();
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Validate the envelope before dispatch. A missing ID is a notification;
/// an explicitly null ID is still a request and receives a response.
fn handle_message(pool: &AnyPool, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let invalid_id = id
        .as_ref()
        .is_some_and(|id| !id.is_string() && !id.is_number() && !id.is_null());
    if !msg.is_object()
        || msg["jsonrpc"] != "2.0"
        || msg["method"].as_str().is_none()
        || invalid_id
        || msg
            .get("params")
            .is_some_and(|p| !p.is_object() && !p.is_array())
    {
        return Some(rpc_error(
            if invalid_id {
                Value::Null
            } else {
                id.unwrap_or(Value::Null)
            },
            -32600,
            "Invalid Request",
        ));
    }

    let result = match msg["method"].as_str().unwrap_or_default() {
        "notifications/initialized" => Ok(json!({})),
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "harness-mem", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let params = msg["params"].as_object();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tool_args = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            if params.is_none() || tool_name.is_empty() || !tool_args.is_object() {
                Err((-32602, "Invalid params"))
            } else {
                Ok(call_tool(pool, tool_name, &tool_args))
            }
        }
        _ => Err((-32601, "Method not found")),
    };
    // Notifications may execute tools, but must never elicit a response,
    // including errors for unknown methods or invalid method parameters.
    id.map(|id| match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => rpc_error(id, code, message),
    })
}

fn response_for_line(pool: &AnyPool, line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(message) => handle_message(pool, &message),
        Err(_) => Some(rpc_error(Value::Null, -32700, "Parse error")),
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────

/// Run the stdio MCP server loop. Reads newline-delimited JSON-RPC from stdin.
///
/// Opens the database pool once at startup so that all tool calls within the
/// session share a single connection pool — avoids re-running WAL setup and
/// schema migration on every request.
pub fn run_mcp_server() -> i32 {
    let pool = match runtime::block_on(crate::store::pool::memory_pool()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("harness-mem: failed to open database: {e}");
            return 1;
        }
    };

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(response) = response_for_line(&pool, &line) {
            send(&response);
        }
    }
    0
}
