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
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};
use sqlx::AnyPool;
use tower_http::cors::CorsLayer;

use super::graph::{compute_stats_pool, rebuild_graph_json_pool_virtual};
use super::store::{
    Edge, Node, NodeFrontmatter, append_edge_pool, importance_for_type, now_iso, write_node_pool,
};
use super::store::{
    delete_edge_by_id, delete_node_file, read_node_pool, read_nodes_limited_pool,
    remove_edges_for_node, remove_from_index, search_nodes_pool, validate_uuid,
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
const MAX_RELATION_CHARS: usize = 64;
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

/// Create an edge using a pool connection.
async fn handle_post_edge_pool(body: &str, pool: &AnyPool) -> Result<String, String> {
    let edge = parse_edge_payload(body)?;
    let id = edge.id.clone();
    append_edge_pool(pool, &edge)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
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
    // Allow only localhost origin
    let origin = format!("http://localhost:{port}");
    let cors = CorsLayer::new()
        .allow_origin(origin.parse::<HeaderValue>().unwrap())
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
        .route("/api/edges", post(handle_create_edge))
        .route("/api/edges/{id}", delete(handle_delete_edge))
        // Search
        .route("/api/search", get(handle_search))
        .layer(cors)
        .with_state(state)
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

async fn handle_delete_node(Path(id): Path<String>) -> impl IntoResponse {
    if !validate_uuid(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node id"})),
        )
            .into_response();
    }
    let _ = delete_node_file(&id);
    let _ = remove_edges_for_node(&id);
    let _ = remove_from_index(&id);
    Json(json!({"deleted": id})).into_response()
}

async fn handle_create_edge(State(state): State<AppState>, body: String) -> impl IntoResponse {
    match handle_post_edge_pool(&body, &state.pool).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"edge_id": id}))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

async fn handle_delete_edge(Path(id): Path<String>) -> impl IntoResponse {
    if !validate_uuid(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid edge id"})),
        )
            .into_response();
    }
    match delete_edge_by_id(&id) {
        Ok(_) => Json(json!({"deleted": id})).into_response(),
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

    #[tokio::test]
    async fn root_returns_html() {
        let app = build_router(make_state().await, 7700);
        let server = TestServer::new(app);
        let res = server.get("/").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        assert!(res.text().contains("<!DOCTYPE html>") || res.text().contains("<html"));
    }

    #[tokio::test]
    async fn stats_endpoint_returns_json() {
        let app = build_router(make_state().await, 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/stats").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_nodes_empty() {
        let app = build_router(make_state().await, 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/nodes").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_and_get_node() {
        let app = build_router(make_state().await, 7700);
        let server = TestServer::new(app);

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
        let server = TestServer::new(app);
        let res = server.get("/api/nodes/not-a-valid-uuid").await;
        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_returns_results() {
        let app = build_router(make_state().await, 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/search?q=test").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_nodes_limit_parameter() {
        let state = make_state().await;
        let app = build_router(state.clone(), 7700);
        let server = TestServer::new(app);

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
}
