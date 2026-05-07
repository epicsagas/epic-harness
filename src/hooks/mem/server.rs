//! server.rs — tiny_http based REST API server

use std::cell::RefCell;
use std::io::{Cursor, Read as _};
use std::rc::Rc;

use rusqlite::Connection;
use tiny_http::{Header, Method, Response, Server};

use super::graph::{compute_stats, rebuild_graph_json_conn};
use super::store::{
    Edge, Node, NodeFrontmatter, append_edge, delete_edge_by_id, delete_node_file,
    importance_for_type, now_iso, open_db, read_index, read_node_conn, remove_edges_for_node,
    remove_from_index, search_nodes_conn, validate_node_id, write_node_conn,
};

const WEBVIEW_HTML: &str = include_str!("webview.html");
const D3_JS: &str = include_str!("d3.min.js");

const VALID_NODE_TYPES: &[&str] = &[
    "decision",
    "resolution",
    "concept",
    "project",
    "error",
    "session",
    "pattern",
    "instinct",
    "psychographic",
];

const MAX_BODY_CHARS: usize = 65_536;
const MAX_TITLE_CHARS: usize = 512;
const MAX_ARRAY_ITEMS: usize = 50;
const MAX_RELATION_CHARS: usize = 64;
const WEIGHT_MIN: f64 = 0.0;
const WEIGHT_MAX: f64 = 100.0;

fn cors_headers(port: u16) -> Vec<Header> {
    let origin = format!("http://localhost:{port}");
    vec![
        Header::from_bytes(b"Access-Control-Allow-Origin", origin.as_bytes()).unwrap(),
        Header::from_bytes(
            b"Access-Control-Allow-Methods",
            b"GET, POST, PUT, DELETE, OPTIONS",
        )
        .unwrap(),
        Header::from_bytes(b"Access-Control-Allow-Headers", b"Content-Type").unwrap(),
        Header::from_bytes(b"Content-Type", b"application/json").unwrap(),
    ]
}

fn html_headers(port: u16) -> Vec<Header> {
    let origin = format!("http://localhost:{port}");
    vec![
        Header::from_bytes(b"Access-Control-Allow-Origin", origin.as_bytes()).unwrap(),
        Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
    ]
}

fn json_response(body: &str, code: u16, port: u16) -> Response<Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    let len = data.len();
    Response::new(
        tiny_http::StatusCode(code),
        cors_headers(port),
        Cursor::new(data),
        Some(len),
        None,
    )
}

fn html_response(body: &str, port: u16) -> Response<Cursor<Vec<u8>>> {
    let data = body.as_bytes().to_vec();
    let len = data.len();
    Response::new(
        tiny_http::StatusCode(200),
        html_headers(port),
        Cursor::new(data),
        Some(len),
        None,
    )
}

pub fn serve(args: &[String]) -> i32 {
    let port: u16 = args
        .windows(2)
        .find(|w| w[0] == "--port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(7700);

    let addr = format!("127.0.0.1:{port}");

    // Open a single long-lived DB connection for the entire server lifetime.
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to open memory DB: {e}");
            return 1;
        }
    };
    let conn = Rc::new(RefCell::new(conn));

    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start server: {e}");
            return 1;
        }
    };

    eprintln!("[harness] Web UI listening on http://localhost:{port}");

    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();

        let response: Box<dyn Fn() -> Response<Cursor<Vec<u8>>>> = match (method, url.as_str()) {
            // ── GET / ────────────────────────────────────────
            (Method::Get, "/") => {
                let p = port;
                Box::new(move || html_response(WEBVIEW_HTML, p))
            }

            // ── GET /d3.js ───────────────────────────────────
            (Method::Get, "/d3.js") => Box::new(move || {
                let data = D3_JS.as_bytes().to_vec();
                Response::new(
                    tiny_http::StatusCode(200),
                    vec![
                        Header::from_bytes(
                            b"Content-Type",
                            b"application/javascript; charset=utf-8",
                        )
                        .unwrap(),
                        Header::from_bytes(b"Cache-Control", b"public, max-age=86400").unwrap(),
                    ],
                    Cursor::new(data),
                    Some(D3_JS.len()),
                    None,
                )
            }),

            // ── GET /api/stats ───────────────────────────────
            (Method::Get, "/api/stats") => {
                let body = compute_stats()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| "{\"error\":\"stats unavailable\"}".to_string());
                let p = port;
                Box::new(move || json_response(&body, 200, p))
            }

            // ── GET /api/graph ────────────────────────────────
            (Method::Get, "/api/graph") => {
                let db = Rc::clone(&conn);
                let body =
                    rebuild_graph_json_conn(&db.borrow()).unwrap_or_else(|_| "{}".to_string());
                let p = port;
                Box::new(move || json_response(&body, 200, p))
            }

            // ── GET /api/graph/centrality ────────────────────
            _ if url.starts_with("/api/graph/centrality")
                && matches!(request.method(), Method::Get) =>
            {
                let limit: usize = url
                    .split('?')
                    .nth(1)
                    .and_then(|qs| {
                        qs.split('&')
                            .find_map(|p| p.strip_prefix("limit=").and_then(|v| v.parse().ok()))
                    })
                    .unwrap_or(20);
                let db = Rc::clone(&conn);
                let centrality = compute_centrality(&db.borrow(), limit);
                let body = serde_json::to_string(&centrality).unwrap_or_default();
                let p = port;
                Box::new(move || json_response(&body, 200, p))
            }

            // ── GET /api/nodes ────────────────────────────────
            (Method::Get, "/api/nodes") => {
                let idx = read_index();
                let body = serde_json::to_string(&idx.nodes).unwrap_or_default();
                let p = port;
                Box::new(move || json_response(&body, 200, p))
            }

            // ── POST /api/nodes ───────────────────────────────
            (Method::Post, "/api/nodes") => {
                let mut body = String::new();
                let _ = request.as_reader().take(1 << 20).read_to_string(&mut body);
                let db = Rc::clone(&conn);
                let result = handle_post_node_conn(&body, &db.borrow());
                let (resp_body, code) = match result {
                    Ok(id) => (format!("{{\"id\":\"{id}\"}}"), 201u16),
                    Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                };
                let p = port;
                Box::new(move || json_response(&resp_body, code, p))
            }

            // ── DELETE /api/edges/:id ─────────────────────────
            _ if url.starts_with("/api/edges/") && matches!(request.method(), Method::Delete) => {
                let edge_id = url
                    .trim_start_matches("/api/edges/")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !validate_node_id(&edge_id) {
                    let body = "{\"error\":\"invalid edge id\"}".to_string();
                    let p = port;
                    Box::new(move || json_response(&body, 400, p))
                } else {
                    let result = delete_edge_by_id(&edge_id);
                    let (body, code) = match result {
                        Ok(_) => (format!("{{\"deleted\":\"{edge_id}\"}}"), 200u16),
                        Err(e) => (format!("{{\"error\":\"{e}\"}}"), 500),
                    };
                    let p = port;
                    Box::new(move || json_response(&body, code, p))
                }
            }

            // ── POST /api/edges ───────────────────────────────
            (Method::Post, "/api/edges") => {
                let mut body = String::new();
                let _ = request.as_reader().take(1 << 20).read_to_string(&mut body);
                let result = handle_post_edge(&body);
                let (resp_body, code) = match result {
                    Ok(id) => (format!("{{\"edge_id\":\"{id}\"}}"), 201u16),
                    Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                };
                let p = port;
                Box::new(move || json_response(&resp_body, code, p))
            }

            // ── GET /api/nodes/:id ────────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Get) => {
                let id = url
                    .trim_start_matches("/api/nodes/")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    let p = port;
                    Box::new(move || json_response(&body, 400, p))
                } else {
                    let db = Rc::clone(&conn);
                    let result = read_node_conn(&db.borrow(), &id);
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
                                "importance": node.frontmatter.importance,
                                "access_count": node.frontmatter.access_count,
                                "body": node.body
                            });
                            (v.to_string(), 200u16)
                        }
                        Err(e) => (format!("{{\"error\":\"{e}\"}}"), 404),
                    };
                    let p = port;
                    Box::new(move || json_response(&body, code, p))
                }
            }

            // ── PUT /api/nodes/:id ────────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Put) => {
                let id = url
                    .trim_start_matches("/api/nodes/")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    let p = port;
                    Box::new(move || json_response(&body, 400, p))
                } else {
                    let mut body = String::new();
                    let _ = request.as_reader().take(1 << 20).read_to_string(&mut body);
                    let db = Rc::clone(&conn);
                    let result = handle_put_node_conn(&id, &body, &db.borrow());
                    let (resp_body, code) = match result {
                        Ok(_) => (format!("{{\"id\":\"{id}\"}}"), 200u16),
                        Err(e) => (format!("{{\"error\":\"{e}\"}}"), 400),
                    };
                    let p = port;
                    Box::new(move || json_response(&resp_body, code, p))
                }
            }

            // ── DELETE /api/nodes/:id ─────────────────────────
            _ if url.starts_with("/api/nodes/") && matches!(request.method(), Method::Delete) => {
                let id = url
                    .trim_start_matches("/api/nodes/")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !validate_node_id(&id) {
                    let body = "{\"error\":\"invalid node id\"}".to_string();
                    let p = port;
                    Box::new(move || json_response(&body, 400, p))
                } else {
                    let _ = delete_node_file(&id);
                    let _ = remove_edges_for_node(&id);
                    let _ = remove_from_index(&id);
                    let body = format!("{{\"deleted\":\"{id}\"}}");
                    let p = port;
                    Box::new(move || json_response(&body, 200, p))
                }
            }

            // ── GET /api/search?q=... ─────────────────────────
            _ if url.starts_with("/api/search") && matches!(request.method(), Method::Get) => {
                let q = url
                    .split('?')
                    .nth(1)
                    .and_then(|qs| {
                        qs.split('&')
                            .find(|p| p.starts_with("q="))
                            .map(|p| percent_decode(p.trim_start_matches("q=")))
                    })
                    .unwrap_or_default();
                let db = Rc::clone(&conn);
                let results = do_search_conn(&q, &db.borrow());
                let body = serde_json::to_string(&results).unwrap_or_default();
                let p = port;
                Box::new(move || json_response(&body, 200, p))
            }

            // ── OPTIONS (CORS preflight) ───────────────────────
            (Method::Options, _) => {
                let p = port;
                Box::new(move || json_response("{}", 204, p))
            }

            // ── 404 ───────────────────────────────────────────
            _ => {
                let body = "{\"error\":\"not found\"}".to_string();
                let p = port;
                Box::new(move || json_response(&body, 404, p))
            }
        };

        let _ = request.respond(response());
    }

    0
}

// ── Helpers ───────────────────────────────────────────

/// Create a new node using a shared connection (avoids per-request open_db).
fn handle_post_node_conn(body: &str, conn: &Connection) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();

    let tags: Vec<String> = v["tags"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(MAX_ARRAY_ITEMS)
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let projects: Vec<String> = v["projects"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(MAX_ARRAY_ITEMS)
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let agents: Vec<String> = v["agents"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(MAX_ARRAY_ITEMS)
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let node_type = v["type"].as_str().unwrap_or("concept").to_string();
    if !VALID_NODE_TYPES.contains(&node_type.as_str()) {
        return Err(format!("invalid node type: {node_type}"));
    }
    let importance = v["importance"]
        .as_f64()
        .unwrap_or_else(|| importance_for_type(&node_type))
        .clamp(0.0, 1.0);
    let node = Node {
        frontmatter: NodeFrontmatter {
            id: id.clone(),
            node_type,
            title: v["title"]
                .as_str()
                .unwrap_or("Untitled")
                .chars()
                .take(MAX_TITLE_CHARS)
                .collect(),
            tags,
            projects,
            agents,
            created: now.clone(),
            updated: now,
            importance,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: v["body"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(MAX_BODY_CHARS)
            .collect(),
    };

    write_node_conn(conn, &node).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Update a node using a shared connection (avoids per-request open_db).
fn handle_put_node_conn(id: &str, body: &str, conn: &Connection) -> Result<(), String> {
    let mut node = read_node_conn(conn, id).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;

    if let Some(t) = v["title"].as_str() {
        node.frontmatter.title = t.chars().take(MAX_TITLE_CHARS).collect();
    }
    if let Some(t) = v["type"].as_str() {
        if !VALID_NODE_TYPES.contains(&t) {
            return Err(format!("invalid node type: {t}"));
        }
        node.frontmatter.node_type = t.to_string();
    }
    if let Some(b) = v["body"].as_str() {
        node.body = b.chars().take(MAX_BODY_CHARS).collect();
    }
    if let Some(tags) = v["tags"].as_array() {
        node.frontmatter.tags = tags
            .iter()
            .take(MAX_ARRAY_ITEMS)
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(imp) = v["importance"].as_f64() {
        node.frontmatter.importance = imp.clamp(0.0, 1.0);
    }
    node.frontmatter.updated = now_iso();

    write_node_conn(conn, &node).map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_edge_payload(body: &str) -> Result<Edge, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let source = v["source"].as_str().unwrap_or("").to_string();
    let target = v["target"].as_str().unwrap_or("").to_string();
    if !validate_node_id(&source) || !validate_node_id(&target) {
        return Err("invalid source or target node id".to_string());
    }
    let relation: String = v["relation"]
        .as_str()
        .unwrap_or("related")
        .chars()
        .take(MAX_RELATION_CHARS)
        .collect();
    let weight = v["weight"]
        .as_f64()
        .unwrap_or(1.0)
        .clamp(WEIGHT_MIN, WEIGHT_MAX);
    Ok(Edge {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        target,
        relation,
        weight,
        ts: now_iso(),
    })
}

fn handle_post_edge(body: &str) -> Result<String, String> {
    let edge = parse_edge_payload(body)?;
    let id = edge.id.clone();
    append_edge(&edge).map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
fn handle_post_edge_conn(body: &str, conn: &Connection) -> Result<String, String> {
    use super::store::append_edge_conn;
    let edge = parse_edge_payload(body)?;
    let id = edge.id.clone();
    append_edge_conn(conn, &edge).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Search using a shared connection (avoids per-request open_db).
fn do_search_conn(query: &str, conn: &Connection) -> Vec<serde_json::Value> {
    use serde_json::json;
    search_nodes_conn(conn, query, 20)
        .into_iter()
        .map(|n| {
            let snippet: String = n
                .body
                .chars()
                .take(160)
                .collect::<String>()
                .replace('\n', " ");
            json!({
                "id": n.frontmatter.id,
                "title": n.frontmatter.title,
                "type": n.frontmatter.node_type,
                "snippet": snippet
            })
        })
        .collect()
}

/// Compute degree centrality: top N nodes by total edge count (in + out).
/// Uses a shared connection (no per-request open_db).
pub fn compute_centrality(conn: &Connection, limit: usize) -> Vec<serde_json::Value> {
    let safe_limit = limit.min(100);
    let sql = format!(
        "SELECT n.id, n.title, n.type, n.importance, cnt.total_degree
         FROM (
             SELECT node_id, SUM(degree) AS total_degree FROM (
                 SELECT source AS node_id, COUNT(*) AS degree FROM edges GROUP BY source
                 UNION ALL
                 SELECT target AS node_id, COUNT(*) AS degree FROM edges GROUP BY target
             ) GROUP BY node_id
         ) cnt
         JOIN nodes n ON n.id = cnt.node_id
         ORDER BY cnt.total_degree DESC
         LIMIT {safe_limit}"
    );

    conn.prepare(&sql)
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "type": row.get::<_, String>(2)?,
                    "importance": row.get::<_, f64>(3)?,
                    "degree": row.get::<_, i64>(4)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
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
    fn test_cors_headers_restrict_origin_to_localhost() {
        let headers = cors_headers(9999);
        let origin = headers
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .expect("CORS origin header should exist");
        assert_eq!(
            origin.value.as_str(),
            "http://localhost:9999",
            "CORS origin must be restricted to localhost with the given port"
        );
    }

    #[test]
    fn test_cors_headers_different_ports() {
        let h1 = cors_headers(7700);
        let h2 = cors_headers(8080);
        let o1 = h1
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .unwrap();
        let o2 = h2
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .unwrap();
        assert_eq!(o1.value.as_str(), "http://localhost:7700");
        assert_eq!(o2.value.as_str(), "http://localhost:8080");
    }

    #[test]
    fn test_cors_headers_include_content_type_json() {
        let headers = cors_headers(7700);
        assert!(
            headers
                .iter()
                .any(|h| h.field.equiv("Content-Type") && h.value.as_str() == "application/json")
        );
    }

    #[test]
    fn test_cors_headers_include_methods() {
        let headers = cors_headers(7700);
        assert!(
            headers
                .iter()
                .any(|h| h.field.equiv("Access-Control-Allow-Methods"))
        );
    }

    #[test]
    fn test_html_headers_restrict_origin_to_localhost() {
        let headers = html_headers(7700);
        let origin = headers
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .expect("HTML CORS origin header should exist");
        assert_eq!(
            origin.value.as_str(),
            "http://localhost:7700",
            "HTML CORS origin must be restricted to localhost with the given port"
        );
    }

    #[test]
    fn test_html_headers_content_type() {
        let headers = html_headers(7700);
        assert!(
            headers
                .iter()
                .any(|h| h.field.equiv("Content-Type") && h.value.as_str().contains("text/html"))
        );
    }

    #[test]
    fn test_json_response_uses_port_specific_cors() {
        let resp = json_response("{}", 200, 1234);
        let origin = resp
            .headers()
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .expect("response should have CORS origin header");
        assert_eq!(origin.value.as_str(), "http://localhost:1234");
    }

    #[test]
    fn test_html_response_uses_port_specific_cors() {
        let resp = html_response("<html></html>", 5678);
        let origin = resp
            .headers()
            .iter()
            .find(|h| h.field.equiv("Access-Control-Allow-Origin"))
            .expect("response should have CORS origin header");
        assert_eq!(origin.value.as_str(), "http://localhost:5678");
    }

    #[test]
    fn post_node_rejects_invalid_node_type() {
        use super::super::store::init_schema;
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let body = r#"{"type":"invalid_type","title":"Test","body":"hello"}"#;
        let result = handle_post_node_conn(body, &conn);
        assert!(result.is_err(), "invalid node type must be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("invalid node type"),
            "error must mention invalid node type, got: {err}"
        );
    }

    #[test]
    fn post_node_accepts_valid_node_type() {
        use super::super::store::init_schema;
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let body = r#"{"type":"concept","title":"Test","body":"hello"}"#;
        let result = handle_post_node_conn(body, &conn);
        assert!(
            result.is_ok(),
            "valid node type must be accepted, got err: {:?}",
            result.err()
        );
    }

    #[test]
    fn put_node_rejects_invalid_node_type() {
        use super::super::store::{Node, NodeFrontmatter, init_schema, write_node_conn};
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        // Write a valid node first
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: "test-id-001".to_string(),
                node_type: "concept".to_string(),
                title: "Test".to_string(),
                tags: vec![],
                projects: vec![],
                agents: vec![],
                created: "2026-01-01T00:00:00Z".to_string(),
                updated: "2026-01-01T00:00:00Z".to_string(),
                importance: 0.7,
                access_count: 0,
                accessed_at: String::new(),
            },
            body: "body".to_string(),
        };
        write_node_conn(&conn, &node).expect("write node");
        let update_body = r#"{"type":"bad_type"}"#;
        let result = handle_put_node_conn("test-id-001", update_body, &conn);
        assert!(result.is_err(), "invalid node type in PUT must be rejected");
    }

    #[test]
    fn post_node_caps_body_at_max_chars() {
        use super::super::store::{init_schema, read_node_conn};
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let long_body: String = "a".repeat(MAX_BODY_CHARS + 100);
        let body = format!(
            r#"{{"type":"concept","title":"Big","body":{}}}"#,
            serde_json::to_string(&long_body).unwrap()
        );
        let id = handle_post_node_conn(&body, &conn).expect("should create node");
        let node = read_node_conn(&conn, &id).expect("node should exist");
        assert_eq!(
            node.body.chars().count(),
            MAX_BODY_CHARS,
            "body must be capped at MAX_BODY_CHARS"
        );
    }

    #[test]
    fn put_node_caps_body_at_max_chars() {
        use super::super::store::{
            Node, NodeFrontmatter, init_schema, read_node_conn, write_node_conn,
        };
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: "test-id-002".to_string(),
                node_type: "concept".to_string(),
                title: "Test".to_string(),
                tags: vec![],
                projects: vec![],
                agents: vec![],
                created: "2026-01-01T00:00:00Z".to_string(),
                updated: "2026-01-01T00:00:00Z".to_string(),
                importance: 0.7,
                access_count: 0,
                accessed_at: String::new(),
            },
            body: "original".to_string(),
        };
        write_node_conn(&conn, &node).expect("write node");
        let long_body: String = "x".repeat(MAX_BODY_CHARS + 200);
        let update = format!(
            r#"{{"body":{}}}"#,
            serde_json::to_string(&long_body).unwrap()
        );
        handle_put_node_conn("test-id-002", &update, &conn).expect("should update node");
        let updated = read_node_conn(&conn, "test-id-002").expect("node should exist");
        assert_eq!(
            updated.body.chars().count(),
            MAX_BODY_CHARS,
            "body must be capped at MAX_BODY_CHARS after PUT"
        );
    }

    #[test]
    fn post_node_caps_title_at_max_chars() {
        use super::super::store::{init_schema, read_node_conn};
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let long_title: String = "t".repeat(MAX_TITLE_CHARS + 50);
        let body = format!(
            r#"{{"type":"concept","title":{},"body":"hello"}}"#,
            serde_json::to_string(&long_title).unwrap()
        );
        let id = handle_post_node_conn(&body, &conn).expect("should create node");
        let node = read_node_conn(&conn, &id).expect("node should exist");
        assert_eq!(
            node.frontmatter.title.chars().count(),
            MAX_TITLE_CHARS,
            "title must be capped at MAX_TITLE_CHARS"
        );
    }

    #[test]
    fn post_node_caps_tags_array_at_max_items() {
        use super::super::store::{init_schema, read_node_conn};
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let tags: Vec<String> = (0..(MAX_ARRAY_ITEMS + 10))
            .map(|i| format!("tag{i}"))
            .collect();
        let body = format!(
            r#"{{"type":"concept","title":"T","body":"b","tags":{}}}"#,
            serde_json::to_string(&tags).unwrap()
        );
        let id = handle_post_node_conn(&body, &conn).expect("should create node");
        let node = read_node_conn(&conn, &id).expect("node should exist");
        assert!(
            node.frontmatter.tags.len() <= MAX_ARRAY_ITEMS,
            "tags array must be capped at MAX_ARRAY_ITEMS, got {}",
            node.frontmatter.tags.len()
        );
    }

    #[test]
    fn post_edge_caps_relation_at_max_chars() {
        use super::super::store::{
            Node, NodeFrontmatter, init_schema, read_edges_conn, write_node_conn,
        };
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let id1 = "a1b2c3d4-e5f6-4a7b-8c9d-e0f1a2b3c4d5";
        let id2 = "b2c3d4e5-f6a7-4b8c-9d0e-f1a2b3c4d5e6";
        for (id, title) in [(id1, "Node1"), (id2, "Node2")] {
            let node = Node {
                frontmatter: NodeFrontmatter {
                    id: id.to_string(),
                    node_type: "concept".to_string(),
                    title: title.to_string(),
                    tags: vec![],
                    projects: vec![],
                    agents: vec![],
                    created: "2026-01-01T00:00:00Z".to_string(),
                    updated: "2026-01-01T00:00:00Z".to_string(),
                    importance: 0.5,
                    access_count: 0,
                    accessed_at: String::new(),
                },
                body: "b".to_string(),
            };
            write_node_conn(&conn, &node).expect("write node");
        }
        let long_rel: String = "r".repeat(MAX_RELATION_CHARS + 20);
        let body = format!(
            r#"{{"source":"{id1}","target":"{id2}","relation":{},"weight":1.0}}"#,
            serde_json::to_string(&long_rel).unwrap()
        );
        handle_post_edge_conn(&body, &conn).expect("should create edge");
        let edges = read_edges_conn(&conn);
        assert_eq!(edges.len(), 1);
        assert!(
            edges[0].relation.chars().count() <= MAX_RELATION_CHARS,
            "relation must be capped at MAX_RELATION_CHARS, got {}",
            edges[0].relation.chars().count()
        );
    }

    #[test]
    fn post_edge_clamps_weight_to_valid_range() {
        use super::super::store::{
            Node, NodeFrontmatter, init_schema, read_edges_conn, write_node_conn,
        };
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        let a1 = "a1b2c3d4-e5f6-4a7b-8c9d-000000000001";
        let a2 = "a1b2c3d4-e5f6-4a7b-8c9d-000000000002";
        let b1 = "a1b2c3d4-e5f6-4a7b-8c9d-000000000003";
        let b2 = "a1b2c3d4-e5f6-4a7b-8c9d-000000000004";
        for (id, title) in [(a1, "A1"), (a2, "A2"), (b1, "B1"), (b2, "B2")] {
            let node = Node {
                frontmatter: NodeFrontmatter {
                    id: id.to_string(),
                    node_type: "concept".to_string(),
                    title: title.to_string(),
                    tags: vec![],
                    projects: vec![],
                    agents: vec![],
                    created: "2026-01-01T00:00:00Z".to_string(),
                    updated: "2026-01-01T00:00:00Z".to_string(),
                    importance: 0.5,
                    access_count: 0,
                    accessed_at: String::new(),
                },
                body: "b".to_string(),
            };
            write_node_conn(&conn, &node).expect("write node");
        }
        // Weight below minimum → clamped to WEIGHT_MIN
        handle_post_edge_conn(
            &format!(r#"{{"source":"{a1}","target":"{a2}","relation":"r","weight":-5.0}}"#),
            &conn,
        )
        .expect("should clamp negative weight");
        // Weight above maximum → clamped to WEIGHT_MAX
        handle_post_edge_conn(
            &format!(r#"{{"source":"{b1}","target":"{b2}","relation":"r","weight":999.0}}"#),
            &conn,
        )
        .expect("should clamp oversized weight");

        let edges = read_edges_conn(&conn);
        assert_eq!(edges.len(), 2);
        for e in &edges {
            assert!(
                e.weight >= WEIGHT_MIN && e.weight <= WEIGHT_MAX,
                "weight {} must be in [{WEIGHT_MIN}, {WEIGHT_MAX}]",
                e.weight
            );
        }
    }

    #[test]
    fn test_cors_origin_is_never_wildcard() {
        // Ensure no header set ever returns "*" as origin
        for port in [7700u16, 8080, 3000, 9999] {
            for h in cors_headers(port) {
                if h.field.equiv("Access-Control-Allow-Origin") {
                    assert_ne!(
                        h.value.as_str(),
                        "*",
                        "CORS origin must never be wildcard (port={port})"
                    );
                }
            }
            for h in html_headers(port) {
                if h.field.equiv("Access-Control-Allow-Origin") {
                    assert_ne!(
                        h.value.as_str(),
                        "*",
                        "CORS origin must never be wildcard (port={port})"
                    );
                }
            }
        }
    }
}
