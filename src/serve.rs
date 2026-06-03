//! serve.rs — Web dashboard for orchestration state and memory graph
//! Serves a single-page Svelte dashboard at http://localhost:{port}

use tiny_http::{Header, Method, Response, Server};

use crate::hooks::common;
use crate::mem::{graph, store};
use crate::orchestrate::state as orch;

const DEFAULT_PORT: u16 = 7700;

// HTML is embedded at compile time from assets/dashboard.html
static DASHBOARD_HTML: &str = include_str!("../assets/dashboard.html");

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

    // Open DB once and reuse across all requests (single-threaded server).
    // Avoids per-request schema init + migration check overhead.
    // Cross-session / cross-hook concurrency is handled at the SQLite layer:
    // hooks run as separate processes each with their own connection, WAL mode
    // allows concurrent reads without blocking, and busy_timeout=5000ms serializes
    // concurrent writers. No thread-level sharing is needed here.
    let db = crate::store::open_harness_db().ok();
    let harness_dir = common::harness_dir();

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();

        let response = match (method, url.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => Response::from_string(
                DASHBOARD_HTML,
            )
            .with_header(Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap())
            .with_header(Header::from_bytes(b"Cache-Control", b"no-cache, no-store, must-revalidate").unwrap()),

            // ── Orchestration API ───────────────────────────
            (Method::Get, "/api/run") => handle_get_run(db.as_ref(), &harness_dir),
            (Method::Get, "/api/events") => handle_get_events(db.as_ref(), &harness_dir),

            // ── Memory API (Graph/Stats) ─────────────────────
            (Method::Get, "/api/graph") => handle_get_graph(),
            (Method::Get, "/api/stats") => handle_get_stats(),

            // ── Harness API ──────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/harness") => {
                let cmd = parse_query_param(url, "cmd").unwrap_or_default();
                let body = handle_harness_cmd(db.as_ref(), &cmd, &harness_dir);
                json_response(&body)
            }

            // ── Orbit Pipeline Dismiss ───────────────────────
            (Method::Delete, url) if url.starts_with("/api/orbit/") => {
                let pipeline_id = url.trim_start_matches("/api/orbit/").trim_end_matches('/');
                let body = dismiss_orbit_pipeline(db.as_ref(), pipeline_id, &harness_dir);
                json_response(&body)
            }

            // ── Memory API (Nodes) ───────────────────────────
            (Method::Get, u) if u == "/api/nodes" || u.starts_with("/api/nodes?") => {
                handle_list_nodes(u)
            }

            (Method::Get, url) if url.starts_with("/api/nodes/") => handle_get_node(url),

            // ── Memory API (Search) ──────────────────────────
            (Method::Get, url) if url.starts_with("/api/search") => handle_search(url),

            // ── Agents API ───────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/agents/") && url.ends_with("/status") => {
                let agent_id = url
                    .trim_start_matches("/api/agents/")
                    .trim_end_matches("/status");
                handle_agent_status(db.as_ref(), agent_id, &harness_dir)
            }

            (Method::Delete, url) if url.starts_with("/api/agents/") => {
                let agent_id = url.trim_start_matches("/api/agents/").trim_end_matches('/');
                handle_agent_dismiss(db.as_ref(), agent_id, &harness_dir)
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

// ── Route handlers ────────────────────────────────────

/// Try a DB operation with a cached connection. Logs errors and returns None on failure.
/// Replaces per-request `crate::store::with_harness_db()` calls with the pre-opened connection.
fn try_db<T>(
    db: Option<&rusqlite::Connection>,
    f: impl FnOnce(&rusqlite::Connection) -> std::io::Result<T>,
) -> Option<T> {
    db.and_then(|conn| match f(conn) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[serve] query failed: {e}");
            None
        }
    })
}

fn handle_get_run(
    db: Option<&rusqlite::Connection>,
    harness_dir: &std::path::Path,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let db_ok = try_db(db, |conn| {
        crate::store::orchestrator::read_run_conn(conn).map(|opt| {
            opt.as_ref()
                .map(|r| json_or(r, "{}"))
                .unwrap_or_else(|| "{}".into())
        })
    });
    let body = match db_ok {
        Some(body) => body,
        None => {
            let body: serde_json::Value = orch::read_run(harness_dir)
                .as_ref()
                .map(|r| serde_json::to_value(r).unwrap_or_default())
                .unwrap_or_default();
            body.to_string()
        }
    };
    json_response(&body)
}

fn handle_get_events(
    db: Option<&rusqlite::Connection>,
    harness_dir: &std::path::Path,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let db_ok = try_db(db, |conn| {
        crate::store::orchestrator::read_run_conn(conn).map(|opt| {
            opt.as_ref()
                .map(|r| json_or(r, "null"))
                .unwrap_or_else(|| "null".into())
        })
    });
    let data = match db_ok {
        Some(data) => data,
        None => json_or(&orch::read_run(harness_dir), "null"),
    };
    let sse_body = format!("data: {data}\n\n");
    Response::from_string(sse_body)
        .with_header(Header::from_bytes(b"Content-Type", b"text/event-stream").unwrap())
        .with_header(Header::from_bytes(b"Cache-Control", b"no-cache").unwrap())
        .with_header(Header::from_bytes(b"Connection", b"keep-alive").unwrap())
}

fn handle_get_graph() -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match graph::rebuild_graph_json() {
        Ok(json) => json,
        Err(_) => "{}".into(),
    };
    json_response(&body)
}

fn handle_get_stats() -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match graph::compute_stats() {
        Ok(v) => v.to_string(),
        Err(_) => "{}".into(),
    };
    json_response(&body)
}

fn handle_list_nodes(url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let limit: usize = parse_query_param(url, "limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
        .min(1000);
    // Note: uses store::open_db() (memory.db for the knowledge graph),
    // not the harness operational DB passed as `db` to other handlers.
    let nodes = match store::open_db() {
        Ok(conn) => store::read_nodes_limited_conn(&conn, limit).unwrap_or_default(),
        Err(e) => {
            eprintln!("[serve] failed to open memory DB for /api/nodes: {e}");
            Vec::new()
        }
    };
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

fn handle_get_node(url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
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

fn handle_search(url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let query = url
        .split("q=")
        .nth(1)
        .unwrap_or("")
        .split('&')
        .next()
        .unwrap_or("");
    let decoded = percent_decode(query);
    let nodes = store::search_nodes(&decoded, 20);
    let results: Vec<serde_json::Value> = nodes
        .into_iter()
        .map(|n| {
            serde_json::json!({
                "id": n.frontmatter.id,
                "title": n.frontmatter.title,
                "type": n.frontmatter.node_type,
                "snippet": n.body.chars().take(160).collect::<String>()
            })
        })
        .collect();
    let body = serde_json::to_string(&results).unwrap_or_else(|_| "[]".into());
    json_response(&body)
}

fn handle_agent_status(
    db: Option<&rusqlite::Connection>,
    agent_id: &str,
    harness_dir: &std::path::Path,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if !orch::validate_agent_id(agent_id) {
        return json_response("{\"error\":\"invalid agent id\"}").with_status_code(400);
    }
    try_db(db, |conn| {
        crate::store::orchestrator::read_agent_conn(conn, agent_id).map(|opt| {
            opt.map(|a| {
                serde_json::json!({
                    "agent_id": a.id,
                    "phase": a.phase,
                    "progress": a.progress,
                    "last_heartbeat": a.last_heartbeat,
                    "status": a.status,
                })
                .to_string()
            })
            .unwrap_or_else(|| "{}".into())
        })
    })
    .map(|body| json_response(&body))
    .unwrap_or_else(|| {
        let body: serde_json::Value = orch::read_agent_status(harness_dir, agent_id)
            .map(|s| serde_json::to_value(&s).unwrap_or_default())
            .unwrap_or_default();
        json_response(&body.to_string())
    })
}

fn handle_agent_dismiss(
    db: Option<&rusqlite::Connection>,
    agent_id: &str,
    harness_dir: &std::path::Path,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if !orch::validate_agent_id(agent_id) {
        return json_response("{\"error\":\"invalid agent id\"}").with_status_code(400);
    }
    // SQLite-first: try DB dismiss, fall back to file-based if DB unavailable
    let db_ok = try_db(db, |conn| {
        crate::store::orchestrator::dismiss_agent_conn(conn, agent_id)
    });
    let ok = db_ok.unwrap_or_else(|| orch::dismiss_agent(harness_dir, agent_id));
    let body = serde_json::json!({"ok": ok, "dismissed": agent_id});
    json_response(&body.to_string())
}

// ── Response helpers ──────────────────────────────────

fn json_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(Header::from_bytes(b"Content-Type", b"application/json").unwrap())
        .with_header(
            Header::from_bytes(b"Access-Control-Allow-Origin", b"http://localhost:5173").unwrap(),
        )
}

/// Serialize a value to JSON, or return the fallback string on failure.
fn json_or<T: serde::Serialize>(val: &T, fallback: &str) -> String {
    serde_json::to_string(val).unwrap_or_else(|_| fallback.into())
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

/// Dismiss (delete) an orbit pipeline across SQLite and the filesystem.
///
/// SQLite-first: deletes from `orbit_pipelines` table when DB is available.
/// Falls back to scanning PIPELINE-*.json files in project orbit directories.
fn dismiss_orbit_pipeline(
    db: Option<&rusqlite::Connection>,
    pipeline_id: &str,
    harness_dir: &std::path::Path,
) -> String {
    // Validate pipeline_id to prevent unintended file matches.
    // Expected format: alphanumeric + hyphens (e.g. "20260523105350" or "20260522-170016").
    if pipeline_id.is_empty()
        || !pipeline_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return serde_json::json!({"ok": false, "error": "invalid pipeline id"}).to_string();
    }

    // SQLite-first: remove the DB record
    let db_deleted = try_db(db, |conn| {
        crate::store::orbit_store::dismiss_pipeline_conn(conn, pipeline_id)
    })
    .unwrap_or(false);

    // File-based: remove matching PIPELINE-*.json files across all project dirs
    let projects_root = harness_dir.parent().unwrap_or(harness_dir);
    let mut file_deleted = false;

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
                        && fname.contains(pipeline_id)
                        && std::fs::remove_file(f.path()).is_ok()
                    {
                        file_deleted = true;
                    }
                }
            }
        }
    }

    if db_deleted || file_deleted {
        serde_json::json!({"ok": true, "dismissed": pipeline_id}).to_string()
    } else {
        serde_json::json!({"ok": false, "error": "pipeline not found"}).to_string()
    }
}

// ── Harness command handler ───────────────────────────

fn handle_harness_cmd(
    db: Option<&rusqlite::Connection>,
    cmd: &str,
    harness_dir: &std::path::Path,
) -> String {
    match cmd {
        "get_harness_metrics" => cmd_get_metrics(db, harness_dir),
        "get_evolved_skills" => cmd_get_evolved_skills(db, harness_dir),
        "get_obs_summary" => cmd_get_obs_summary(db, harness_dir),
        "get_orbit_pipelines" => cmd_get_orbit_pipelines(db, harness_dir),
        "get_integration_status" => cmd_get_integration_status(),
        "get_graph" => {
            graph::rebuild_graph_json().unwrap_or_else(|_| r#"{"nodes":[],"edges":[]}"#.into())
        }
        _ => "null".into(),
    }
}

fn cmd_get_metrics(db: Option<&rusqlite::Connection>, harness_dir: &std::path::Path) -> String {
    try_db(db, |conn| {
        crate::store::metrics::load_metrics_conn(conn)
            .map(|m| serde_json::to_string(&m).unwrap_or_else(|_| "null".into()))
    })
    .unwrap_or_else(|| {
        let p = harness_dir.join("metrics.json");
        std::fs::read_to_string(&p).unwrap_or_else(|_| "null".into())
    })
}

fn cmd_get_evolved_skills(
    db: Option<&rusqlite::Connection>,
    harness_dir: &std::path::Path,
) -> String {
    // Try SQLite first
    if let Some(result) = try_db(db, |conn| {
        let skills = crate::store::evolved::list_skills_full_conn(conn)
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
        let history = crate::store::evolution::query_recent_records_conn(conn, 50)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect::<Vec<_>>();
        let total_sessions = crate::store::metrics::load_metrics_conn(conn)
            .map(|m| m.total_sessions)
            .unwrap_or(0);
        if !skills.is_empty() || !history.is_empty() {
            Ok(serde_json::json!({
                "evolved_skills": skills,
                "evolution_history": history,
                "total_sessions_analyzed": total_sessions,
                "patterns_detected": history.len()
            })
            .to_string())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "empty"))
        }
    }) {
        return result;
    }

    // Fallback: file-based reading
    let evolved_dir = harness_dir.join("evolved");
    let skills: Vec<serde_json::Value> = if evolved_dir.exists() {
        std::fs::read_dir(&evolved_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let skill_md_path = e.path().join("SKILL.md");
                        let skill_md = std::fs::read_to_string(&skill_md_path).unwrap_or_default();
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
        if let Ok(file) = std::fs::File::open(&evo_log) {
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
    let total_sessions: u64 = std::fs::read_to_string(&metrics_path)
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

fn cmd_get_obs_summary(db: Option<&rusqlite::Connection>, harness_dir: &std::path::Path) -> String {
    // Try SQLite first
    if let Some(result) =
        try_db(db, |conn| {
            let stats = crate::store::observations::query_obs_stats_conn(
                conn,
                "2020-01-01", // all data
                "2099-12-31",
            )?;
            if stats.total > 0 {
                let tool_stats: Vec<serde_json::Value> =
                    {
                        let mut v: Vec<_> = stats.tool_stats.iter().map(|t| {
                    serde_json::json!({
                        "tool": t.tool,
                        "calls": t.calls,
                        "success_rate": if t.calls > 0 {
                            (t.successes as f64 / t.calls as f64 * 1000.0).round() / 1000.0
                        } else { 0.0 },
                        "avg_score": (t.avg_score * 1000.0).round() / 1000.0
                    })
                }).collect();
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
                        let date = s
                            .session_id
                            .split('_')
                            .next()
                            .unwrap_or("unknown")
                            .to_string();
                        serde_json::json!({
                            "session_id": s.session_id,
                            "date": date,
                            "tool_calls": s.calls,
                            "avg_score": (s.avg_score * 1000.0).round() / 1000.0,
                            "failures": s.failures
                        })
                    })
                    .collect();
                return Ok(serde_json::json!({
                    "recent_sessions": recent_sessions,
                    "tool_stats": tool_stats,
                    "total_tool_calls": stats.total,
                    "avg_score": (stats.avg_score * 1000.0).round() / 1000.0,
                    "active_agents": []
                })
                .to_string());
            }
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "empty"))
        })
    {
        return result;
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

    let mut files: Vec<_> = std::fs::read_dir(&obs_dir)
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
        let date = fname.split('_').nth(1).unwrap_or("unknown").to_string();
        let sess = session_map
            .entry(session_key.clone())
            .or_insert((0, 0.0, 0, date));

        if let Ok(content) = std::fs::read_to_string(entry.path()) {
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
            .map(|t| t["avg_score"].as_f64().unwrap_or(0.0) * t["calls"].as_f64().unwrap_or(0.0))
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

fn cmd_get_orbit_pipelines(
    db: Option<&rusqlite::Connection>,
    harness_dir: &std::path::Path,
) -> String {
    // Try SQLite first
    if let Some(result) = try_db(db, |conn| {
        let pipelines = crate::store::orbit_store::list_all_pipelines_conn(conn)?;
        if !pipelines.is_empty() {
            let mut sorted = pipelines;
            sorted.sort_by(|a, b| {
                let ta = a["started_at"].as_str().unwrap_or("");
                let tb = b["started_at"].as_str().unwrap_or("");
                tb.cmp(ta)
            });
            Ok(serde_json::to_string(&sorted).unwrap_or_else(|_| "[]".into()))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "empty"))
        }
    }) {
        return result;
    }
    // Fallback: scan PIPELINE-*.json files
    let projects_root = harness_dir.parent().unwrap_or(harness_dir);
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
                        && let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&content)
                    {
                        v["_project"] = serde_json::Value::String(
                            proj_entry.file_name().to_string_lossy().to_string(),
                        );
                        all.push(v);
                    }
                }
            }
        }
    }
    all.sort_by(|a, b| {
        let ta = a["started_at"].as_str().unwrap_or("");
        let tb = b["started_at"].as_str().unwrap_or("");
        tb.cmp(ta)
    });
    serde_json::to_string(&all).unwrap_or_else(|_| "[]".into())
}

fn cmd_get_integration_status() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
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
        { "name": "Codex",  "installed": false, "config_path": null, "version": null },
        { "name": "Cursor", "installed": false, "config_path": null, "version": null },
        { "name": "Cline",  "installed": false, "config_path": null, "version": null },
        { "name": "Aider",  "installed": false, "config_path": null, "version": null }
    ]);
    integrations.to_string()
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
