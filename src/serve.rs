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

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();

        let response = match (method, url.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => Response::from_string(
                DASHBOARD_HTML,
            )
            .with_header(Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap()),

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

            // ── Harness API ──────────────────────────────────
            (Method::Get, url) if url.starts_with("/api/harness") => {
                let cmd = parse_query_param(url, "cmd").unwrap_or_default();
                let harness_dir = common::harness_dir();
                let body = handle_harness_cmd(&cmd, &harness_dir);
                json_response(&body)
            }

            // ── Memory API (Nodes) ───────────────────────────
            (Method::Get, "/api/nodes") => {
                let nodes =
                    store::read_all_nodes_conn(&store::open_db().unwrap()).unwrap_or_default();
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
            (Method::Options, _) => Response::from_string("{}")
                .with_status_code(204)
                .with_header(Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap())
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

            // ── SPA fallback: serve index.html for any non-API GET route ──
            (Method::Get, _) => Response::from_string(DASHBOARD_HTML).with_header(
                Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
            ),

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

fn handle_harness_cmd(cmd: &str, harness_dir: &std::path::Path) -> String {
    use std::fs;
    match cmd {
        "get_harness_metrics" => {
            let p = harness_dir.join("metrics.json");
            fs::read_to_string(&p).unwrap_or_else(|_| "null".into())
        }
        "get_evolved_skills" => {
            let evolved_dir = harness_dir.join("evolved");
            let skills: Vec<serde_json::Value> = if evolved_dir.exists() {
                fs::read_dir(&evolved_dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .filter(|e| e.path().is_dir())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                let skill_md_path = e.path().join("SKILL.md");
                                let skill_md =
                                    fs::read_to_string(&skill_md_path).unwrap_or_default();
                                serde_json::json!({
                                    "name": name,
                                    "skill_md": skill_md,
                                    "created_at": null
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            };
            let evo_log = harness_dir.join("evolution.jsonl");
            let history: Vec<serde_json::Value> = if evo_log.exists() {
                fs::read_to_string(&evo_log)
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| !l.is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect::<Vec<serde_json::Value>>()
                    .into_iter()
                    .rev()
                    .take(50)
                    .rev()
                    .collect()
            } else {
                vec![]
            };
            let metrics_path = harness_dir.join("metrics.json");
            let total_sessions: u64 = fs::read_to_string(&metrics_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v["total_sessions"].as_u64())
                .unwrap_or(0);
            let result = serde_json::json!({
                "evolved_skills": skills,
                "evolution_history": history,
                "total_sessions_analyzed": total_sessions,
                "patterns_detected": history.len()
            });
            result.to_string()
        }
        "get_obs_summary" => {
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
                let date = fname.split('_').nth(1).unwrap_or("unknown").to_string();
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
                v.into_iter()
                    .take(10)
                    .map(|(sid, (calls, score_sum, failures, date))| {
                        serde_json::json!({
                            "session_id": sid,
                            "date": date,
                            "tool_calls": calls,
                            "avg_score": if *calls > 0 {
                                (*score_sum / *calls as f64 * 1000.0).round() / 1000.0
                            } else { 0.0 },
                            "failures": failures
                        })
                    })
                    .collect()
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
            let projects_root = harness_dir.parent().unwrap_or(harness_dir);
            let mut all: Vec<serde_json::Value> = vec![];
            if let Ok(rd) = fs::read_dir(projects_root) {
                for proj_entry in rd.filter_map(|e| e.ok()) {
                    let orbit_dir = proj_entry.path().join("orbit");
                    if !orbit_dir.exists() {
                        continue;
                    }
                    if let Ok(files) = fs::read_dir(&orbit_dir) {
                        for f in files.filter_map(|e| e.ok()) {
                            let fname = f.file_name().to_string_lossy().to_string();
                            if fname.starts_with("PIPELINE-")
                                && fname.ends_with(".json")
                                && let Ok(content) = fs::read_to_string(f.path())
                                && let Ok(mut v) =
                                    serde_json::from_str::<serde_json::Value>(&content)
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
        "get_integration_status" => {
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
        "get_graph" => {
            graph::rebuild_graph_json().unwrap_or_else(|_| r#"{"nodes":[],"edges":[]}"#.into())
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
}
