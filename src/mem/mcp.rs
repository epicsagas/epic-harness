//! mcp.rs — Stdio JSON-RPC 2.0 MCP server for the unified memory system
//!
//! Memory features require the `epic-harness` binary — no Node.js runtime needed.
//! Usage: `epic-harness mem mcp`
//!
//! Implements MCP protocol version 2024-11-05 over stdin/stdout.
//! Tools: mem_add, mem_query, mem_search, mem_related, mem_context

use serde::Deserialize;
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
    let project = args["project"].as_str().filter(|p| !p.is_empty());
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

const NEIGHBOR_TAKE: usize = 5;
const NEIGHBOR_SCAN_CHUNK: usize = 32;
// ponytail: bounded scan; a hub node with more than 256 neighbors may hide in-scope ones past the bound.
const NEIGHBOR_SCAN_MAX: usize = 256;

/// Stale nodes never come back as neighbors; with a project, only its own nodes do.
fn neighbor_in_scope(node: &Node, project: Option<&str>) -> bool {
    !node.frontmatter.tags.iter().any(|t| t == "stale")
        && project.is_none_or(|p| node.frontmatter.projects.iter().any(|x| x == p))
}

fn tool_mem_recall(pool: &AnyPool, args: &Value) -> Value {
    let hint = match args["hint"].as_str() {
        Some(s) if !s.is_empty() => s,
        _ => return json!({ "error": "mem_recall requires hint" }),
    };
    let project = args["project"].as_str().filter(|p| !p.is_empty());
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

        // Add up to NEIGHBOR_TAKE in-scope neighbors not already in results.
        let existing_ids: std::collections::HashSet<&str> = scored
            .iter()
            .map(|sn| sn.node.frontmatter.id.as_str())
            .collect();

        let candidates: Vec<(&str, f64)> = neighbors
            .iter()
            .filter(|(nid, _)| !existing_ids.contains(nid.as_str()))
            .take(NEIGHBOR_SCAN_MAX)
            .map(|(nid, w)| (nid.as_str(), *w))
            .collect();

        // Scan in chunks, in discovery order, keeping only nodes in scope.
        let mut selected: Vec<(Node, f64)> = Vec::new();
        'scan: for chunk in candidates.chunks(NEIGHBOR_SCAN_CHUNK) {
            let chunk_ids: Vec<&str> = chunk.iter().map(|(id, _)| *id).collect();
            let mut nodes: std::collections::HashMap<String, Node> =
                runtime::block_on(read_nodes_pool(pool, &chunk_ids))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|n| (n.frontmatter.id.clone(), n))
                    .collect();
            for (id, weight) in chunk {
                if let Some(node) = nodes.remove(*id).filter(|n| neighbor_in_scope(n, project)) {
                    selected.push((node, *weight));
                    if selected.len() == NEIGHBOR_TAKE {
                        break 'scan;
                    }
                }
            }
        }

        results.extend(selected.into_iter().map(|(node, edge_weight)| {
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
        }));
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

#[derive(Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn send(obj: &Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{}", obj);
    let _ = out.flush();
}

fn send_error(id: Value, code: i64, message: &str) {
    send(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }));
}

fn handle_message(pool: &AnyPool, msg: &RpcRequest) {
    // Notifications (no id, or a null id which MCP forbids) never get a response.
    if msg.id.is_none() {
        return;
    }
    match msg.method.as_str() {
        "initialize" => {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": msg.id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "harness-mem", "version": env!("CARGO_PKG_VERSION") }
                }
            });
            send(&resp);
        }
        "ping" => send(&json!({ "jsonrpc": "2.0", "id": msg.id, "result": {} })),
        "tools/list" => {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": msg.id,
                "result": { "tools": tool_definitions() }
            });
            send(&resp);
        }
        "tools/call" => {
            let params = msg.params.as_ref().and_then(|p| p.as_object());
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tool_args = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            let result = call_tool(pool, tool_name, &tool_args);
            let resp = json!({
                "jsonrpc": "2.0",
                "id": msg.id,
                "result": result
            });
            send(&resp);
        }
        _ => send_error(
            msg.id.clone().unwrap_or(Value::Null),
            -32601,
            "Method not found",
        ),
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
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            send_error(Value::Null, -32700, "Parse error");
            continue;
        };
        let Some(obj) = value.as_object() else {
            send_error(Value::Null, -32600, "Invalid Request");
            continue;
        };
        let has_id = obj.contains_key("id");
        let err_id = match obj.get("id") {
            Some(id) if id.is_string() || id.is_number() => id.clone(),
            _ => Value::Null,
        };
        match serde_json::from_value::<RpcRequest>(value) {
            Ok(msg) if msg.jsonrpc == "2.0" => handle_message(&pool, &msg),
            _ if has_id => send_error(err_id, -32600, "Invalid Request"),
            _ => {}
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::store::{Edge, append_edge_pool, init_schema_pool, write_node_pool};

    fn setup() -> AnyPool {
        runtime::block_on(async {
            let pool = crate::store::pool::test_memory_pool().await;
            init_schema_pool(&pool).await.unwrap();
            pool
        })
    }

    fn add_node(pool: &AnyPool, id: &str, title: &str, project: &str, tags: &[&str]) {
        let ts = "2024-01-01T00:00:00Z".to_string();
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: id.to_string(),
                node_type: "concept".to_string(),
                title: title.to_string(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                projects: vec![project.to_string()],
                created: ts.clone(),
                updated: ts,
                ..Default::default()
            },
            body: format!("body of {id}"),
        };
        runtime::block_on(write_node_pool(pool, &node)).unwrap();
    }

    fn link(pool: &AnyPool, source: &str, target: &str) {
        let edge = Edge {
            id: new_uuid(),
            source: source.to_string(),
            target: target.to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            ts: now_iso(),
        };
        runtime::block_on(append_edge_pool(pool, &edge)).unwrap();
    }

    /// Ids of the graph neighbors returned by `mem_recall` for the hint "needle".
    fn neighbor_ids(pool: &AnyPool, project: Option<&str>) -> Vec<String> {
        let mut args = json!({"hint": "needle"});
        if let Some(p) = project {
            args["project"] = json!(p);
        }
        let out = tool_mem_recall(pool, &args);
        out["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["via_graph"] == json!(true))
            .map(|n| n["id"].as_str().unwrap().to_string())
            .collect()
    }

    /// Seed "a" (alpha, matches the hint) linked to alpha "c" and beta "b".
    fn seed_basic(pool: &AnyPool) {
        add_node(pool, "a", "needle", "alpha", &[]);
        add_node(pool, "b", "other beta", "beta", &[]);
        add_node(pool, "c", "other alpha", "alpha", &[]);
        link(pool, "a", "b");
        link(pool, "a", "c");
    }

    #[test]
    fn f_mcp_neighbors_scoped() {
        let pool = setup();
        seed_basic(&pool);
        assert_eq!(neighbor_ids(&pool, Some("alpha")), vec!["c"]);
    }

    #[test]
    fn f_mcp_neighbors_unscoped_unchanged() {
        let pool = setup();
        seed_basic(&pool);
        assert_eq!(neighbor_ids(&pool, None), vec!["b", "c"]);
        // an empty project is unscoped as well
        assert_eq!(neighbor_ids(&pool, Some("")), vec!["b", "c"]);
    }

    #[test]
    fn f_mcp_neighbors_skip_stale() {
        let pool = setup();
        seed_basic(&pool);
        add_node(&pool, "d", "other alpha stale", "alpha", &["stale"]);
        link(&pool, "a", "d");
        assert_eq!(neighbor_ids(&pool, Some("alpha")), vec!["c"]);
        assert_eq!(neighbor_ids(&pool, None), vec!["b", "c"]);
    }

    #[test]
    fn f_mcp_neighbors_skip_out_of_scope_prefix() {
        let pool = setup();
        add_node(&pool, "a", "needle", "alpha", &[]);
        for i in 0..13 {
            let project = if i < 10 { "beta" } else { "alpha" };
            let id = format!("n-{i:02}");
            add_node(&pool, &id, "other", project, &[]);
            link(&pool, "a", &id);
        }
        assert_eq!(
            neighbor_ids(&pool, Some("alpha")),
            vec!["n-10", "n-11", "n-12"]
        );
    }

    #[test]
    fn f_mcp_neighbors_scan_bound() {
        let pool = setup();
        add_node(&pool, "a", "needle", "alpha", &[]);
        for i in 0..301 {
            let project = if i < 300 { "beta" } else { "alpha" };
            let id = format!("n-{i:03}");
            add_node(&pool, &id, "other", project, &[]);
            link(&pool, "a", &id);
        }
        // Documented ceiling: only the first NEIGHBOR_SCAN_MAX candidates are scanned.
        assert!(neighbor_ids(&pool, Some("alpha")).is_empty());
    }
}
