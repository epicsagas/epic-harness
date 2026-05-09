//! serve.rs — Web dashboard for orchestration state and memory graph
//! Serves a single-page Svelte dashboard at http://localhost:{port}

use tiny_http::{Response, Server, Method, Header};

use crate::hooks::common;
use crate::orchestrate::state as orch;
use crate::mem::{graph, store};

const DEFAULT_PORT: u16 = 7700;

// HTML is embedded at compile time from assets/dashboard.html
static DASHBOARD_HTML: &str = include_str!("../assets/dashboard.html");

pub fn run_serve(port: Option<u16>) -> i32 {
    let port = port.unwrap_or(DEFAULT_PORT);
    let addr = format!("127.0.0.1:{port}");
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to start server on {addr}: {e}");
            return 1;
        }
    };
    eprintln!("Combined dashboard: http://localhost:{port}");
    eprintln!("Press Ctrl+C to stop.");

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();
        
        let response = match (method, url.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => Response::from_string(DASHBOARD_HTML).with_header(
                Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
            ),
            
            // ── Orchestration API ───────────────────────────
            (Method::Get, "/api/run") => {
                let harness_dir = common::harness_dir();
                let body = match orch::read_run(&harness_dir) {
                    Some(run) => serde_json::to_string(&run).unwrap_or_else(|_| "{}".into()),
                    None => "{}".into(),
                };
                json_response(&body)
            }
            
            (Method::Get, "/api/events") => {
                let harness_dir = common::harness_dir();
                let run = orch::read_run(&harness_dir);
                let data = serde_json::to_string(&run).unwrap_or_else(|_| "null".into());
                let sse_body = format!("data: {data}\n\n");
                Response::from_string(sse_body)
                    .with_header(Header::from_bytes(b"Content-Type", b"text/event-stream").unwrap())
                    .with_header(Header::from_bytes(b"Cache-Control", b"no-cache").unwrap())
            }

            // ── Memory API (Graph/Stats) ─────────────────────
            (Method::Get, "/api/graph") => {
                let body = match graph::rebuild_graph_json() {
                    Ok(json) => json,
                    Err(_) => "{}".into(),
                };
                json_response(&body)
            }
            
            (Method::Get, "/api/stats") => {
                let body = match graph::compute_stats() {
                    Ok(v) => v.to_string(),
                    Err(_) => "{}".into(),
                };
                json_response(&body)
            }

            // ── Memory API (Nodes) ───────────────────────────
            (Method::Get, "/api/nodes") => {
                let nodes = store::read_all_nodes_conn(&store::open_db().unwrap()).unwrap_or_default();
                let results: Vec<serde_json::Value> = nodes.into_iter().map(|n| {
                    serde_json::json!({
                        "id": n.frontmatter.id,
                        "type": n.frontmatter.node_type,
                        "title": n.frontmatter.title,
                        "tags": n.frontmatter.tags,
                        "projects": n.frontmatter.projects,
                        "updated": n.frontmatter.updated
                    })
                }).collect();
                json_response(&serde_json::to_string(&results).unwrap_or_else(|_| "[]".into()))
            }
            
            (Method::Get, url) if url.starts_with("/api/nodes/") => {
                let id = url.trim_start_matches("/api/nodes/").split('?').next().unwrap_or("");
                let body = match store::read_node(id) {
                    Ok(node) => {
                        let v = serde_json::json!({
                            "id": node.frontmatter.id,
                            "type": node.frontmatter.node_type,
                            "title": node.frontmatter.title,
                            "tags": node.frontmatter.tags,
                            "projects": node.frontmatter.projects,
                            "agents": node.frontmatter.agents,
                            "created": node.frontmatter.created,
                            "updated": node.frontmatter.updated,
                            "importance": node.frontmatter.importance,
                            "access_count": node.frontmatter.access_count,
                            "body": node.body
                        });
                        v.to_string()
                    },
                    Err(_) => "{\"error\":\"not found\"}".into(),
                };
                json_response(&body)
            }

            // ── Memory API (Search) ──────────────────────────
            (Method::Get, url) if url.starts_with("/api/search") => {
                let query = url.split("q=").nth(1).unwrap_or("").split('&').next().unwrap_or("");
                let decoded = percent_decode(query);
                let nodes = store::search_nodes(&decoded, 20);
                let results: Vec<serde_json::Value> = nodes.into_iter().map(|n| {
                    serde_json::json!({
                        "id": n.frontmatter.id,
                        "title": n.frontmatter.title,
                        "type": n.frontmatter.node_type,
                        "snippet": n.body.chars().take(160).collect::<String>()
                    })
                }).collect();
                let body = serde_json::to_string(&results).unwrap_or_else(|_| "[]".into());
                json_response(&body)
            }

            // ── Agents API ───────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/agents/") && url.ends_with("/status") => {
                let agent_id = url
                    .trim_start_matches("/api/agents/")
                    .trim_end_matches("/status");
                let harness_dir = common::harness_dir();
                if !orch::validate_agent_id(agent_id) {
                    json_response("{\"error\":\"invalid agent id\"}").with_status_code(400)
                } else {
                    let body = match orch::read_agent_status(&harness_dir, agent_id) {
                        Some(s) => serde_json::to_string(&s).unwrap_or_else(|_| "{}".into()),
                        None => "{}".into(),
                    };
                    json_response(&body)
                }
            }

            // ── CORS & Other ────────────────────────────────
            (Method::Options, _) => {
                Response::from_string("{}").with_status_code(204)
                    .with_header(Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap())
                    .with_header(Header::from_bytes(b"Access-Control-Allow-Methods", b"GET, POST, PUT, DELETE, OPTIONS").unwrap())
                    .with_header(Header::from_bytes(b"Access-Control-Allow-Headers", b"Content-Type").unwrap())
            }
            
            _ => Response::from_string("Not Found").with_status_code(404),
        };
        let _ = request.respond(response);
    }
    0
}

fn json_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(Header::from_bytes(b"Content-Type", b"application/json").unwrap())
        .with_header(Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap())
}

fn percent_decode(s: &str) -> String {
    let bytes: Vec<u8> = s.bytes().collect();
    let mut decoded: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                decoded.push(b);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            decoded.push(b' ');
        } else {
            decoded.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_html_not_empty() {
        assert!(!DASHBOARD_HTML.is_empty());
        assert!(DASHBOARD_HTML.contains("<html"));
    }

    #[test]
    fn default_port_is_7700() {
        assert_eq!(DEFAULT_PORT, 7700);
    }
}
