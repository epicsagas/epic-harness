//! serve.rs — Web dashboard for orchestration state and memory graph
//! Serves a single-page Svelte dashboard at http://localhost:{port}

use tiny_http::{Header, Method, Response, Server};

use crate::hooks::common;
use crate::mem::{graph, store};
use crate::orchestrate::state as orch;

const DEFAULT_PORT: u16 = 7700;

// HTML is embedded at compile time from assets/dashboard.html
static DASHBOARD_HTML: &str = include_str!("../assets/dashboard.html");

// The binary's own version, stamped into the served dashboard HTML so the UI
// always shows the version users actually installed — regardless of whether
// the bundled assets/dashboard.html was rebuilt or app/package.json was bumped
// before release. Source of truth = the running binary, not the frontend build.
const HARNESS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Inject the binary version into the embedded dashboard HTML as a meta tag.
/// Called on every dashboard page serve so the version is always correct.
fn dashboard_html_with_version() -> String {
    let meta = format!("<meta name=\"harness-version\" content=\"{HARNESS_VERSION}\">");
    // Inject immediately after the opening <head> (first occurrence only).
    match DASHBOARD_HTML.split_once("<head>") {
        Some((before, after)) => format!("{before}<head>{meta}{after}"),
        None => {
            // Fallback: no <head> tag found — prepend the meta so the version
            // is still discoverable.
            format!("{meta}{DASHBOARD_HTML}")
        }
    }
}

/// `epic-harness dashboard [--port=N]` — serve + open browser
pub fn run_dashboard(port: Option<u16>) -> i32 {
    let port = port.unwrap_or(DEFAULT_PORT);
    let url = format!("http://localhost:{port}");

    // Spawn the server in a background thread
    let port_copy = port;
    std::thread::spawn(move || {
        run_serve(Some(port_copy));
    });

    // Give the server a moment to bind
    std::thread::sleep(std::time::Duration::from_millis(200));

    eprintln!("epic-harness dashboard → {url}");

    // Open browser (best-effort, platform-aware)
    let opened = open_browser(&url);
    if !opened {
        eprintln!("브라우저를 자동으로 열 수 없습니다. 직접 접속하세요: {url}");
    }

    // Block main thread — Ctrl+C to stop
    eprintln!("Press Ctrl+C to stop.");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().is_ok()
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

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
            // Every route, reads included: a DNS-rebound page keeps its own
            // Host and could otherwise read /api/* from the browser.
            _ if !is_loopback_host(request.headers(), port) => forbidden_response(),

            (Method::Get, "/") | (Method::Get, "/index.html") => Response::from_string(
                dashboard_html_with_version(),
            )
            .with_header(Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap()),

            // ── Orchestration API ───────────────────────────
            (Method::Get, url) if url.split('?').next() == Some("/api/run") => {
                let (status, body) = handle_orch_request(url);
                json_response(&body).with_status_code(status)
            }

            (Method::Get, "/api/events") => {
                let data = if let Ok(Some(run)) = crate::store::runtime::block_on(async {
                    let pool = crate::store::pool::harness_pool().await?;
                    crate::store::orchestrator::read_run_pool(&pool).await
                }) {
                    serde_json::to_string(&run).unwrap_or_else(|_| "null".into())
                } else {
                    let harness_dir = common::harness_dir();
                    serde_json::to_string(&orch::read_run(&harness_dir))
                        .unwrap_or_else(|_| "null".into())
                };
                let sse_body = format!("data: {data}\n\n");
                Response::from_string(sse_body)
                    .with_header(Header::from_bytes(b"Content-Type", b"text/event-stream").unwrap())
                    .with_header(Header::from_bytes(b"Cache-Control", b"no-cache").unwrap())
                    .with_header(Header::from_bytes(b"Connection", b"keep-alive").unwrap())
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

            // ── Project list ─────────────────────────────────
            // Returns the sorted list of project slugs discovered under
            // ~/.harness/projects/. The frontend project dropdown populates
            // from this; without it the dropdown is empty and the previously-
            // selected project stays pinned (cannot switch, cannot see all).
            (Method::Get, "/api/projects") => {
                let root = crate::shared::paths::harness_projects_root();
                let mut slugs: Vec<String> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&root) {
                    for entry in rd.filter_map(|e| e.ok()) {
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                            && let Some(name) = entry.file_name().to_str()
                        {
                            slugs.push(name.to_string());
                        }
                    }
                }
                slugs.sort();
                json_response(&serde_json::to_string(&slugs).unwrap_or_else(|_| "[]".into()))
            }

            // ── Harness API ──────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/harness") => {
                let (status, body) = handle_harness_request(url, &common::harness_dir());
                json_response(&body).with_status_code(status)
            }

            // ── Orbit Pipeline Dismiss ───────────────────────
            (Method::Delete, url) if url.starts_with("/api/orbit/") => {
                if !is_loopback_same_origin(request.headers(), port) {
                    forbidden_response()
                } else {
                    let pipeline_id = url.trim_start_matches("/api/orbit/").trim_end_matches('/');
                    let harness_dir = common::harness_dir();
                    let pool =
                        crate::store::runtime::block_on(crate::store::pool::harness_pool()).ok();
                    let (status, body) =
                        dismiss_orbit_pipeline(pipeline_id, &harness_dir, pool.as_ref());
                    json_response(&body).with_status_code(status)
                }
            }

            // ── Memory API (Nodes) ───────────────────────────
            (Method::Get, "/api/nodes") => {
                let nodes = crate::store::runtime::block_on(async {
                    let pool = crate::store::pool::memory_pool().await?;
                    store::read_all_nodes_pool(&pool).await
                })
                .unwrap_or_default();
                let results: Vec<serde_json::Value> = nodes
                    .into_iter()
                    .map(|n| {
                        serde_json::json!({
                            "id": n.frontmatter.id,
                            "type": n.frontmatter.node_type,
                            "title": n.frontmatter.title,
                            "tags": n.frontmatter.tags,
                            "projects": n.frontmatter.projects,
                            "updated": n.frontmatter.updated
                        })
                    })
                    .collect();
                json_response(&serde_json::to_string(&results).unwrap_or_else(|_| "[]".into()))
            }

            (Method::Get, url) if url.starts_with("/api/nodes/") => {
                let id = url
                    .trim_start_matches("/api/nodes/")
                    .split('?')
                    .next()
                    .unwrap_or("");
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
                    }
                    Err(_) => "{\"error\":\"not found\"}".into(),
                };
                json_response(&body)
            }

            // ── Memory API (Search) ──────────────────────────
            (Method::Get, url) if url.starts_with("/api/search") => {
                let query = url
                    .split("q=")
                    .nth(1)
                    .unwrap_or("")
                    .split('&')
                    .next()
                    .unwrap_or("");
                let decoded = percent_decode(query);
                let nodes = store::search_nodes(&decoded, 20);
                let results: Vec<serde_json::Value> =
                    nodes.into_iter().map(search_result_json).collect();
                let body = serde_json::to_string(&results).unwrap_or_else(|_| "[]".into());
                json_response(&body)
            }

            // ── Agents API ───────────────────────────────────
            (Method::Get, url)
                if url.starts_with("/api/agents/")
                    && url
                        .split('?')
                        .next()
                        .is_some_and(|p| p.ends_with("/status")) =>
            {
                let (status, body) = handle_orch_request(url);
                json_response(&body).with_status_code(status)
            }

            (Method::Delete, url) if url.starts_with("/api/agents/") => {
                let agent_id = url.trim_start_matches("/api/agents/").trim_end_matches('/');
                let harness_dir = common::harness_dir();
                if !is_loopback_same_origin(request.headers(), port) {
                    forbidden_response()
                } else if !orch::validate_agent_id(agent_id) {
                    json_response("{\"error\":\"invalid agent id\"}").with_status_code(400)
                } else {
                    let ok = crate::store::runtime::block_on(async {
                        let pool = crate::store::pool::harness_pool().await.ok()?;
                        crate::store::orchestrator::dismiss_agent_pool(&pool, agent_id)
                            .await
                            .ok()
                    })
                    .unwrap_or_else(|| {
                        // Fallback: delete agent status file directly
                        let status_path = harness_dir
                            .join("orchestrator")
                            .join("agents")
                            .join(agent_id)
                            .join("status.json");
                        status_path.exists() && std::fs::remove_file(status_path).is_ok()
                    });
                    let body = serde_json::json!({"ok": ok, "dismissed": agent_id}).to_string();
                    json_response(&body)
                }
            }

            // ── CORS & Other ────────────────────────────────
            (Method::Options, _) => Response::from_string("{}")
                .with_status_code(204)
                .with_header(
                    Header::from_bytes(b"Access-Control-Allow-Origin", b"http://localhost:5173")
                        .unwrap(),
                )
                .with_header(
                    Header::from_bytes(
                        b"Access-Control-Allow-Methods",
                        b"GET, POST, PUT, DELETE, OPTIONS",
                    )
                    .unwrap(),
                )
                .with_header(
                    Header::from_bytes(b"Access-Control-Allow-Headers", b"Content-Type").unwrap(),
                ),

            // ── SPA fallback: serve index.html for non-API, non-static-asset GET routes ──
            (Method::Get, url)
                if !url.starts_with("/api/")
                    && !url.contains('.')
                    && !url.starts_with("/favicon")
                    && !url.starts_with("/robots")
                    && !url.starts_with("/sitemap") =>
            {
                Response::from_string(DASHBOARD_HTML).with_header(
                    Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
                )
            }

            _ => Response::from_string("Not Found").with_status_code(404),
        };
        let _ = request.respond(response);
    }
    0
}

/// One `/api/search` row. The Memory page renders it as a `MemoryNode`, so it
/// carries the list columns (`tags`, `importance`, `updated`), not only a snippet.
fn search_result_json(n: crate::mem::store::Node) -> serde_json::Value {
    serde_json::json!({
        "id": n.frontmatter.id,
        "title": n.frontmatter.title,
        "type": n.frontmatter.node_type,
        "tags": n.frontmatter.tags,
        "importance": n.frontmatter.importance,
        "updated": n.frontmatter.updated,
        "snippet": n.body.chars().take(160).collect::<String>()
    })
}

fn json_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(Header::from_bytes(b"Content-Type", b"application/json").unwrap())
        .with_header(
            Header::from_bytes(b"Access-Control-Allow-Origin", b"http://localhost:5173").unwrap(),
        )
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

/// Dismiss (delete) an orbit pipeline across all projects: the
/// `PIPELINE-<id>.json` sidecar and, when `pool` is given, the DB-backed row
/// (reflect re-syncs sidecars into SQLite, so both must go or it reappears).
///
/// The id may carry the `PIPELINE-` file prefix; the rest must be non-empty
/// `[A-Za-z0-9_-]` and is matched against the exact file name (e.g. id
/// "20260523105350"). Returns `(http_status, json_body)`.
fn dismiss_orbit_pipeline(
    pipeline_id: &str,
    harness_dir: &std::path::Path,
    pool: Option<&sqlx::AnyPool>,
) -> (u16, String) {
    let bare = pipeline_id.strip_prefix("PIPELINE-").unwrap_or(pipeline_id);
    if bare.is_empty()
        || !bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return (
            400,
            serde_json::json!({"ok": false, "error": "invalid pipeline id"}).to_string(),
        );
    }

    let projects_root = harness_dir.parent().unwrap_or(harness_dir);
    let file_name = format!("PIPELINE-{bare}.json");
    let mut deleted = false;

    if let Ok(rd) = std::fs::read_dir(projects_root) {
        for proj_entry in rd.filter_map(|e| e.ok()) {
            let path = proj_entry.path().join("orbit").join(&file_name);
            if path.is_file() && std::fs::remove_file(path).is_ok() {
                deleted = true;
            }
        }
    }

    // Rows are keyed by the sidecar's `id` field, or by the file stem when it
    // has none, so either spelling can be stored.
    if let Some(pool) = pool {
        for id in [bare.to_string(), format!("PIPELINE-{bare}")] {
            deleted |= crate::store::runtime::block_on(
                crate::store::orbit_store::dismiss_pipeline_pool(pool, &id),
            )
            .unwrap_or(false);
        }
    }

    if deleted {
        (
            200,
            serde_json::json!({"ok": true, "dismissed": pipeline_id}).to_string(),
        )
    } else {
        (
            404,
            serde_json::json!({"ok": false, "error": "pipeline not found"}).to_string(),
        )
    }
}

/// Header value by case-insensitive name.
fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

/// `Host` must be a loopback name on our port (DNS rebinding: a rebound
/// hostname resolves to 127.0.0.1 but keeps its own `Host`). Fails closed on
/// a missing header.
fn is_loopback_host(headers: &[Header], port: u16) -> bool {
    header_value(headers, "Host").is_some_and(|h| {
        h.eq_ignore_ascii_case(&format!("localhost:{port}"))
            || h.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
    })
}

/// Extra gate for mutating (DELETE) routes, mirroring `mem/axum_server.rs`:
/// the loopback `Host` check plus rejecting a foreign `Origin` or
/// `Sec-Fetch-Site: cross-site` (CSRF).
fn is_loopback_same_origin(headers: &[Header], port: u16) -> bool {
    let host_ok = is_loopback_host(headers, port);
    let origin_ok = header_value(headers, "Origin").is_none_or(|o| {
        o.eq_ignore_ascii_case(&format!("http://localhost:{port}"))
            || o.eq_ignore_ascii_case(&format!("http://127.0.0.1:{port}"))
    });
    let cross_site = header_value(headers, "Sec-Fetch-Site").is_some_and(|v| v == "cross-site");
    host_ok && origin_ok && !cross_site
}

fn forbidden_response() -> Response<std::io::Cursor<Vec<u8>>> {
    json_response("{\"error\":\"cross-origin request rejected\"}").with_status_code(403)
}

/// `/api/run` and `/api/agents/{id}/status`, optionally scoped by `?project=`.
/// Same selector rules as `handle_harness_request`: absent or `__all__` is the
/// dashboard process's own project, an invalid slug is a 400 before any path
/// is built. A missing file keeps the old `{}` body.
fn handle_orch_request(url: &str) -> (u16, String) {
    let project = parse_query_param(url, "project").filter(|p| !p.is_empty() && p != "__all__");
    if let Some(p) = &project
        && !crate::shared::paths::is_valid_project_slug(p)
    {
        return (400, "{\"error\":\"invalid project\"}".into());
    }
    let base = crate::shared::paths::resolve_harness_dir(project.as_deref());
    let path = url.split('?').next().unwrap_or("");
    let body = if path == "/api/run" {
        serde_json::to_string(&orch::read_run(&base))
    } else {
        let agent_id = path
            .trim_start_matches("/api/agents/")
            .trim_end_matches("/status");
        if !orch::validate_agent_id(agent_id) {
            return (400, "{\"error\":\"invalid agent id\"}".into());
        }
        serde_json::to_string(&orch::read_agent_status(&base, agent_id))
    };
    match body {
        Ok(b) if b != "null" => (200, b),
        _ => (200, "{}".into()),
    }
}

/// `/api/harness?cmd=..&project=..`. `project` selects a specific project
/// slug; absent or `__all__` means cross-project aggregate (the frontend's
/// projectArgs() omits the param entirely for "all"). A slug is joined onto
/// the data root by the file-backed readers, so anything outside
/// `[A-Za-z0-9._-]` (or `.`/`..`) is rejected with 400 here, the single entry
/// every project-scoped command routes through.
fn handle_harness_request(url: &str, harness_dir: &std::path::Path) -> (u16, String) {
    let cmd = parse_query_param(url, "cmd").unwrap_or_default();
    let project = parse_query_param(url, "project").filter(|p| !p.is_empty() && p != "__all__");
    if let Some(p) = &project
        && !crate::shared::paths::is_valid_project_slug(p)
    {
        return (400, "{\"error\":\"invalid project\"}".into());
    }
    (
        200,
        handle_harness_cmd(&cmd, harness_dir, project.as_deref()),
    )
}

/// Collect `PIPELINE-*.json` sidecars from every project's `orbit/` dir,
/// tagging each with its `_project`. Files that are not JSON objects are
/// skipped: indexing a non-object would panic the single request loop.
fn scan_orbit_pipeline_files(projects_root: &std::path::Path) -> Vec<serde_json::Value> {
    let mut all: Vec<serde_json::Value> = vec![];
    if let Ok(rd) = std::fs::read_dir(projects_root) {
        for proj_entry in rd.filter_map(|e| e.ok()) {
            let orbit_dir = proj_entry.path().join("orbit");
            if !orbit_dir.exists() {
                continue;
            }
            if let Ok(files) = std::fs::read_dir(&orbit_dir) {
                for f in files.filter_map(|e| e.ok()) {
                    let fname = f.file_name().to_string_lossy().to_string();
                    if fname.starts_with("PIPELINE-")
                        && fname.ends_with(".json")
                        && let Ok(content) = std::fs::read_to_string(f.path())
                        && let Ok(mut v) = crate::shared::helpers::parse_persisted_json::<
                            serde_json::Value,
                        >(&content)
                        && let Some(obj) = v.as_object_mut()
                    {
                        // Same key `dismiss_orbit_pipeline` stores and deletes by.
                        if !obj.get("id").is_some_and(|i| i.is_string()) {
                            obj.insert(
                                "id".into(),
                                serde_json::Value::String(
                                    fname.trim_end_matches(".json").to_string(),
                                ),
                            );
                        }
                        obj.insert(
                            "_project".into(),
                            serde_json::Value::String(
                                proj_entry.file_name().to_string_lossy().to_string(),
                            ),
                        );
                        all.push(v);
                    }
                }
            }
        }
    }
    all
}

/// Orbit pipelines for the dashboard: SQLite rows merged with `PIPELINE-*.json`
/// sidecars, scoped to `project` when given.
pub fn orbit_pipelines(
    harness_dir: &std::path::Path,
    project: Option<&str>,
) -> Vec<serde_json::Value> {
    let db = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        crate::store::orbit_store::list_pipelines_scoped_pool(&pool, project, None).await
    })
    .unwrap_or_default();
    let projects_root = harness_dir.parent().unwrap_or(harness_dir);
    let mut sidecars = scan_orbit_pipeline_files(projects_root);
    if let Some(p) = project {
        sidecars.retain(|v| v["_project"].as_str() == Some(p));
    }
    merge_orbit_pipelines(db, sidecars)
}

/// Merge DB rows and sidecars keyed by `(project, bare id)` where the bare id
/// drops a `PIPELINE-` prefix. The DB row wins; sidecars without a string id
/// are always kept. Newest `started_at` first, missing last.
fn merge_orbit_pipelines(
    db: Vec<serde_json::Value>,
    sidecars: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let key = |v: &serde_json::Value, project_key: &str| {
        v["id"].as_str().map(|id| {
            (
                v[project_key].as_str().unwrap_or("").to_string(),
                id.strip_prefix("PIPELINE-").unwrap_or(id).to_string(),
            )
        })
    };
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<serde_json::Value> = Vec::new();
    for mut row in db {
        if let Some(p) = row["project"].clone().as_str()
            && let Some(obj) = row.as_object_mut()
        {
            obj.insert("_project".into(), p.into());
        }
        seen.extend(key(&row, "project"));
        out.push(row);
    }
    for sc in sidecars {
        if key(&sc, "_project").is_none_or(|k| seen.insert(k)) {
            out.push(sc);
        }
    }
    out.sort_by(|a, b| b["started_at"].as_str().cmp(&a["started_at"].as_str()));
    out
}

/// Session date from a `YYYYMMDD_...` id prefix; anything else is "unknown".
fn session_date(session_id: &str) -> String {
    match session_id.split('_').next() {
        Some(d) if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) => d.to_string(),
        _ => "unknown".into(),
    }
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next() == Some(key) {
            return kv.next().map(percent_decode);
        }
    }
    None
}

/// Tail-read the falsifiability ledger sidecar (manifests.jsonl), newest
/// first. Capped at the most recent 50 to bound payload size. Shared by the
/// dashboard server and the native app.
pub fn recent_manifests(project: Option<&str>) -> Vec<serde_json::Value> {
    let path = crate::shared::paths::manifests_file_for(project);
    let mut items: Vec<serde_json::Value> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines().rev().take(50) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                items.push(v);
            }
        }
    }
    items
}

fn handle_harness_cmd(cmd: &str, harness_dir: &std::path::Path, project: Option<&str>) -> String {
    use std::fs;
    match cmd {
        "get_harness_metrics" => {
            // Project-scoped load: a specific slug, or cross-project aggregate
            // when `project` is None. Uses the (key, project)-keyed
            // metrics_state table correctly (the unfiltered loader returned
            // indeterminate results with multiple projects).
            if let Ok(metrics) = crate::store::runtime::block_on(async {
                let pool = crate::store::pool::harness_pool().await?;
                crate::store::metrics::load_metrics_scoped_pool(&pool, project).await
            }) {
                return serde_json::to_string(&metrics).unwrap_or_else(|_| "null".into());
            }
            // File fallback — only meaningful for the active project dir.
            let p = harness_dir.join("metrics.json");
            fs::read_to_string(&p).unwrap_or_else(|_| "null".into())
        }
        "get_evolved_skills" => {
            // Try SQLite first — scoped to the selected project.
            if let Ok(pool) = crate::store::runtime::block_on(crate::store::pool::harness_pool()) {
                let skills = crate::store::runtime::block_on(
                    crate::store::evolved::list_skills_full_scoped_pool(&pool, project),
                )
                .unwrap_or_default()
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "skill_md": s.skill_md,
                        "created_at": s.created
                    })
                })
                .collect::<Vec<_>>();
                let history = crate::store::runtime::block_on(
                    crate::store::evolution::query_recent_records_scoped_pool(&pool, 50, project),
                )
                .unwrap_or_default()
                .into_iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect::<Vec<_>>();
                let total_sessions = crate::store::runtime::block_on(
                    crate::store::metrics::load_metrics_scoped_pool(&pool, project),
                )
                .map(|m| m.total_sessions)
                .unwrap_or(0);
                if !skills.is_empty() || !history.is_empty() {
                    return serde_json::json!({
                        "evolved_skills": skills,
                        "evolution_history": history,
                        "total_sessions_analyzed": total_sessions,
                        "patterns_detected": history.len()
                    })
                    .to_string();
                }
            }

            // Fallback: file-based reading
            let evolved_dir = harness_dir.join("evolved");
            let skills: Vec<serde_json::Value> = if evolved_dir.exists() {
                fs::read_dir(&evolved_dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .filter(|e| e.path().is_dir())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                let skill_md_path = e.path().join("SKILL.md");
                                let skill_md = fs::read_to_string(&skill_md_path).unwrap_or_default();
                                serde_json::json!({ "name": name, "skill_md": skill_md, "created_at": null })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            };
            let evo_log = harness_dir.join("evolution.jsonl");
            let history: Vec<serde_json::Value> = if evo_log.exists() {
                let mut buf: std::collections::VecDeque<serde_json::Value> =
                    std::collections::VecDeque::with_capacity(51);
                if let Ok(file) = fs::File::open(&evo_log) {
                    use std::io::BufRead;
                    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str(&line) {
                            buf.push_back(v);
                            if buf.len() > 50 {
                                buf.pop_front();
                            }
                        }
                    }
                }
                buf.into_iter().rev().collect()
            } else {
                vec![]
            };
            let metrics_path = harness_dir.join("metrics.json");
            let total_sessions: u64 = fs::read_to_string(&metrics_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v["total_sessions"].as_u64())
                .unwrap_or(0);
            serde_json::json!({
                "evolved_skills": skills,
                "evolution_history": history,
                "total_sessions_analyzed": total_sessions,
                "patterns_detected": history.len()
            })
            .to_string()
        }
        "get_obs_summary" => {
            // Try SQLite first, fallback to JSONL
            if let Ok(stats) = crate::store::runtime::block_on(async {
                let pool = crate::store::pool::harness_pool().await?;
                crate::store::observations::query_obs_stats_scoped_pool(
                    &pool,
                    "2020-01-01", // all data
                    "2099-12-31",
                    project,
                )
                .await
            }) && stats.total > 0
            {
                let tool_stats: Vec<serde_json::Value> = {
                    let mut v: Vec<_> = stats
                        .tool_stats
                        .iter()
                        .map(|t| {
                            serde_json::json!({
                                "tool": t.tool,
                                "calls": t.calls,
                                "success_rate": if t.calls > 0 {
                                    (t.successes as f64 / t.calls as f64 * 1000.0).round() / 1000.0
                                } else { 0.0 },
                                "avg_score": (t.avg_score * 1000.0).round() / 1000.0
                            })
                        })
                        .collect();
                    v.sort_by(|a, b| {
                        b["calls"]
                            .as_i64()
                            .unwrap_or(0)
                            .cmp(&a["calls"].as_i64().unwrap_or(0))
                    });
                    v
                };
                let recent_sessions: Vec<serde_json::Value> = stats
                    .session_stats
                    .iter()
                    .take(10)
                    .map(|s| {
                        serde_json::json!({
                            "session_id": s.session_id,
                            "date": session_date(&s.session_id),
                            "tool_calls": s.calls,
                            "avg_score": (s.avg_score * 1000.0).round() / 1000.0,
                            "failures": s.failures
                        })
                    })
                    .collect();
                return serde_json::json!({
                    "recent_sessions": recent_sessions,
                    "tool_stats": tool_stats,
                    "total_tool_calls": stats.total,
                    "avg_score": (stats.avg_score * 1000.0).round() / 1000.0,
                    "active_agents": []
                })
                .to_string();
            }

            // Fallback: JSONL file parsing (for legacy data)
            let obs_dir = harness_dir.join("obs");
            if !obs_dir.exists() {
                return serde_json::json!({
                    "recent_sessions": [],
                    "tool_stats": [],
                    "total_tool_calls": 0,
                    "avg_score": 0.0,
                    "active_agents": []
                })
                .to_string();
            }
            use std::collections::HashMap;
            let mut tool_map: HashMap<String, (u64, u64, f64)> = HashMap::new();
            let mut session_map: HashMap<String, (u64, f64, u64, String)> = HashMap::new();

            let mut files: Vec<_> = fs::read_dir(&obs_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
                        .collect()
                })
                .unwrap_or_default();
            files.sort_by_key(|e| e.file_name());

            for entry in &files {
                let fname = entry.file_name().to_string_lossy().to_string();
                let session_key = fname.trim_end_matches(".jsonl").to_string();
                let date = session_date(fname.split_once('_').map_or("", |(_, rest)| rest));
                let sess = session_map
                    .entry(session_key.clone())
                    .or_insert((0, 0.0, 0, date));

                if let Ok(content) = fs::read_to_string(entry.path()) {
                    for line in content.lines().filter(|l| !l.is_empty()) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                            let tool = v["tool"].as_str().unwrap_or("unknown").to_string();
                            let is_success = v["result"].as_str() == Some("success")
                                || v["tool_success"].as_bool() == Some(true);
                            let score = v["score"]
                                .as_f64()
                                .or_else(|| v["composite_score"].as_f64())
                                .unwrap_or(0.0);
                            let t = tool_map.entry(tool).or_insert((0, 0, 0.0));
                            t.0 += 1;
                            if is_success {
                                t.1 += 1;
                            }
                            t.2 += score;
                            sess.0 += 1;
                            sess.1 += score;
                            if !is_success {
                                sess.2 += 1;
                            }
                        }
                    }
                }
            }

            let tool_stats: Vec<serde_json::Value> = {
                let mut v: Vec<_> = tool_map
                    .iter()
                    .map(|(tool, (calls, successes, score_sum))| {
                        serde_json::json!({
                            "tool": tool,
                            "calls": calls,
                            "success_rate": if *calls > 0 {
                                (*successes as f64 / *calls as f64 * 1000.0).round() / 1000.0
                            } else { 0.0 },
                            "avg_score": if *calls > 0 {
                                (score_sum / *calls as f64 * 1000.0).round() / 1000.0
                            } else { 0.0 }
                        })
                    })
                    .collect();
                v.sort_by(|a, b| {
                    b["calls"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&a["calls"].as_u64().unwrap_or(0))
                });
                v
            };
            let recent_sessions: Vec<serde_json::Value> = {
                let mut v: Vec<_> = session_map.iter().collect();
                v.sort_by(|a, b| b.0.cmp(a.0));
                v.into_iter().take(10).map(|(sid, (calls, score_sum, failures, date))| {
                    serde_json::json!({
                        "session_id": sid,
                        "date": date,
                        "tool_calls": calls,
                        "avg_score": if *calls > 0 { (*score_sum / *calls as f64 * 1000.0).round() / 1000.0 } else { 0.0 },
                        "failures": failures
                    })
                }).collect()
            };
            let total: u64 = tool_stats
                .iter()
                .map(|t| t["calls"].as_u64().unwrap_or(0))
                .sum();
            let avg = if total > 0 {
                tool_stats
                    .iter()
                    .map(|t| {
                        t["avg_score"].as_f64().unwrap_or(0.0) * t["calls"].as_f64().unwrap_or(0.0)
                    })
                    .sum::<f64>()
                    / total as f64
            } else {
                0.0
            };

            serde_json::json!({
                "recent_sessions": recent_sessions,
                "tool_stats": tool_stats,
                "total_tool_calls": total,
                "avg_score": (avg * 1000.0).round() / 1000.0,
                "active_agents": []
            })
            .to_string()
        }
        "get_orbit_pipelines" => serde_json::to_string(&orbit_pipelines(harness_dir, project))
            .unwrap_or_else(|_| "[]".into()),
        "get_integration_status" => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default();
            // Codex: detect either the legacy `~/.codex/hooks.json` install path
            // (written by `epic install codex`) OR the modern plugin marketplace
            // install (Codex's own `codex plugin install epic@epicsagas`), which
            // unpacks the plugin under `~/.codex/plugins/cache/epicsagas/`.
            let codex_legacy = std::path::Path::new(&home)
                .join(".codex/hooks.json")
                .exists();
            let codex_plugin = std::path::Path::new(&home)
                .join(".codex/plugins/cache/epicsagas")
                .exists();
            let codex_installed = codex_legacy || codex_plugin;
            // null when not installed — matches Cursor/Cline/Aider so the
            // dashboard never shows a phantom path for an absent Codex.
            let codex_config_path: Option<&str> = if !codex_installed {
                None
            } else if codex_plugin {
                Some("~/.codex/plugins/cache/epicsagas/")
            } else {
                Some("~/.codex/hooks.json")
            };
            let integrations = serde_json::json!([
                {
                    "name": "Claude Code",
                    "installed": std::path::Path::new(&home)
                        .join(".claude/settings.json")
                        .exists(),
                    "config_path": "~/.claude/settings.json",
                    "version": null
                },
                {
                    "name": "Antigravity",
                    "installed": std::path::Path::new(&home)
                        .join(".gemini/config/mcp_config.json")
                        .exists(),
                    "config_path": "~/.gemini/config/mcp_config.json",
                    "version": null
                },
                {
                    "name": "Codex",
                    "installed": codex_installed,
                    "config_path": codex_config_path,
                    "version": null
                },
                { "name": "Cursor", "installed": false, "config_path": null, "version": null },
                { "name": "Cline",  "installed": false, "config_path": null, "version": null },
                { "name": "Aider",  "installed": false, "config_path": null, "version": null }
            ]);
            integrations.to_string()
        }
        "get_graph" => {
            graph::rebuild_graph_json().unwrap_or_else(|_| r#"{"nodes":[],"edges":[]}"#.into())
        }
        // ── HarnessX evolution-engine surfaces ────────────────────────────
        // These read the state the 4-PR evolution stack writes, so the
        // dashboard can surface reward-hacking, regression, variant, and
        // adaptation-landscape state. Each is a thin reader; computation
        // stays in the evolve modules.
        "get_seesaw_registry" => {
            let reg = crate::evolve::seesaw::load_registry_for(project);
            serde_json::to_string(&reg).unwrap_or_else(|_| "null".into())
        }
        "get_variant_pool" => {
            let pool = crate::evolve::variants::VariantPool::load_for(project);
            serde_json::to_string(&pool).unwrap_or_else(|_| "null".into())
        }
        "get_harness_snapshot" => {
            let snap = crate::evolve::snapshot::build_snapshot_for(project);
            serde_json::to_string(&snap).unwrap_or_else(|_| "null".into())
        }
        "get_adaptation_landscape" => {
            // Landscape is computed from evolution history + current digests.
            // For the dashboard we compute it from history + an empty digest
            // set (the per-session digests are reflect-only here); the
            // persistent_failures / edit_type_coverage / untried_edit_types
            // come from history alone.
            let history = crate::store::runtime::block_on(async {
                let pool = crate::store::pool::harness_pool().await?;
                crate::store::evolution::query_recent_records_scoped_pool(&pool, i64::MAX, project)
                    .await
            })
            .unwrap_or_default();
            let landscape = crate::evolve::planner::build_landscape(&history, &[], 2);
            serde_json::to_string(&landscape).unwrap_or_else(|_| "null".into())
        }
        "get_manifests" => {
            serde_json::to_string(&recent_manifests(project)).unwrap_or_else(|_| "[]".into())
        }
        // ── Previously-broken panels (returned "null" via the fallthrough) ─
        "get_session_snapshots" => {
            let snaps = crate::store::runtime::block_on(async {
                let pool = crate::store::pool::harness_pool().await?;
                crate::store::sessions::list_recent_snapshots_scoped_pool(&pool, 50, project).await
            })
            .unwrap_or_default();
            serde_json::to_string(&snaps).unwrap_or_else(|_| "[]".into())
        }
        "get_global_patterns" => {
            // Cross-project global patterns (opt-in feature). Parse the JSONL
            // ledger line-by-line into a JSON array; raw text would not parse
            // as a single JSON value on the TS client. Cap at recent 50.
            let path = crate::shared::paths::global_patterns_file();
            let mut items: Vec<serde_json::Value> = Vec::new();
            if let Ok(text) = std::fs::read_to_string(&path) {
                for line in text.lines().rev().take(50) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        items.push(v);
                    }
                }
            }
            serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
        }
        "get_effect_pending" => {
            // Whether a cross-project effect export is pending this session.
            // We approximate: present if the marker file exists.
            let enabled = crate::shared::paths::cross_project_file().is_file();
            serde_json::json!({ "enabled": enabled }).to_string()
        }
        _ => "null".into(),
    }
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

    /// `<root>/projects/<slug>/orbit/<file>` with `content`; returns the path.
    fn write_sidecar(
        root: &std::path::Path,
        slug: &str,
        file: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let dir = root.join("projects").join(slug).join("orbit");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(file);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn dismiss_orbit_pipeline_rejects_empty_and_invalid_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let a = write_sidecar(tmp.path(), "proj-a", "PIPELINE-20260101000000.json", "{}");
        let b = write_sidecar(tmp.path(), "proj-b", "PIPELINE-20260202000000.json", "{}");
        let harness_dir = tmp.path().join("projects").join("proj-a");

        for bad in ["", "..", "../x", "a/b", "a.b", "20260101000000.json", "%2e"] {
            let (status, _) = dismiss_orbit_pipeline(bad, &harness_dir, None);
            assert_eq!(status, 400, "id {bad:?}");
        }
        assert!(a.exists() && b.exists(), "invalid ids must delete nothing");
    }

    #[test]
    fn dismiss_orbit_pipeline_matches_exact_file_name_only() {
        let tmp = tempfile::tempdir().unwrap();
        let a = write_sidecar(tmp.path(), "proj-a", "PIPELINE-20260101000000.json", "{}");
        let b = write_sidecar(tmp.path(), "proj-b", "PIPELINE-20260202000000.json", "{}");
        let harness_dir = tmp.path().join("projects").join("proj-a");

        // A substring of an existing id is not an id.
        let (status, _) = dismiss_orbit_pipeline("2026", &harness_dir, None);
        assert_eq!(status, 404);
        assert!(a.exists() && b.exists());

        let (status, body) = dismiss_orbit_pipeline("20260101000000", &harness_dir, None);
        assert_eq!(status, 200);
        assert!(body.contains("\"ok\":true"));
        assert!(!a.exists(), "exact id deletes its file");
        assert!(b.exists(), "other pipelines are untouched");
    }

    #[test]
    fn scan_orbit_pipeline_files_skips_non_object_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_sidecar(tmp.path(), "proj-a", "PIPELINE-1.json", "[]");
        write_sidecar(tmp.path(), "proj-a", "PIPELINE-2.json", "42");
        write_sidecar(tmp.path(), "proj-a", "PIPELINE-3.json", "not json");
        write_sidecar(tmp.path(), "proj-a", "PIPELINE-4.json", r#"{"id":"4"}"#);

        let all = scan_orbit_pipeline_files(&tmp.path().join("projects"));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["id"], "4");
        assert_eq!(all[0]["_project"], "proj-a");
    }

    #[test]
    fn merge_orbit_pipelines_dedups_per_project_and_sorts() {
        use serde_json::json;
        let merged = merge_orbit_pipelines(
            vec![json!({"id": "PIPELINE-123", "project": "a", "started_at": "2026-01-01"})],
            vec![
                json!({"id": "123", "_project": "a", "started_at": "2026-09-09"}),
                json!({"id": "123", "_project": "b", "started_at": "2026-02-02"}),
                json!({"_project": "b"}),
            ],
        );
        assert_eq!(merged.len(), 3, "{merged:?}");
        let a: Vec<_> = merged.iter().filter(|v| v["_project"] == "a").collect();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0]["id"], "PIPELINE-123", "DB row wins");
        let starts: Vec<_> = merged.iter().map(|v| v["started_at"].as_str()).collect();
        assert_eq!(starts, [Some("2026-02-02"), Some("2026-01-01"), None]);
    }

    #[test]
    #[serial_test::serial]
    fn orbit_pipelines_filters_sidecars_by_project() {
        let tmp = tempfile::tempdir().unwrap();
        struct RestoreHarnessDir(Option<std::ffi::OsString>);
        impl Drop for RestoreHarnessDir {
            fn drop(&mut self) {
                // SAFETY: serial test, no other thread reads the environment.
                unsafe {
                    match self.0.take() {
                        Some(v) => std::env::set_var("HARNESS_DIR", v),
                        None => std::env::remove_var("HARNESS_DIR"),
                    }
                }
            }
        }
        let _restore = RestoreHarnessDir(std::env::var_os("HARNESS_DIR"));
        // SAFETY: relocates the DB store to the temp dir so the real
        // ~/.harness is never opened; restored by the guard above.
        unsafe { std::env::set_var("HARNESS_DIR", tmp.path().join("projects").join("a")) };
        write_sidecar(tmp.path(), "a", "PIPELINE-1.json", r#"{"id":"1"}"#);
        write_sidecar(tmp.path(), "b", "PIPELINE-2.json", r#"{"id":"2"}"#);
        let harness_dir = tmp.path().join("projects").join("a");
        let rows = orbit_pipelines(&harness_dir, Some("a"));
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0]["_project"], "a");
        assert_eq!(orbit_pipelines(&harness_dir, None).len(), 2);
    }

    #[test]
    fn scan_orbit_pipeline_files_defaults_id_to_file_stem() {
        let tmp = tempfile::tempdir().unwrap();
        write_sidecar(tmp.path(), "proj-a", "PIPELINE-5.json", "{}");
        let all = scan_orbit_pipeline_files(&tmp.path().join("projects"));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["id"], "PIPELINE-5");
    }

    #[test]
    fn session_snapshot_serializes_as_object_with_type_key() {
        let snap = crate::shared::types::SessionSnapshot {
            timestamp: "2026-01-01T00:00:00Z".into(),
            snap_type: "auto".into(),
            summary: "s".into(),
            pending_tasks: vec![],
            context_usage: None,
            pipeline_state: None,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&vec![snap]).unwrap()).unwrap();
        assert!(v[0].is_object() && v[0]["type"] == "auto");
        assert!(v.as_array().unwrap().iter().all(|i| !i.is_string()));
    }

    #[test]
    fn session_date_requires_eight_digit_prefix() {
        assert_eq!(session_date("20260602_12345"), "20260602");
        assert_eq!(
            session_date("9f1c2b3a-0000-4000-8000-000000000000"),
            "unknown"
        );
        assert_eq!(session_date(""), "unknown");
        assert_eq!(session_date("2026060_x"), "unknown");
    }

    #[test]
    fn search_result_carries_tags_for_the_memory_page() {
        let node = crate::mem::store::Node {
            frontmatter: crate::mem::store::NodeFrontmatter {
                id: "n1".into(),
                node_type: "decision".into(),
                title: "t".into(),
                tags: vec!["a".into(), "b".into()],
                updated: "2026-01-01".into(),
                importance: 0.9,
                ..Default::default()
            },
            body: "x".repeat(300),
        };
        let v = search_result_json(node);
        assert_eq!(v["tags"], serde_json::json!(["a", "b"]));
        assert_eq!(v["importance"], 0.9);
        assert_eq!(v["updated"], "2026-01-01");
        assert_eq!(v["snippet"].as_str().unwrap().len(), 160);
    }

    fn hdr(name: &str, value: &str) -> Header {
        Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
    }

    #[test]
    fn harness_request_rejects_unsafe_project_selectors() {
        let tmp = tempfile::tempdir().unwrap();
        let harness_dir = tmp.path().join("projects").join("proj-a");
        for bad in [
            "..", ".", "../x", "%2e%2e", "..%2Fx", "a%2Fb", "%2Fetc", "a%5Cb", "C%3A",
        ] {
            let url = format!("/api/harness?cmd=get_seesaw_registry&project={bad}");
            let (status, _) = handle_harness_request(&url, &harness_dir);
            assert_eq!(status, 400, "project {bad:?}");
        }
        assert!(
            !tmp.path().join("projects").exists(),
            "rejected selectors must not touch the filesystem"
        );

        // Valid slugs, absent and `__all__` still reach the command.
        for url in [
            "/api/harness?cmd=get_effect_pending&project=proj-b.1_x",
            "/api/harness?cmd=get_effect_pending&project=__all__",
            "/api/harness?cmd=get_effect_pending",
        ] {
            let (status, body) = handle_harness_request(url, &harness_dir);
            assert_eq!(status, 200, "{url}");
            assert!(body.contains("enabled"), "{url}");
        }
    }

    /// Point `HARNESS_DIR` at `root` for one serial test, restoring on drop.
    struct HarnessDirGuard(Option<std::ffi::OsString>);
    impl HarnessDirGuard {
        fn set(root: &std::path::Path) -> Self {
            let prev = std::env::var_os("HARNESS_DIR");
            // SAFETY: serial test, no other thread reads the environment.
            unsafe { std::env::set_var("HARNESS_DIR", root) };
            Self(prev)
        }
    }
    impl Drop for HarnessDirGuard {
        fn drop(&mut self) {
            // SAFETY: serial test, no other thread reads the environment.
            unsafe {
                match self.0.take() {
                    Some(v) => std::env::set_var("HARNESS_DIR", v),
                    None => std::env::remove_var("HARNESS_DIR"),
                }
            }
        }
    }

    fn write_orch_run(root: &std::path::Path, slug: &str, run_id: &str, agent_id: &str) {
        let base = root.join("projects").join(slug);
        let run = orch::OrchestrationRun {
            id: run_id.into(),
            status: orch::RunStatus::Running,
            agents: vec![],
            dependency_graph: Default::default(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        std::fs::create_dir_all(orch::orchestrator_dir(&base)).unwrap();
        orch::write_run(&base, &run).unwrap();
        let status = orch::AgentStatusFile {
            agent_id: agent_id.into(),
            phase: format!("phase-{slug}"),
            progress: 0.5,
            last_heartbeat: "2026-01-01T00:00:00Z".into(),
            status: orch::AgentStatus::Running,
        };
        orch::write_agent_status(&base, agent_id, &status).unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn api_run_is_scoped_by_project_query() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = HarnessDirGuard::set(tmp.path());
        write_orch_run(tmp.path(), "proj-a", "run-a", "ag-1");
        write_orch_run(tmp.path(), "proj-b", "run-b", "ag-1");

        let (status, body) = handle_orch_request("/api/run?project=proj-a");
        assert_eq!(status, 200);
        assert!(body.contains("run-a") && !body.contains("run-b"), "{body}");
        let (_, body) = handle_orch_request("/api/run?project=proj-b");
        assert!(body.contains("run-b") && !body.contains("run-a"), "{body}");

        // Valid slug with no data keeps the old empty-object body.
        assert_eq!(
            handle_orch_request("/api/run?project=proj-none"),
            (200, "{}".into())
        );

        // Absent and `__all__` both mean the dashboard process's own project.
        let own = serde_json::to_string(&orch::read_run(&common::harness_dir()))
            .map(|b| if b == "null" { "{}".into() } else { b })
            .unwrap();
        for url in ["/api/run", "/api/run?project=__all__", "/api/run?project="] {
            assert_eq!(handle_orch_request(url), (200, own.clone()), "{url}");
        }
    }

    #[test]
    #[serial_test::serial]
    fn api_run_and_status_reject_unsafe_project_selectors() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = HarnessDirGuard::set(tmp.path());
        for bad in ["..", "a/b", "a%2Fb", "%2e%2e", "%2Fetc"] {
            for path in ["/api/run", "/api/agents/ag-1/status"] {
                let url = format!("{path}?project={bad}");
                assert_eq!(
                    handle_orch_request(&url),
                    (400, "{\"error\":\"invalid project\"}".into()),
                    "{url}"
                );
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn agent_status_route_is_scoped_and_validates_agent_id() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = HarnessDirGuard::set(tmp.path());
        write_orch_run(tmp.path(), "proj-a", "run-a", "ag-1");
        write_orch_run(tmp.path(), "proj-b", "run-b", "ag-1");

        let (status, body) = handle_orch_request("/api/agents/ag-1/status?project=proj-b");
        assert_eq!(status, 200);
        assert!(body.contains("phase-proj-b"), "{body}");
        let (_, body) = handle_orch_request("/api/agents/ag-1/status?project=proj-a");
        assert!(body.contains("phase-proj-a"), "{body}");
        assert_eq!(
            handle_orch_request("/api/agents/ag-2/status?project=proj-a"),
            (200, "{}".into())
        );
        for id in ["..", "a%2Fb", "a.b"] {
            let url = format!("/api/agents/{id}/status?project=proj-a");
            assert_eq!(
                handle_orch_request(&url),
                (400, "{\"error\":\"invalid agent id\"}".into()),
                "{url}"
            );
        }
    }

    #[test]
    fn mutating_routes_require_loopback_host_and_origin() {
        let ok = [hdr("Host", "localhost:7700")];
        assert!(is_loopback_same_origin(&ok, 7700));
        let ok_ip = [
            hdr("host", "127.0.0.1:7700"),
            hdr("Origin", "http://127.0.0.1:7700"),
            hdr("Sec-Fetch-Site", "same-origin"),
        ];
        assert!(is_loopback_same_origin(&ok_ip, 7700));

        // DNS rebinding: loopback IP, attacker Host.
        assert!(!is_loopback_same_origin(
            &[hdr("Host", "evil.example:7700")],
            7700
        ));
        assert!(!is_loopback_same_origin(
            &[hdr("Host", "localhost:7701")],
            7700
        ));
        assert!(!is_loopback_same_origin(&[hdr("Host", "localhost")], 7700));
        assert!(!is_loopback_same_origin(&[], 7700));
        // Foreign origin, even with a good Host.
        for origin in ["http://evil.example", "http://localhost:5173", "null"] {
            let h = [hdr("Host", "localhost:7700"), hdr("Origin", origin)];
            assert!(!is_loopback_same_origin(&h, 7700), "origin {origin:?}");
        }
        let cross = [
            hdr("Host", "localhost:7700"),
            hdr("Sec-Fetch-Site", "cross-site"),
        ];
        assert!(!is_loopback_same_origin(&cross, 7700));
    }

    /// Raw HTTP/1.1 GET against a live `run_serve`; returns the status code.
    fn get_status(port: u16, path: &str, host: Option<&str>) -> u16 {
        use std::io::{Read, Write};
        let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let host_line = host.map(|h| format!("Host: {h}\r\n")).unwrap_or_default();
        write!(
            c,
            "GET {path} HTTP/1.1\r\n{host_line}Connection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = Vec::new();
        c.read_to_end(&mut out).unwrap();
        String::from_utf8_lossy(&out)
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn every_route_rejects_a_foreign_host() {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        std::thread::spawn(move || run_serve(Some(port)));
        for _ in 0..50 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Rejected before routing: reads, the SSE stream, and unknown paths.
        for path in [
            "/",
            "/api/graph",
            "/api/events",
            "/api/projects",
            "/api/harness?cmd=get_manifests",
        ] {
            for host in [
                Some("evil.example:1"),
                Some("localhost"),
                Some("localhost:1"),
                None,
            ] {
                assert_eq!(get_status(port, path, host), 403, "{path} {host:?}");
            }
        }
        // The dashboard page loads for both loopback names (case-insensitive).
        for host in [
            format!("localhost:{port}"),
            format!("127.0.0.1:{port}"),
            format!("LOCALHOST:{port}"),
        ] {
            assert_eq!(get_status(port, "/", Some(&host)), 200, "{host}");
        }
    }

    #[test]
    fn dismiss_orbit_pipeline_accepts_prefixed_id_and_removes_db_row() {
        use crate::store::orbit_store::{list_all_pipelines_pool, upsert_pipeline_pool};
        let tmp = tempfile::tempdir().unwrap();
        let a = write_sidecar(tmp.path(), "proj-a", "PIPELINE-20260101000000.json", "{}");
        let harness_dir = tmp.path().join("projects").join("proj-a");
        let pool = crate::store::runtime::block_on(async {
            let pool = crate::store::pool::test_memory_pool().await;
            crate::store::schema::init_schema_pool(&pool).await.unwrap();
            // One row keyed by the sidecar `id`, one by the file stem.
            for id in ["20260101000000", "PIPELINE-20260202000000"] {
                upsert_pipeline_pool(&pool, id, "proj-a", "running", None, None, "{}")
                    .await
                    .unwrap();
            }
            pool
        });
        let rows = || {
            crate::store::runtime::block_on(list_all_pipelines_pool(&pool))
                .unwrap()
                .len()
        };

        let (status, _) =
            dismiss_orbit_pipeline("PIPELINE-20260101000000", &harness_dir, Some(&pool));
        assert_eq!(status, 200);
        assert!(!a.exists(), "prefixed id still removes the sidecar");
        assert_eq!(rows(), 1, "the DB row for the same pipeline is gone");

        // DB-only row (no sidecar), bare id for a prefixed row.
        let (status, _) = dismiss_orbit_pipeline("20260202000000", &harness_dir, Some(&pool));
        assert_eq!(status, 200);
        assert_eq!(rows(), 0);

        let (status, _) = dismiss_orbit_pipeline("PIPELINE-", &harness_dir, Some(&pool));
        assert_eq!(status, 400);
    }
}
