//! mcp.rs — Stdio JSON-RPC 2.0 MCP server for the unified memory system
//!
//! Memory features require the `epic-harness` binary — no Node.js runtime needed.
//! Usage: `epic-harness mem mcp`
//!
//! Implements MCP protocol version 2024-11-05 over stdin/stdout.
//! Tools: mem_add, mem_query, mem_search, mem_related, mem_context

use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use super::graph::related_nodes;
use super::store::{
    list_node_ids, nodes_dir, now_iso, read_index, read_node, upsert_index, validate_node_id,
    write_node, Node, NodeFrontmatter,
};

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
                        "enum": ["concept", "pattern", "project", "decision", "error"],
                        "description": "Node type"
                    },
                    "body":    { "type": "string", "description": "Markdown content (the actual knowledge)" },
                    "tags":    { "type": "array", "items": { "type": "string" }, "description": "Tags for filtering" },
                    "project": { "type": "string", "description": "Project slug (optional)" }
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
                        "enum": ["concept", "pattern", "project", "decision", "error"]
                    },
                    "project": { "type": "string" },
                    "limit":   { "type": "number", "default": 10 }
                }
            }
        },
        {
            "name": "mem_search",
            "description": "Full-text search across all memory nodes. Use when you need to find specific knowledge by keyword.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search keyword or phrase" }
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
            "description": "Get relevant memory context for a project. Call at session start to load project-specific knowledge.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project slug" },
                    "limit":   { "type": "number", "default": 5 }
                }
            }
        }
    ])
}

// ── Tool implementations ───────────────────────────────────────────────────────

fn tool_mem_add(args: &Value) -> Value {
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
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let projects: Vec<String> = args["project"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| vec![s.to_string()])
        .unwrap_or_default();

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
        },
        body,
    };

    if let Err(e) = write_node(&node) {
        return json!({ "error": format!("write failed: {e}") });
    }
    if let Err(e) = upsert_index(&node) {
        return json!({ "error": format!("index update failed: {e}") });
    }

    json!({ "id": id, "created": now })
}

fn tool_mem_query(args: &Value) -> Value {
    let tag = args["tag"].as_str();
    let type_filter = args["type"].as_str();
    let project = args["project"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;

    let idx = read_index();
    let nd = nodes_dir();

    // Build candidate id set from index filters
    let mut candidates: Option<std::collections::HashSet<String>> = None;

    if let Some(t) = tag {
        if let Some(ids) = idx.by_tag.get(t) {
            candidates = Some(ids.iter().cloned().collect());
        }
    }
    if let Some(ty) = type_filter {
        if let Some(ids) = idx.by_type.get(ty) {
            let type_set: std::collections::HashSet<String> = ids.iter().cloned().collect();
            candidates = Some(match candidates {
                Some(c) => c.intersection(&type_set).cloned().collect(),
                None => type_set,
            });
        }
    }
    if let Some(p) = project {
        if let Some(ids) = idx.by_project.get(p) {
            let proj_set: std::collections::HashSet<String> = ids.iter().cloned().collect();
            candidates = Some(match candidates {
                Some(c) => c.intersection(&proj_set).cloned().collect(),
                None => proj_set,
            });
        }
    }

    let all_ids: Vec<String> = match candidates {
        Some(set) => set.into_iter().collect(),
        None => idx.nodes.iter().map(|n| n.id.clone()).collect(),
    };

    if !nd.exists() {
        return json!([]);
    }

    let mut results = vec![];
    for id in all_ids.iter().take(limit) {
        if let Ok(node) = read_node(id) {
            let fm = &node.frontmatter;
            results.push(json!({
                "id":      fm.id,
                "title":   fm.title,
                "type":    fm.node_type,
                "tags":    fm.tags,
                "updated": fm.updated,
                "projects": fm.projects,
                "body":    node.body.chars().take(200).collect::<String>()
            }));
        }
    }

    json!(results)
}

fn tool_mem_search(args: &Value) -> Value {
    let query = match args["query"].as_str() {
        Some(s) if !s.is_empty() => s.to_lowercase(),
        _ => return json!({ "error": "mem_search requires query" }),
    };

    let nd = nodes_dir();
    if !nd.exists() {
        return json!([]);
    }

    let ids = match list_node_ids() {
        Ok(v) => v,
        Err(_) => return json!([]),
    };

    let mut results = vec![];
    for id in &ids {
        if results.len() >= 10 {
            break;
        }
        let Ok(node) = read_node(id) else { continue };
        let content = format!("{} {}", node.frontmatter.title, node.body).to_lowercase();
        if !content.contains(&query) {
            continue;
        }
        // Find snippet
        let idx = content.find(&query).unwrap_or(0);
        let raw = format!("{} {}", node.frontmatter.title, node.body);
        let start = idx.saturating_sub(40);
        let snippet: String = raw
            .chars()
            .skip(start)
            .take(160)
            .collect::<String>()
            .replace('\n', " ");

        results.push(json!({
            "id":      node.frontmatter.id,
            "title":   node.frontmatter.title,
            "type":    node.frontmatter.node_type,
            "snippet": snippet
        }));
    }

    json!(results)
}

fn tool_mem_related(args: &Value) -> Value {
    let id = match args["id"].as_str() {
        Some(s) if !s.is_empty() => s,
        _ => return json!({ "error": "mem_related requires id" }),
    };
    if !validate_node_id(id) {
        return json!({ "error": "invalid node id" });
    }

    let depth = args["depth"].as_u64().unwrap_or(2) as usize;
    let related_ids = related_nodes(id, depth);

    let results: Vec<Value> = related_ids
        .iter()
        .filter_map(|rid| {
            read_node(rid).ok().map(|node| {
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

fn tool_mem_context(args: &Value) -> Value {
    let project = args["project"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(5) as usize;

    let idx = read_index();

    let ids: Vec<String> = if let Some(p) = project {
        idx.by_project
            .get(p)
            .cloned()
            .unwrap_or_default()
    } else {
        idx.nodes.iter().map(|n| n.id.clone()).collect()
    };

    let mut nodes: Vec<_> = ids
        .iter()
        .filter_map(|id| read_node(id).ok())
        .collect();

    // Sort by updated desc
    nodes.sort_by(|a, b| b.frontmatter.updated.cmp(&a.frontmatter.updated));

    let results: Vec<Value> = nodes
        .iter()
        .take(limit)
        .map(|node| {
            json!({
                "id":       node.frontmatter.id,
                "title":    node.frontmatter.title,
                "type":     node.frontmatter.node_type,
                "tags":     node.frontmatter.tags,
                "updated":  node.frontmatter.updated,
                "summary":  node.body.chars().take(300).collect::<String>()
            })
        })
        .collect();

    json!(results)
}

fn call_tool(name: &str, args: &Value) -> Value {
    let result = match name {
        "mem_add"     => tool_mem_add(args),
        "mem_query"   => tool_mem_query(args),
        "mem_search"  => tool_mem_search(args),
        "mem_related" => tool_mem_related(args),
        "mem_context" => tool_mem_context(args),
        _ => json!({ "error": format!("Unknown tool: {name}") }),
    };
    json!({ "content": [{ "type": "text", "text": result.to_string() }] })
}

// ── JSON-RPC dispatch ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
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

fn handle_message(msg: &RpcRequest) {
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
        "notifications/initialized" => {
            // client notification, no response
        }
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

            let result = call_tool(tool_name, &tool_args);
            let resp = json!({
                "jsonrpc": "2.0",
                "id": msg.id,
                "result": result
            });
            send(&resp);
        }
        _ => {
            if msg.id.is_some() {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": msg.id,
                    "error": { "code": -32601, "message": "Method not found" }
                });
                send(&resp);
            }
        }
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────

/// Run the stdio MCP server loop. Reads newline-delimited JSON-RPC from stdin.
pub fn run_mcp_server() -> i32 {
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
        match serde_json::from_str::<RpcRequest>(&line) {
            Ok(msg) => handle_message(&msg),
            Err(_) => {
                // Ignore parse errors silently (per MCP spec)
            }
        }
    }
    0
}

// ── UUID generation (std-only, no uuid crate) ─────────────────────────────────

fn new_uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Seed from time + pid for uniqueness
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let pid = std::process::id();

    // Mix entropy sources using a simple LCG
    let mut state = (nanos as u64).wrapping_add(pid as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let r = |s: &mut u64| -> u8 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*s >> 33) as u8
    };

    let mut b = [0u8; 16];
    for byte in &mut b {
        *byte = r(&mut state);
    }
    // Set version 4 and variant bits
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}
