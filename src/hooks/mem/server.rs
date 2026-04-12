/// server.rs — tiny_http based REST API server

use std::io::{self, Cursor, Read};

use tiny_http::{Header, Method, Response, Server};

use super::graph::{rebuild_graph, related_nodes};
use super::store::{
    append_edge, delete_edge_by_id, delete_node_file, graph_path, index_path, node_path,
    now_iso, parse_node, read_edges, read_index, read_node, remove_edges_for_node,
    remove_from_index, safe_node_path, upsert_index, validate_node_id, write_node, Edge, Node,
    NodeFrontmatter,
};

const WEBVIEW_HTML: &str = include_str!("webview.html");

fn cors_headers() -> Vec<Header> {
    vec![
        Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap(),
        Header::from_bytes(b"Access-Control-Allow-Methods", b"GET, POST, PUT, DELETE, OPTIONS").unwrap(),
        Header::from_bytes(b"Access-Control-Allow-Headers", b"Content-Type").unwrap(),
        Header::from_bytes(b"Content-Type", b"application/json").unwrap(),
    ]
}

fn html_headers() -> Vec<Header> {
    vec![
        Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap(),
        Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
    ]
}

fn json_response(body: &str, code: u16) -> Response<Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    let mut resp = Response::new(
        tiny_http::StatusCode(code),
        cors_headers(),
        Cursor::new(data.clone()),
        Some(data.len()),
        None,
    );
    resp
}

fn html_response(body: &str) -> Response<Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    Response::new(
        tiny_http::StatusCode(200),
        html_headers(),
        Cursor::new(data.clone()),
        Some(data.len()),
        None,
    )
}

pub fn serve(args: &[String]) -> i32 {
    let port: u16 = args.windows(2)
        .find(|w| w[0] == "--port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(7700);

    let addr = format!("127.0.0.1:{port}");
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start server: {e}");
            return 1;
        }
    };

    println!("Listening on http://localhost:{port}");

    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();

        let response: Box<dyn Fn() -> Response<Cursor<Vec<u8>>>> = match (method, url.as_str()) {
            // ── GET / ────────────────────────────────────────
            (Method::Get, "/") => Box::new(|| html_response(WEBVIEW_HTML)),

            // ── GET /api/graph ────────────────────────────────
            (Method::Get, "/api/graph") => {
                let body = std::fs::read_to_string(graph_path())
                    .unwrap_or_else(|_| "{}".to_string());
                Box::new(move || json_response(&body, 200))
            }

            // ── GET /api/nodes ────────────────────────────────
            (Method::Get, "/api/nodes") => {
                let idx = read_index();
                let body = serde_json::to_string(&idx.nodes).unwrap_or_default();
                Box::new(move || json_response(&body, 200))
            }

            // ── POST /api/nodes ───────────────────────────────
            (Method::Post, "/api/nodes") => {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let result = handle_post_node(&body);
                let (resp_body, code) = match result {
                    Ok(id) => (format!("{{\"id\":\"{id}\"}}"), 201u16),
                    Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                };
                Box::new(move || json_response(&resp_body, code))
            }

            // ── DELETE /api/edges/:id ─────────────────────────
            _ if url.starts_with("/api/edges/") && matches!(request.method(), Method::Delete) => {
                let edge_id = url.trim_start_matches("/api/edges/").to_string();
                let result = delete_edge_by_id(&edge_id);
                let (body, code) = match result {
                    Ok(_) => (format!("{{\"deleted\":\"{edge_id}\"}}"), 200u16),
                    Err(e) => (format!("{{\"error\":\"{e}\"}}"), 500),
                };
                Box::new(move || json_response(&body, code))
            }

            // ── POST /api/edges ───────────────────────────────
            (Method::Post, "/api/edges") => {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let result = handle_post_edge(&body);
                let (resp_body, code) = match result {
                    Ok(id) => (format!("{{\"edge_id\":\"{id}\"}}"), 201u16),
                    Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                };
                Box::new(move || json_response(&resp_body, code))
            }

            // ── GET /api/nodes/:id ────────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Get) => {
                let id = url.trim_start_matches("/api/nodes/").to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    Box::new(move || json_response(&body, 400))
                } else {
                    let result = read_node(&id);
                    let (body, code) = match result {
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
                                "body": node.body
                            });
                            (v.to_string(), 200u16)
                        }
                        Err(e) => (format!("{{\"error\":\"{e}\"}}"), 404),
                    };
                    Box::new(move || json_response(&body, code))
                }
            }

            // ── PUT /api/nodes/:id ────────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Put) => {
                let id = url.trim_start_matches("/api/nodes/").to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    Box::new(move || json_response(&body, 400))
                } else {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
                    let result = handle_put_node(&id, &body);
                    let (resp_body, code) = match result {
                        Ok(_) => (format!("{{\"id\":\"{id}\"}}"), 200u16),
                        Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                    };
                    Box::new(move || json_response(&resp_body, code))
                }
            }

            // ── DELETE /api/nodes/:id ─────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Delete) => {
                let id = url.trim_start_matches("/api/nodes/").to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    Box::new(move || json_response(&body, 400))
                } else {
                    let _ = delete_node_file(&id);
                    let _ = remove_edges_for_node(&id);
                    let _ = remove_from_index(&id);
                    let body = format!("{{\"deleted\":\"{id}\"}}");
                    Box::new(move || json_response(&body, 200))
                }
            }

            // ── GET /api/search?q=... ─────────────────────────
            _ if url.starts_with("/api/search") && matches!(request.method(), Method::Get) => {
                let q = url
                    .split('?')
                    .nth(1)
                    .and_then(|qs| {
                        qs.split('&').find(|p| p.starts_with("q=")).map(|p| {
                            percent_decode(p.trim_start_matches("q="))
                        })
                    })
                    .unwrap_or_default();
                let results = do_search(&q);
                let body = serde_json::to_string(&results).unwrap_or_default();
                Box::new(move || json_response(&body, 200))
            }

            // ── OPTIONS (CORS preflight) ───────────────────────
            (Method::Options, _) => {
                Box::new(|| json_response("{}", 204))
            }

            // ── 404 ───────────────────────────────────────────
            _ => {
                let body = "{\"error\":\"not found\"}".to_string();
                Box::new(move || json_response(&body, 404))
            }
        };

        let _ = request.respond(response());
    }

    0
}

// ── Helpers ───────────────────────────────────────────

fn handle_post_node(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();

    let tags: Vec<String> = v["tags"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let projects: Vec<String> = v["projects"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let agents: Vec<String> = v["agents"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let node = Node {
        frontmatter: NodeFrontmatter {
            id: id.clone(),
            node_type: v["type"].as_str().unwrap_or("concept").to_string(),
            title: v["title"].as_str().unwrap_or("Untitled").to_string(),
            tags,
            projects,
            agents,
            created: now.clone(),
            updated: now,
        },
        body: v["body"].as_str().unwrap_or("").to_string(),
    };

    write_node(&node).map_err(|e| e.to_string())?;
    let _ = upsert_index(&node);
    Ok(id)
}

fn handle_put_node(id: &str, body: &str) -> Result<(), String> {
    let mut node = read_node(id).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;

    if let Some(t) = v["title"].as_str() {
        node.frontmatter.title = t.to_string();
    }
    if let Some(t) = v["type"].as_str() {
        node.frontmatter.node_type = t.to_string();
    }
    if let Some(b) = v["body"].as_str() {
        node.body = b.to_string();
    }
    if let Some(tags) = v["tags"].as_array() {
        node.frontmatter.tags = tags.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
    }
    node.frontmatter.updated = now_iso();

    write_node(&node).map_err(|e| e.to_string())?;
    let _ = upsert_index(&node);
    Ok(())
}

fn handle_post_edge(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let edge_id = uuid::Uuid::new_v4().to_string();
    let edge = Edge {
        id: edge_id.clone(),
        source: v["source"].as_str().unwrap_or("").to_string(),
        target: v["target"].as_str().unwrap_or("").to_string(),
        relation: v["relation"].as_str().unwrap_or("related").to_string(),
        weight: v["weight"].as_f64().unwrap_or(1.0),
        ts: now_iso(),
    };
    append_edge(&edge).map_err(|e| e.to_string())?;
    Ok(edge_id)
}

fn do_search(query: &str) -> Vec<String> {
    let dir = super::store::nodes_dir();
    let output = std::process::Command::new("rg")
        .arg("-l")
        .arg("--")
        .arg(query)
        .arg(dir.to_str().unwrap_or("."))
        .output();

    match output {
        Ok(o) if !o.stdout.is_empty() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect()
        }
        _ => {
            let o = std::process::Command::new("grep")
                .arg("-rl")
                .arg("--")
                .arg(query)
                .arg(dir.to_str().unwrap_or("."))
                .output()
                .unwrap_or_else(|_| std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: vec![],
                    stderr: vec![],
                });
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect()
        }
    }
}

fn percent_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    let bytes: Vec<u8> = s.bytes().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                result.push(b as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(' ');
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}
