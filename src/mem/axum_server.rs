//! axum_server.rs — axum + tokio based REST API server for harness-mem
//!
//! Replaces the single-threaded `tiny_http` server with an async multi-threaded
//! axum 0.8 server. All business logic (node CRUD, graph, search) is unchanged;
//! only the HTTP transport layer is updated.
//!
//! ## Key improvements over tiny_http
//! - Concurrent request handling (tokio multi-thread runtime)
//! - Standards-compliant CORS via `tower-http`
//! - Proper async I/O — no blocking the thread pool on DB calls (DB ops are
//!   still sync SQLite; they are offloaded with `spawn_blocking`)
//! - Graceful shutdown via CTRL-C signal

use axum::{
    Router,
    body::Body,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};
use sqlx::AnyPool;
use tower_http::cors::CorsLayer;

use super::graph::{compute_stats_pool, rebuild_graph_json_pool_virtual};
use super::store::{
    Edge, MAX_RELATION_CHARS, Node, NodeFrontmatter, append_edge_pool, importance_for_type,
    normalize_relation, now_iso, write_node_pool,
};
use super::store::{
    delete_edge_pool, delete_node_with_edges_pool, read_node_pool, read_nodes_limited_pool,
    search_nodes_pool, validate_uuid,
};
use crate::store::pool;

const WEBVIEW_HTML: &str = include_str!("webview.html");
const D3_JS: &str = include_str!("d3.min.js");

// ── App State ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool: AnyPool,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Start the axum-based harness-mem REST + Web UI server.
/// Blocks until CTRL-C is received or the process is killed.
pub fn serve_axum(args: &[String]) -> i32 {
    let port: u16 = args
        .windows(2)
        .find(|w| w[0] == "--port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(7700);

    let mem_pool = match crate::store::runtime::block_on(pool::memory_pool()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to open memory DB pool: {e}");
            return 1;
        }
    };

    let state = AppState { pool: mem_pool };

    crate::store::runtime::block_on(async move {
        let app = build_router(state, port);

        let addr = format!("127.0.0.1:{port}");
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to bind {addr}: {e}");
                std::process::exit(1);
            }
        };

        eprintln!("[harness] Web UI listening on http://localhost:{port}  (axum)");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .ok();
    });

    0
}

/// Graceful shutdown: wait for CTRL-C.
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
}

// ── Business logic helpers (moved from server.rs) ─────────────────────────────

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
const WEIGHT_MIN: f64 = 0.0;
const WEIGHT_MAX: f64 = 100.0;

/// Create a new node using a pool connection.
async fn handle_post_node_pool(body: &str, pool: &AnyPool) -> Result<String, String> {
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

    write_node_pool(pool, &node)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

/// Update a node using a pool connection.
async fn handle_put_node_pool(id: &str, body: &str, pool: &AnyPool) -> Result<(), String> {
    let mut node = read_node_pool(pool, id).await.map_err(|e| e.to_string())?;
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

    write_node_pool(pool, &node)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_edge_payload(body: &str) -> Result<Edge, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let source = v["source"].as_str().unwrap_or("").to_string();
    let target = v["target"].as_str().unwrap_or("").to_string();
    if !validate_uuid(&source) || !validate_uuid(&target) {
        return Err("invalid source or target node id".to_string());
    }
    let relation = normalize_relation(v["relation"].as_str());
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

/// Create an edge using a pool connection.
async fn handle_post_edge_pool(body: &str, pool: &AnyPool) -> Result<Edge, String> {
    let edge = parse_edge_payload(body)?;
    append_edge_pool(pool, &edge)
        .await
        .map_err(|e| e.to_string())?;
    Ok(edge)
}

/// Compute degree centrality using a pool connection.
pub async fn compute_centrality_pool(pool: &AnyPool, limit: usize) -> Vec<serde_json::Value> {
    use sqlx::Row as SqlxRow;
    let safe_limit = limit.min(100) as i64;
    let sql = "SELECT n.id, n.title, n.type, n.importance, cnt.total_degree
         FROM (
             SELECT node_id, SUM(degree) AS total_degree FROM (
                 SELECT source AS node_id, COUNT(*) AS degree FROM edges GROUP BY source
                 UNION ALL
                 SELECT target AS node_id, COUNT(*) AS degree FROM edges GROUP BY target
             ) GROUP BY node_id
         ) cnt
         JOIN nodes n ON n.id = cnt.node_id
         ORDER BY cnt.total_degree DESC
         LIMIT ?1";

    sqlx::query(sql)
        .bind(safe_limit)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.try_get::<String, _>(0).unwrap_or_default(),
                        "title": r.try_get::<String, _>(1).unwrap_or_default(),
                        "type": r.try_get::<String, _>(2).unwrap_or_default(),
                        "importance": r.try_get::<f64, _>(3).unwrap_or(0.0),
                        "degree": r.try_get::<i64, _>(4).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn build_router(state: AppState, port: u16) -> Router {
    // Allow only the loopback origins (the server binds 127.0.0.1)
    let origins: [HeaderValue; 2] = [
        format!("http://localhost:{port}").parse().unwrap(),
        format!("http://127.0.0.1:{port}").parse().unwrap(),
    ];
    let hosts = [format!("localhost:{port}"), format!("127.0.0.1:{port}")];
    let cors = CorsLayer::new()
        .allow_origin(origins.clone())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT, header::AUTHORIZATION]);

    Router::new()
        // Static
        .route("/", get(handle_root))
        .route("/d3.js", get(handle_d3))
        // Stats & graph
        .route("/api/stats", get(handle_stats))
        .route("/api/graph", get(handle_graph))
        .route("/api/graph/centrality", get(handle_centrality))
        // Nodes collection
        .route("/api/nodes", get(handle_list_nodes))
        .route("/api/nodes", post(handle_create_node))
        // Node by ID
        .route("/api/nodes/{id}", get(handle_get_node))
        .route("/api/nodes/{id}", put(handle_update_node))
        .route("/api/nodes/{id}", delete(handle_delete_node))
        // Edges
        .route(
            "/api/edges",
            post(handle_create_edge).delete(handle_delete_edge),
        )
        // Search
        .route("/api/search", get(handle_search))
        .layer(middleware::from_fn_with_state(origins, guard_mutations))
        .layer(cors)
        .layer(middleware::from_fn_with_state(hosts, guard_host))
        .with_state(state)
}

/// DNS rebinding: a page on an attacker domain that resolves to 127.0.0.1 is
/// same-origin with this server, so neither CORS nor the Origin check applies,
/// but its requests still carry the attacker's Host. Serve only the loopback
/// hosts the server binds, for every method (a rebound GET can read the graph).
async fn guard_host(State(hosts): State<[String; 2]>, req: Request, next: Next) -> Response {
    // HTTP/2 has no Host header, only the :authority carried in the URI.
    let host = match req.headers().get(header::HOST) {
        Some(h) => h.to_str().ok(),
        None => req.uri().authority().map(|a| a.as_str()),
    };
    if !host.is_some_and(|h| hosts.iter().any(|a| a.eq_ignore_ascii_case(h))) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "host not allowed"})),
        )
            .into_response();
    }
    next.run(req).await
}

/// CORS alone does not stop a cross-site `text/plain` POST (a "simple" request
/// needs no preflight and the handler still runs). Reject mutating requests
/// from a foreign origin or `Sec-Fetch-Site: cross-site`, and require a JSON
/// content type on bodies, which a foreign page cannot send without a preflight.
async fn guard_mutations(
    State(origins): State<[HeaderValue; 2]>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method();
    if matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        let h = req.headers();
        let foreign_origin = h.get(header::ORIGIN).is_some_and(|o| !origins.contains(o));
        let cross_site = h.get("sec-fetch-site").is_some_and(|v| v == "cross-site");
        if foreign_origin || cross_site {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "cross-origin request rejected"})),
            )
                .into_response();
        }
        if *method != Method::DELETE && !is_json(h) {
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(json!({"error": "content-type must be application/json"})),
            )
                .into_response();
        }
    }
    next.run(req).await
}

fn is_json(h: &HeaderMap) -> bool {
    h.get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn handle_root() -> Html<&'static str> {
    Html(WEBVIEW_HTML)
}

async fn handle_d3() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(D3_JS))
        .unwrap()
}

async fn handle_stats(State(state): State<AppState>) -> impl IntoResponse {
    let body = compute_stats_pool(&state.pool)
        .await
        .map(|v| v.to_string())
        .unwrap_or_else(|_| r#"{"error":"stats unavailable"}"#.to_string());
    json_ok(body)
}

#[derive(serde::Deserialize)]
struct GraphQuery {
    #[serde(default = "default_true")]
    include_virtual: bool,
}

fn default_true() -> bool {
    true
}

async fn handle_graph(
    State(state): State<AppState>,
    Query(q): Query<GraphQuery>,
) -> impl IntoResponse {
    let body = rebuild_graph_json_pool_virtual(&state.pool, q.include_virtual)
        .await
        .unwrap_or_else(|_| "{}".to_string());
    json_ok(body)
}

#[derive(serde::Deserialize)]
struct CentralityQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}

async fn handle_centrality(
    State(state): State<AppState>,
    Query(q): Query<CentralityQuery>,
) -> impl IntoResponse {
    let data = compute_centrality_pool(&state.pool, q.limit).await;
    Json(data).into_response()
}

#[derive(serde::Deserialize)]
struct ListNodesQuery {
    #[serde(default = "default_list_limit")]
    limit: usize,
}

fn default_list_limit() -> usize {
    500
}

async fn handle_list_nodes(
    State(state): State<AppState>,
    Query(q): Query<ListNodesQuery>,
) -> impl IntoResponse {
    use super::store::IndexNode;
    let limit = q.limit.min(5000);
    let nodes: Vec<IndexNode> = read_nodes_limited_pool(&state.pool, limit as i64)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|n| IndexNode {
            id: n.frontmatter.id,
            title: n.frontmatter.title,
            node_type: n.frontmatter.node_type,
            tags: n.frontmatter.tags,
            projects: n.frontmatter.projects,
            updated: n.frontmatter.updated,
        })
        .collect();
    Json(nodes).into_response()
}

async fn handle_create_node(State(state): State<AppState>, body: String) -> impl IntoResponse {
    match handle_post_node_pool(&body, &state.pool).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

async fn handle_get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !validate_uuid(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node id"})),
        )
            .into_response();
    }
    match read_node_pool(&state.pool, &id).await {
        Ok(node) => Json(json!({
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
            "body": node.body,
        }))
        .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn handle_update_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: String,
) -> impl IntoResponse {
    if !validate_uuid(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node id"})),
        )
            .into_response();
    }
    match handle_put_node_pool(&id, &body, &state.pool).await {
        Ok(_) => Json(json!({"id": id})).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

async fn handle_delete_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !validate_uuid(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node id"})),
        )
            .into_response();
    }
    match delete_node_with_edges_pool(&state.pool, &id).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "node not found"})),
        )
            .into_response(),
        Ok(_) => Json(json!({"deleted": id})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn handle_create_edge(State(state): State<AppState>, body: String) -> impl IntoResponse {
    match handle_post_edge_pool(&body, &state.pool).await {
        Ok(e) => (
            StatusCode::CREATED,
            Json(json!({"source": e.source, "target": e.target, "relation": e.relation})),
        )
            .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct EdgeKeyQuery {
    source: Option<String>,
    target: Option<String>,
    relation: Option<String>,
}

async fn handle_delete_edge(
    State(state): State<AppState>,
    Query(q): Query<EdgeKeyQuery>,
) -> impl IntoResponse {
    let source = q.source.unwrap_or_default();
    let target = q.target.unwrap_or_default();
    let relation = q.relation.unwrap_or_default();
    if !validate_uuid(&source)
        || !validate_uuid(&target)
        || relation.is_empty()
        || relation.chars().count() > MAX_RELATION_CHARS
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid source, target or relation"})),
        )
            .into_response();
    }
    match delete_edge_pool(&state.pool, &source, &target, &relation).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "edge not found"})),
        )
            .into_response(),
        Ok(_) => {
            Json(json!({"deleted": {"source": source, "target": target, "relation": relation}}))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

async fn handle_search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let results = search_nodes_pool(&state.pool, &query, 20)
        .await
        .unwrap_or_default()
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
                "snippet": snippet,
            })
        })
        .collect::<Vec<Value>>();
    Json(results).into_response()
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn json_ok(body: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn default_limit() -> usize {
    20
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    async fn make_state() -> AppState {
        // Use an isolated in-memory sqlx pool for each test
        let pool = crate::store::pool::test_memory_pool().await;
        // Initialize schema for the test pool
        crate::mem::store::init_schema_pool(&pool).await.unwrap();
        AppState { pool }
    }

    /// In-process server that, like a real client, sends the loopback Host header.
    fn test_server(app: Router) -> TestServer {
        let mut server = TestServer::new(app);
        server.add_header(header::HOST, "localhost:7700");
        server
    }

    #[tokio::test]
    async fn root_returns_html() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);
        let res = server.get("/").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        assert!(res.text().contains("<!DOCTYPE html>") || res.text().contains("<html"));
    }

    #[tokio::test]
    async fn stats_endpoint_returns_json() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);
        let res = server.get("/api/stats").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_nodes_empty() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);
        let res = server.get("/api/nodes").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_and_get_node() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);

        let payload = json!({
            "type": "concept",
            "title": "Test Node",
            "body": "Test body content",
            "tags": ["test"],
            "projects": [],
            "agents": []
        });

        let create_res = server.post("/api/nodes").json(&payload).await;
        assert_eq!(create_res.status_code(), StatusCode::CREATED);
        let body: Value = create_res.json();
        let id = body["id"].as_str().expect("id in response").to_string();

        let get_res = server.get(&format!("/api/nodes/{id}")).await;
        assert_eq!(get_res.status_code(), StatusCode::OK);
        let node: Value = get_res.json();
        assert_eq!(node["title"].as_str(), Some("Test Node"));
    }

    #[tokio::test]
    async fn invalid_node_id_returns_400() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);
        let res = server.get("/api/nodes/not-a-valid-uuid").await;
        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_returns_results() {
        let app = build_router(make_state().await, 7700);
        let server = test_server(app);
        let res = server.get("/api/search?q=test").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_nodes_limit_parameter() {
        let state = make_state().await;
        let app = build_router(state.clone(), 7700);
        let server = test_server(app);

        // Insert 5 nodes
        for i in 0..5 {
            let payload = json!({
                "type": "concept",
                "title": format!("Limit test node {i}"),
                "body": format!("body {i}"),
                "tags": ["limit-test"],
                "projects": [],
                "agents": []
            });
            let res = server.post("/api/nodes").json(&payload).await;
            assert_eq!(res.status_code(), StatusCode::CREATED);
        }

        // Without limit — should return all 5 (default 500)
        let all = server.get("/api/nodes").await;
        assert_eq!(all.status_code(), StatusCode::OK);
        let nodes: Value = all.json();
        assert_eq!(nodes.as_array().unwrap().len(), 5);

        // With limit=2 — should return exactly 2
        let limited = server.get("/api/nodes?limit=2").await;
        assert_eq!(limited.status_code(), StatusCode::OK);
        let nodes: Value = limited.json();
        assert_eq!(
            nodes.as_array().unwrap().len(),
            2,
            "limit=2 should return exactly 2 nodes"
        );

        // With limit=0 — should return empty
        let zero = server.get("/api/nodes?limit=0").await;
        assert_eq!(zero.status_code(), StatusCode::OK);
        let nodes: Value = zero.json();
        assert!(
            nodes.as_array().unwrap().is_empty(),
            "limit=0 should return empty array"
        );
    }

    async fn create_node(server: &TestServer, title: &str) -> String {
        let res = server
            .post("/api/nodes")
            .json(&json!({"type": "concept", "title": title, "body": "alpha body"}))
            .await;
        assert_eq!(res.status_code(), StatusCode::CREATED);
        res.json::<Value>()["id"].as_str().unwrap().to_string()
    }

    async fn count(pool: &AnyPool, sql: &str) -> i64 {
        use sqlx::Row;
        sqlx::query(sqlx::AssertSqlSafe(sql.to_string()))
            .fetch_one(pool)
            .await
            .unwrap()
            .get::<i64, _>(0)
    }

    #[tokio::test]
    async fn delete_node_removes_node_edges_and_fts_entry() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Delete me").await;
        let b = create_node(&server, "Keep me").await;
        let res = server
            .post("/api/edges")
            .json(&json!({"source": a, "target": b}))
            .await;
        assert_eq!(res.status_code(), StatusCode::CREATED);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 1);

        let res = server.delete(&format!("/api/nodes/{a}")).await;
        assert_eq!(res.status_code(), StatusCode::OK);

        let res = server.get(&format!("/api/nodes/{a}")).await;
        assert_eq!(res.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 0);
        assert_eq!(
            count(
                &state.pool,
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH 'Delete'"
            )
            .await,
            0
        );
        let kept = server.get(&format!("/api/nodes/{b}")).await;
        assert_eq!(kept.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_missing_node_returns_404() {
        let server = test_server(build_router(make_state().await, 7700));
        let res = server
            .delete("/api/nodes/00000000-0000-4000-8000-000000000000")
            .await;
        assert_eq!(res.status_code(), StatusCode::NOT_FOUND);
    }

    const MISSING_ID: &str = "00000000-0000-4000-8000-000000000000";

    async fn create_edge(server: &TestServer, source: &str, target: &str, relation: &str) -> Value {
        let res = server
            .post("/api/edges")
            .json(&json!({"source": source, "target": target, "relation": relation}))
            .await;
        assert_eq!(res.status_code(), StatusCode::CREATED);
        res.json::<Value>()
    }

    fn delete_edge_req(
        server: &TestServer,
        source: &str,
        target: &str,
        relation: &str,
    ) -> axum_test::TestRequest {
        server
            .delete("/api/edges")
            .add_query_param("source", source)
            .add_query_param("target", target)
            .add_query_param("relation", relation)
    }

    #[tokio::test]
    async fn delete_edge_reports_not_found_without_panic() {
        let server = test_server(build_router(make_state().await, 7700));
        let res = delete_edge_req(&server, MISSING_ID, MISSING_ID, "related").await;
        assert_eq!(res.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_edge_by_node_id_keeps_node_edges() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;
        let c = create_node(&server, "Node C").await;
        create_edge(&server, &a, &b, "related").await;
        create_edge(&server, &b, &c, "related").await;
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 2);

        // A node id as source with a target that matches no edge deletes nothing.
        let res = delete_edge_req(&server, &a, &c, "related").await;
        assert_eq!(res.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 2);
    }

    #[tokio::test]
    async fn delete_edge_removes_only_the_exact_triple() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;
        create_edge(&server, &a, &b, "uses").await;
        create_edge(&server, &a, &b, "blocks").await;

        let res = delete_edge_req(&server, &a, &b, "uses").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        assert_eq!(
            res.json::<Value>(),
            json!({"deleted": {"source": a, "target": b, "relation": "uses"}})
        );
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 1);
        assert_eq!(
            count(
                &state.pool,
                "SELECT COUNT(*) FROM edges WHERE label = 'blocks'"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn delete_edge_deletes_at_most_one_row_for_any_triple() {
        let state = make_state().await;
        let ids: Vec<String> = (0..4).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let labels = ["uses", "blocks", "related"];
        for s in &ids {
            for t in &ids {
                for l in labels {
                    let e = Edge {
                        id: String::new(),
                        source: s.clone(),
                        target: t.clone(),
                        relation: l.to_string(),
                        weight: 1.0,
                        ts: now_iso(),
                    };
                    append_edge_pool(&state.pool, &e).await.unwrap();
                }
            }
        }
        // Random triples mix existing ids, a fresh id and a label no edge has.
        let mut pool_ids = ids.clone();
        pool_ids.push(uuid::Uuid::new_v4().to_string());
        let pool_labels = ["uses", "blocks", "related", "absent"];
        let rnd = |n: usize| uuid::Uuid::new_v4().as_u128() as usize % n;
        for _ in 0..64 {
            let s = &pool_ids[rnd(pool_ids.len())];
            let t = &pool_ids[rnd(pool_ids.len())];
            let label = pool_labels[rnd(pool_labels.len())];
            let before = count(&state.pool, "SELECT COUNT(*) FROM edges").await;
            let n = delete_edge_pool(&state.pool, s, t, label).await.unwrap();
            let after = count(&state.pool, "SELECT COUNT(*) FROM edges").await;
            assert!(n <= 1, "deleted {n} rows for one triple");
            assert_eq!(before - after, n as i64);
        }
    }

    #[tokio::test]
    async fn post_edge_response_round_trips_into_delete() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;

        // Default relation and a relation longer than the cap (stored truncated).
        let long = "x".repeat(MAX_RELATION_CHARS + 6);
        for body in [
            json!({"source": a, "target": b}),
            json!({"source": a, "target": b, "relation": long}),
        ] {
            let res = server.post("/api/edges").json(&body).await;
            assert_eq!(res.status_code(), StatusCode::CREATED);
            let key = res.json::<Value>();
            let relation = key["relation"].as_str().unwrap();
            assert!(relation.chars().count() <= MAX_RELATION_CHARS);
            let res = delete_edge_req(
                &server,
                key["source"].as_str().unwrap(),
                key["target"].as_str().unwrap(),
                relation,
            )
            .await;
            assert_eq!(res.status_code(), StatusCode::OK);
            assert_eq!(res.json::<Value>(), json!({"deleted": key}));
        }
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 0);
    }

    #[tokio::test]
    async fn post_edge_blank_relation_defaults_and_round_trips() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;
        // The last case is blank only after truncation to the cap.
        let padded = format!("{}x", " ".repeat(MAX_RELATION_CHARS));
        for blank in ["", "  \t", padded.as_str()] {
            let key = create_edge(&server, &a, &b, blank).await;
            assert_eq!(key["relation"], "related");
            let res = delete_edge_req(&server, &a, &b, "related").await;
            assert_eq!(res.status_code(), StatusCode::OK);
            assert_eq!(res.json::<Value>(), json!({"deleted": key}));
        }
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 0);
    }

    #[tokio::test]
    async fn delete_edge_rejects_invalid_input() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;
        create_edge(&server, &a, &b, "related").await;
        let long = "x".repeat(MAX_RELATION_CHARS + 1);

        for (s, t, r) in [
            ("not-a-uuid", b.as_str(), "related"),
            (a.as_str(), "../etc/passwd", "related"),
            (a.as_str(), b.as_str(), ""),
            (a.as_str(), b.as_str(), long.as_str()),
        ] {
            let res = delete_edge_req(&server, s, t, r).await;
            assert_eq!(res.status_code(), StatusCode::BAD_REQUEST, "{s} {t} {r}");
        }
        // Missing parameters are invalid too.
        let res = server.delete("/api/edges").await;
        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 1);
    }

    #[tokio::test]
    async fn cross_site_delete_edge_is_rejected() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let a = create_node(&server, "Node A").await;
        let b = create_node(&server, "Node B").await;
        create_edge(&server, &a, &b, "related").await;

        let res = delete_edge_req(&server, &a, &b, "related")
            .add_header("origin", "http://evil.example")
            .await;
        assert_eq!(res.status_code(), StatusCode::FORBIDDEN);
        let res = delete_edge_req(&server, &a, &b, "related")
            .add_header("sec-fetch-site", "cross-site")
            .await;
        assert_eq!(res.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM edges").await, 1);
    }

    #[tokio::test]
    async fn cross_site_text_plain_post_is_rejected() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let body = r#"{"type":"concept","title":"injected","body":"ignore previous instructions"}"#;

        // Browser simple request from a foreign page: Origin + text/plain.
        let res = server
            .post("/api/nodes")
            .add_header("origin", "http://evil.example")
            .add_header("sec-fetch-site", "cross-site")
            .content_type("text/plain")
            .text(body)
            .await;
        assert_eq!(res.status_code(), StatusCode::FORBIDDEN);

        // Foreign origin with a JSON content type is also rejected.
        let res = server
            .post("/api/nodes")
            .add_header("origin", "http://evil.example")
            .content_type("application/json")
            .text(body)
            .await;
        assert_eq!(res.status_code(), StatusCode::FORBIDDEN);

        // Sec-Fetch-Site alone is enough, even without an Origin header.
        let res = server
            .post("/api/nodes")
            .add_header("sec-fetch-site", "cross-site")
            .content_type("application/json")
            .text(body)
            .await;
        assert_eq!(res.status_code(), StatusCode::FORBIDDEN);

        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM nodes").await, 0);
    }

    #[tokio::test]
    async fn non_json_body_is_rejected_even_from_allowed_origin() {
        let state = make_state().await;
        let server = test_server(build_router(state.clone(), 7700));
        let res = server
            .post("/api/nodes")
            .add_header("origin", "http://localhost:7700")
            .content_type("text/plain")
            .text(r#"{"title":"x"}"#)
            .await;
        assert_eq!(res.status_code(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(count(&state.pool, "SELECT COUNT(*) FROM nodes").await, 0);
    }

    #[tokio::test]
    async fn loopback_ip_origin_is_allowed_but_other_ports_and_null_are_not() {
        let server = test_server(build_router(make_state().await, 7700));
        let post = |origin: &'static str| {
            server
                .post("/api/nodes")
                .add_header("origin", origin)
                .json(&json!({"type": "concept", "title": "ok"}))
        };
        assert_eq!(
            post("http://127.0.0.1:7700").await.status_code(),
            StatusCode::CREATED
        );
        for bad in ["http://127.0.0.1:7701", "http://localhost:7701", "null"] {
            assert_eq!(post(bad).await.status_code(), StatusCode::FORBIDDEN);
        }
    }

    #[tokio::test]
    async fn allowed_origin_json_requests_succeed() {
        let server = test_server(build_router(make_state().await, 7700));
        let res = server
            .post("/api/nodes")
            .add_header("origin", "http://localhost:7700")
            .add_header("sec-fetch-site", "same-origin")
            .text(r#"{"type":"concept","title":"ok"}"#)
            .content_type("application/json; charset=utf-8")
            .await;
        assert_eq!(res.status_code(), StatusCode::CREATED);
        let id = res.json::<Value>()["id"].as_str().unwrap().to_string();

        // The bundled webview sends DELETE with the same Origin header.
        let res = server
            .delete(&format!("/api/nodes/{id}"))
            .add_header("origin", "http://localhost:7700")
            .await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn only_loopback_host_with_port_is_served() {
        // No default Host here: each request carries its own.
        let server = TestServer::new(build_router(make_state().await, 7700));
        for ok in ["localhost:7700", "127.0.0.1:7700", "LOCALHOST:7700"] {
            let res = server.get("/api/stats").add_header("host", ok).await;
            assert_eq!(res.status_code(), StatusCode::OK, "{ok}");
        }
        // DNS rebinding: the page origin is an attacker domain that resolves to 127.0.0.1.
        for bad in [
            "evil.example",
            "evil.example:7700",
            "localhost",
            "127.0.0.1",
            "localhost:7701",
            "127.0.0.1:7701",
            "localhost.evil.example:7700",
            "[::1]:7700",
        ] {
            let res = server.get("/api/nodes").add_header("host", bad).await;
            assert_eq!(res.status_code(), StatusCode::FORBIDDEN, "GET {bad}");
            let res = server
                .post("/api/nodes")
                .add_header("host", bad)
                .json(&json!({"type": "concept", "title": "rebind"}))
                .await;
            assert_eq!(res.status_code(), StatusCode::FORBIDDEN, "POST {bad}");
        }
    }

    /// Status line of a raw HTTP request over a real socket. `host: None` sends HTTP/1.0 without Host.
    fn raw_status(port: u16, host: Option<&str>) -> String {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let req = match host {
            Some(h) => format!("GET /api/stats HTTP/1.1\r\nHost: {h}\r\nConnection: close\r\n\r\n"),
            None => "GET /api/stats HTTP/1.0\r\n\r\n".to_string(),
        };
        stream.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out.lines().next().unwrap_or_default().to_string()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn real_socket_clients_with_correct_host_keep_working() {
        let state = make_state().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = build_router(state, port);
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let statuses = tokio::task::spawn_blocking(move || {
            [
                raw_status(port, Some(&format!("127.0.0.1:{port}"))),
                raw_status(port, Some(&format!("localhost:{port}"))),
                raw_status(port, Some("evil.example")),
                raw_status(port, Some(&format!("evil.example:{port}"))),
                raw_status(port, None),
            ]
        })
        .await
        .unwrap();
        assert!(statuses[0].contains(" 200 "), "{statuses:?}");
        assert!(statuses[1].contains(" 200 "), "{statuses:?}");
        for rejected in &statuses[2..] {
            assert!(rejected.contains(" 403 "), "{statuses:?}");
        }
    }
}
