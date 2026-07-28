//! serve.rs — Web dashboard for orchestration state and memory graph
//! Serves a single-page Svelte dashboard at http://localhost:{port}

use tiny_http::{Header, Method, Response, Server};

use crate::hooks::common;
use crate::mem::{graph, store};
use crate::orchestrate::state as orch;

const DEFAULT_PORT: u16 = 7700;
const MAX_DASHBOARD_PIPELINES: usize = 100;
const MAX_DASHBOARD_PIPELINE_PROJECTS: usize = 100;
const MAX_DASHBOARD_PIPELINE_FILES_PER_PROJECT: usize = 100;
const MAX_DASHBOARD_PIPELINE_BYTES: usize = 256 * 1024;
const MAX_DASHBOARD_ADAPTATION_RECORDS: i64 = 500;
const MAX_DASHBOARD_OBS_JSONL_FILES: usize = 100;
const MAX_DASHBOARD_OBS_JSONL_BYTES_PER_FILE: u64 = 512 * 1024;
const MAX_DASHBOARD_JSONL_TAIL_BYTES: u64 = 256 * 1024;
const MAX_DASHBOARD_SESSION_SNAPSHOTS: usize = 100;

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
    match crate::hooks::resume::start_dashboard_on_port(port, true) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
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
            (Method::Get, "/") | (Method::Get, "/index.html") => Response::from_string(
                dashboard_html_with_version(),
            )
            .with_header(Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap()),

            // ── Orchestration API ───────────────────────────
            (Method::Get, "/api/run") => {
                let body = if let Ok(Some(run)) = crate::store::runtime::block_on(async {
                    let pool = crate::store::pool::harness_pool().await?;
                    crate::store::orchestrator::read_run_pool(&pool).await
                }) {
                    serde_json::to_string(&run).unwrap_or_else(|_| "{}".into())
                } else {
                    let harness_dir = common::harness_dir();
                    match orch::read_run(&harness_dir) {
                        Some(run) => serde_json::to_string(&run).unwrap_or_else(|_| "{}".into()),
                        None => "{}".into(),
                    }
                };
                json_response(&body)
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
                let cmd = parse_query_param(url, "cmd").unwrap_or_default();
                // `project` selects a specific project slug; absent or
                // `__all__` means cross-project aggregate. The frontend's
                // projectArgs() omits the param entirely for "all".
                match selected_harness_dir(parse_query_param(url, "project")) {
                    Ok((project, harness_dir)) => {
                        let body = handle_harness_cmd(&cmd, &harness_dir, project.as_deref());
                        json_response(&body)
                    }
                    Err((status, body)) => json_response(&body).with_status_code(status),
                }
            }

            // ── Orbit Pipeline Dismiss ───────────────────────
            (Method::Delete, url) if url.starts_with("/api/orbit/") => {
                match parse_orbit_dismiss_request(url) {
                    Ok((pipeline_id, project)) => match selected_harness_dir(Some(project.clone()))
                    {
                        Ok((Some(_), harness_dir)) => {
                            match dismiss_orbit_pipeline(&pipeline_id, &project, &harness_dir) {
                                Ok(body) => json_response(&body),
                                Err((status, body)) => {
                                    json_response(&body).with_status_code(status)
                                }
                            }
                        }
                        Ok((None, _)) => {
                            json_response("{\"error\":\"a concrete project is required\"}")
                                .with_status_code(400)
                        }
                        Err((status, body)) => json_response(&body).with_status_code(status),
                    },
                    Err(body) => json_response(&body).with_status_code(400),
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

            // ── Agents API ───────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/agents/") && url.ends_with("/status") => {
                let agent_id = url
                    .trim_start_matches("/api/agents/")
                    .trim_end_matches("/status");
                if !orch::validate_agent_id(agent_id) {
                    json_response("{\"error\":\"invalid agent id\"}").with_status_code(400)
                } else if let Ok(Some(a)) = crate::store::runtime::block_on(async {
                    let pool = crate::store::pool::harness_pool().await?;
                    crate::store::orchestrator::read_agent_pool(&pool, agent_id).await
                }) {
                    let body = serde_json::json!({
                        "agent_id": a.id,
                        "phase": a.phase,
                        "progress": a.progress,
                        "last_heartbeat": a.last_heartbeat,
                        "status": a.status,
                    })
                    .to_string();
                    json_response(&body)
                } else {
                    let harness_dir = common::harness_dir();
                    let body = match orch::read_agent_status(&harness_dir, agent_id) {
                        Some(s) => serde_json::to_string(&s).unwrap_or_else(|_| "{}".into()),
                        None => "{}".into(),
                    };
                    json_response(&body)
                }
            }

            (Method::Delete, url) if url.starts_with("/api/agents/") => {
                let agent_id = url.trim_start_matches("/api/agents/").trim_end_matches('/');
                let harness_dir = common::harness_dir();
                if !orch::validate_agent_id(agent_id) {
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

fn selected_harness_dir(
    requested_project: Option<String>,
) -> Result<(Option<String>, std::path::PathBuf), (u16, String)> {
    match requested_project {
        None => Ok((None, common::harness_dir())),
        Some(project) if project == "__all__" => Ok((None, common::harness_dir())),
        Some(project) if project.is_empty() => {
            Err((400, "{\"error\":\"project must not be empty\"}".into()))
        }
        Some(project) => crate::shared::paths::resolve_external_harness_dir(&project)
            .map(|dir| (Some(project), dir))
            .map_err(|error| {
                let status = if error.kind() == std::io::ErrorKind::NotFound {
                    404
                } else {
                    400
                };
                let message = if status == 404 {
                    "project not found"
                } else {
                    "invalid project"
                };
                (status, serde_json::json!({"error": message}).to_string())
            }),
    }
}

fn parse_orbit_dismiss_request(url: &str) -> Result<(String, String), String> {
    let path = url.split('?').next().unwrap_or(url);
    let pipeline_id = path
        .strip_prefix("/api/orbit/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or_else(|| "{\"error\":\"an exact pipeline id is required\"}".to_string())?;
    let project = parse_query_param(url, "project")
        .filter(|project| !project.is_empty() && project != "__all__")
        .ok_or_else(|| "{\"error\":\"a concrete project is required\"}".to_string())?;
    Ok((pipeline_id.to_string(), project))
}

/// Dismiss one exact pipeline from the selected project's file and SQLite state.
fn dismiss_orbit_pipeline(
    pipeline_id: &str,
    project: &str,
    harness_dir: &std::path::Path,
) -> Result<String, (u16, String)> {
    let result = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await?;
        crate::store::orbit_store::dismiss_pipeline_state_pool(
            &pool,
            project,
            pipeline_id,
            harness_dir,
        )
        .await
    });
    match result {
        Ok(dismissed) if dismissed.deleted_file || dismissed.deleted_row => Ok(serde_json::json!({
            "ok": true,
            "dismissed": pipeline_id,
            "project": project,
        })
        .to_string()),
        Ok(_) => Err((
            404,
            "{\"ok\":false,\"error\":\"pipeline not found\"}".into(),
        )),
        Err(error) => {
            eprintln!("[serve] failed to dismiss {project}/{pipeline_id}: {error}");
            Err((
                500,
                "{\"ok\":false,\"error\":\"pipeline dismissal failed\"}".into(),
            ))
        }
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

/// Read the newest JSONL entries from a bounded byte tail. The dashboard uses
/// this for append-only ledgers; it never materializes an entire history file.
fn read_jsonl_tail(
    path: &std::path::Path,
    max_items: usize,
) -> std::io::Result<Vec<serde_json::Value>> {
    use std::collections::VecDeque;
    use std::io::{BufRead, BufReader, Seek, SeekFrom};

    let mut file = std::fs::File::open(path)?;
    let start = file
        .metadata()?
        .len()
        .saturating_sub(MAX_DASHBOARD_JSONL_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut reader = BufReader::new(file);
    if start > 0 {
        let mut partial = String::new();
        reader.read_line(&mut partial)?;
    }
    let mut items = VecDeque::with_capacity(max_items.saturating_add(1));
    for line in reader.lines() {
        let line = line?;
        if let Ok(value) = serde_json::from_str(&line) {
            items.push_back(value);
            if items.len() > max_items {
                items.pop_front();
            }
        }
    }
    Ok(items.into_iter().rev().collect())
}

/// Read one dashboard JSON document with a hard byte limit.
fn read_limited_text(path: &std::path::Path, max_bytes: usize) -> std::io::Result<String> {
    use std::io::Read;

    let mut text = String::new();
    std::fs::File::open(path)?
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_string(&mut text)?;
    if text.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "dashboard file exceeds {max_bytes} bytes: {}",
                path.display()
            ),
        ));
    }
    Ok(text)
}

fn list_session_snapshot_names(sessions_dir: &std::path::Path) -> Vec<String> {
    let mut snapshots = std::fs::read_dir(sessions_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .file_type()
                        .map(|kind| kind.is_file())
                        .unwrap_or(false)
                })
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name.ends_with(".json"))
                .take(MAX_DASHBOARD_SESSION_SNAPSHOTS)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    snapshots.sort();
    snapshots
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
                    let mut v: Vec<_> = stats.tool_stats.iter().map(|t| {
                                serde_json::json!({
                                    "tool": t.tool,
                                    "calls": t.calls,
                                    "unknowns": t.unknowns,
                                    // Denominator excludes calls with no outcome evidence.
                                    "success_rate": if t.evaluated() > 0 {
                                        (t.successes as f64 / t.evaluated() as f64 * 1000.0).round() / 1000.0
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
                        .take(MAX_DASHBOARD_OBS_JSONL_FILES)
                        .collect()
                })
                .unwrap_or_default();
            files.sort_by_key(|e| e.file_name());

            for entry in files.iter().rev() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let session_key = fname.trim_end_matches(".jsonl").to_string();
                let date = fname.split('_').nth(1).unwrap_or("unknown").to_string();
                let sess = session_map
                    .entry(session_key.clone())
                    .or_insert((0, 0.0, 0, date));

                if let Ok(file) = fs::File::open(entry.path()) {
                    use std::io::{BufRead, Read};
                    let reader =
                        std::io::BufReader::new(file.take(MAX_DASHBOARD_OBS_JSONL_BYTES_PER_FILE));
                    for line in reader
                        .lines()
                        .map_while(Result::ok)
                        .filter(|l| !l.is_empty())
                    {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
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
        "get_orbit_pipelines" => {
            // Try SQLite first
            if let Ok(pipelines) = crate::store::runtime::block_on(async {
                let pool = crate::store::pool::harness_pool().await?;
                dashboard_pipelines_from_store(&pool, project).await
            }) {
                let scoped = crate::shared::orbit::dashboard_pipelines_for_project(
                    pipelines,
                    project,
                    MAX_DASHBOARD_PIPELINES,
                );
                if !scoped.is_empty() {
                    return serde_json::to_string(&scoped).unwrap_or_else(|_| "[]".into());
                }
            }
            // Fallback: scan PIPELINE-*.json files
            let mut all: Vec<serde_json::Value> = vec![];
            let project_dirs: Vec<(String, std::path::PathBuf)> = match project {
                Some(slug) => vec![(slug.to_string(), harness_dir.to_path_buf())],
                None => {
                    let projects_root = harness_dir.parent().unwrap_or(harness_dir);
                    fs::read_dir(projects_root)
                        .into_iter()
                        .flatten()
                        .filter_map(Result::ok)
                        .filter(|entry| {
                            entry
                                .file_type()
                                .map(|file_type| file_type.is_dir())
                                .unwrap_or(false)
                        })
                        .map(|entry| {
                            (
                                entry.file_name().to_string_lossy().to_string(),
                                entry.path(),
                            )
                        })
                        .take(MAX_DASHBOARD_PIPELINE_PROJECTS)
                        .collect()
                }
            };
            'projects: for (slug, project_dir) in project_dirs {
                let orbit_dir = project_dir.join("orbit");
                if !orbit_dir.is_dir() {
                    continue;
                }
                if let Ok(files) = fs::read_dir(&orbit_dir) {
                    for file in files
                        .filter_map(Result::ok)
                        .filter(|file| {
                            let name = file.file_name();
                            let name = name.to_string_lossy();
                            name.starts_with("PIPELINE-")
                                && name.ends_with(".json")
                                && file
                                    .file_type()
                                    .map(|file_type| file_type.is_file())
                                    .unwrap_or(false)
                        })
                        .take(MAX_DASHBOARD_PIPELINE_FILES_PER_PROJECT)
                    {
                        if all.len() >= MAX_DASHBOARD_PIPELINES {
                            break 'projects;
                        }
                        if let Ok(content) =
                            read_limited_text(&file.path(), MAX_DASHBOARD_PIPELINE_BYTES)
                            && let Ok(mut pipeline) =
                                serde_json::from_str::<serde_json::Value>(&content)
                        {
                            pipeline["_project"] = serde_json::Value::String(slug.clone());
                            all.push(pipeline);
                        }
                    }
                }
            }
            let scoped = crate::shared::orbit::dashboard_pipelines_for_project(
                all,
                project,
                MAX_DASHBOARD_PIPELINES,
            );
            serde_json::to_string(&scoped).unwrap_or_else(|_| "[]".into())
        }
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
                crate::store::evolution::query_recent_records_scoped_pool(
                    &pool,
                    MAX_DASHBOARD_ADAPTATION_RECORDS,
                    project,
                )
                .await
            })
            .unwrap_or_default();
            let landscape = crate::evolve::planner::build_landscape(&history, &[], 2);
            serde_json::to_string(&landscape).unwrap_or_else(|_| "null".into())
        }
        "get_manifests" => {
            // Tail-read the falsifiability ledger sidecar (manifests.jsonl).
            // Cap at the most recent 50 to bound payload size.
            let path = crate::shared::paths::manifests_file_for(project);
            let items = read_jsonl_tail(&path, 50).unwrap_or_default();
            serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
        }
        // ── Previously-broken panels (returned "null" via the fallthrough) ─
        "get_session_snapshots" => {
            // List session snapshot files in the project's sessions/ dir.
            let snaps = list_session_snapshot_names(&harness_dir.join("sessions"));
            serde_json::to_string(&snaps).unwrap_or_else(|_| "[]".into())
        }
        "get_global_patterns" => {
            // Cross-project global patterns (opt-in feature). Parse the JSONL
            // ledger line-by-line into a JSON array; raw text would not parse
            // as a single JSON value on the TS client. Cap at recent 50.
            let path = crate::shared::paths::global_patterns_file();
            let items = read_jsonl_tail(&path, 50).unwrap_or_default();
            serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
        }
        "get_effect_pending" => {
            // Whether a cross-project effect export is pending this session.
            // We approximate: present if the marker file exists.
            let marker = harness_dir.join(".cross-project-enabled");
            serde_json::json!({ "enabled": marker.exists() }).to_string()
        }
        _ => "null".into(),
    }
}

async fn dashboard_pipelines_from_store(
    pool: &sqlx::AnyPool,
    project: Option<&str>,
) -> std::io::Result<Vec<serde_json::Value>> {
    crate::store::orbit_store::list_pipelines_scoped_pool(
        pool,
        project,
        MAX_DASHBOARD_PIPELINES as i64,
    )
    .await
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

    #[test]
    fn selected_project_orbit_fallback_does_not_scan_sibling_projects() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected-fallback-project");
        let sibling = root.path().join("sibling-fallback-project");
        std::fs::create_dir_all(selected.join("orbit")).unwrap();
        std::fs::create_dir_all(sibling.join("orbit")).unwrap();
        std::fs::write(
            selected.join("orbit/PIPELINE-selected.json"),
            r#"{"id":"selected","status":"running","started_at":"2"}"#,
        )
        .unwrap();
        std::fs::write(
            sibling.join("orbit/PIPELINE-sibling.json"),
            r#"{"id":"sibling","status":"running","started_at":"3"}"#,
        )
        .unwrap();

        let response = handle_harness_cmd(
            "get_orbit_pipelines",
            &selected,
            Some("selected-fallback-project"),
        );
        let pipelines: Vec<serde_json::Value> = serde_json::from_str(&response).unwrap();

        assert_eq!(pipelines.len(), 1);
        assert_eq!(pipelines[0]["id"], "selected");
        assert_eq!(pipelines[0]["_project"], "selected-fallback-project");
    }

    #[tokio::test]
    async fn dashboard_pipeline_store_query_is_scoped_and_bounded() {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
        for n in 0..=MAX_DASHBOARD_PIPELINES {
            crate::store::orbit_store::upsert_pipeline_pool(
                &pool,
                &format!("PIPELINE-selected-{n}"),
                "selected",
                "complete",
                None,
                None,
                "{}",
            )
            .await
            .unwrap();
        }
        crate::store::orbit_store::upsert_pipeline_pool(
            &pool,
            "PIPELINE-sibling",
            "sibling",
            "complete",
            None,
            None,
            "{}",
        )
        .await
        .unwrap();

        let pipelines = dashboard_pipelines_from_store(&pool, Some("selected"))
            .await
            .unwrap();

        assert_eq!(pipelines.len(), MAX_DASHBOARD_PIPELINES);
        assert!(
            pipelines
                .iter()
                .all(|pipeline| pipeline["project"] == "selected")
        );
    }

    #[test]
    fn orbit_dismiss_request_requires_exact_project_and_pipeline_identity() {
        assert!(parse_orbit_dismiss_request("/api/orbit/PIPELINE-123").is_err());
        assert!(parse_orbit_dismiss_request("/api/orbit/?project=project-a").is_err());
        assert!(
            parse_orbit_dismiss_request("/api/orbit/PIPELINE-123/extra?project=project-a").is_err()
        );

        assert_eq!(
            parse_orbit_dismiss_request("/api/orbit/PIPELINE-123?project=project-a").unwrap(),
            ("PIPELINE-123".to_string(), "project-a".to_string())
        );
    }

    #[test]
    fn external_project_resolution_rejects_empty_traversal_and_unknown_slugs() {
        assert_eq!(
            selected_harness_dir(Some(String::new())).unwrap_err().0,
            400
        );
        assert_eq!(
            selected_harness_dir(Some("../outside-project".into()))
                .unwrap_err()
                .0,
            400
        );
        assert_eq!(
            selected_harness_dir(Some("definitely-not-a-harness-project".into()))
                .unwrap_err()
                .0,
            404
        );
    }

    #[test]
    fn jsonl_tail_returns_only_the_newest_bounded_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(&path, "{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n").unwrap();

        let records = read_jsonl_tail(&path, 2).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["n"], 3);
        assert_eq!(records[1]["n"], 2);
    }

    #[test]
    fn limited_json_read_rejects_oversized_pipeline_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("PIPELINE-too-large.json");
        std::fs::write(&path, "x".repeat(MAX_DASHBOARD_PIPELINE_BYTES + 1)).unwrap();

        assert!(read_limited_text(&path, MAX_DASHBOARD_PIPELINE_BYTES).is_err());
    }

    #[test]
    fn session_snapshot_listing_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..=MAX_DASHBOARD_SESSION_SNAPSHOTS {
            std::fs::write(dir.path().join(format!("session-{n}.json")), "{}").unwrap();
        }

        assert_eq!(
            list_session_snapshot_names(dir.path()).len(),
            MAX_DASHBOARD_SESSION_SNAPSHOTS
        );
    }
}
