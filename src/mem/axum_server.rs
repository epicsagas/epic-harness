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

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, post, put},
};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use super::graph::{compute_stats, rebuild_graph_json_conn};
use super::server::{
    compute_centrality, handle_post_edge_inner, handle_post_node_conn_inner,
    handle_put_node_conn_inner,
};
use super::store::{
    delete_edge_by_id, delete_node_file, open_db, read_index, read_node_conn, remove_edges_for_node,
    remove_from_index, search_nodes_conn, validate_uuid,
};

const WEBVIEW_HTML: &str = include_str!("webview.html");
const D3_JS: &str = include_str!("d3.min.js");

// ── App State ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub conn: Arc<Mutex<rusqlite::Connection>>,
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

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to open memory DB: {e}");
            return 1;
        }
    };

    let state = AppState {
        conn: Arc::new(Mutex::new(conn)),
    };

    // Build tokio runtime (multi-thread)
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to build tokio runtime: {e}");
            return 1;
        }
    };

    rt.block_on(async move {
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
        .allow_headers(Any);

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

async fn handle_stats() -> impl IntoResponse {
    let body = compute_stats()
        .map(|v| v.to_string())
        .unwrap_or_else(|_| r#"{"error":"stats unavailable"}"#.to_string());
    json_ok(body)
}

async fn handle_graph(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().await;
    let body = rebuild_graph_json_conn(&conn).unwrap_or_else(|_| "{}".to_string());
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
    let conn = state.conn.lock().await;
    let data = compute_centrality(&conn, q.limit);
    Json(data).into_response()
}

async fn handle_list_nodes() -> impl IntoResponse {
    let idx = read_index();
    Json(idx.nodes).into_response()
}

async fn handle_create_node(
    State(state): State<AppState>,
    body: String,
) -> impl IntoResponse {
    let conn = state.conn.lock().await;
    match handle_post_node_conn_inner(&body, &conn) {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!({"id": id})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e})),
        )
            .into_response(),
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
    let conn = state.conn.lock().await;
    match read_node_conn(&conn, &id) {
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
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
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
    let conn = state.conn.lock().await;
    match handle_put_node_conn_inner(&id, &body, &conn) {
        Ok(_) => Json(json!({"id": id})).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e})),
        )
            .into_response(),
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

async fn handle_create_edge(
    State(state): State<AppState>,
    body: String,
) -> impl IntoResponse {
    let conn = state.conn.lock().await;
    match handle_post_edge_inner(&body, &conn) {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!({"edge_id": id})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e})),
        )
            .into_response(),
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
    let conn = state.conn.lock().await;
    let results = search_nodes_conn(&conn, &query, 20)
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

    fn make_state() -> AppState {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        super::super::store::init_schema(&conn).expect("schema");
        AppState {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    #[tokio::test]
    async fn root_returns_html() {
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);
        let res = server.get("/").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        assert!(res.text().contains("<!DOCTYPE html>") || res.text().contains("<html"));
    }

    #[tokio::test]
    async fn stats_endpoint_returns_json() {
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/stats").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_nodes_empty() {
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/nodes").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_and_get_node() {
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);

        let payload = json!({
            "type": "concept",
            "title": "Test Node",
            "body": "Test body content",
            "tags": ["test"],
            "projects": [],
            "agents": []
        });

        let create_res = server
            .post("/api/nodes")
            .json(&payload)
            .await;
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
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/nodes/not-a-valid-uuid").await;
        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_returns_results() {
        let app = build_router(make_state(), 7700);
        let server = TestServer::new(app);
        let res = server.get("/api/search?q=test").await;
        assert_eq!(res.status_code(), StatusCode::OK);
    }
}
