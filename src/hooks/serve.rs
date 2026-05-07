//! serve.rs — Web dashboard for orchestration state
//! Serves a single-page D3.js dashboard at http://localhost:{port}

use tiny_http::{Response, Server};

use super::common;
use crate::orchestrate::state as orch;

const DEFAULT_PORT: u16 = 7700;

// HTML is embedded at compile time from assets/dashboard.html
static DASHBOARD_HTML: &str = include_str!("../../assets/dashboard.html");

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
    eprintln!("Orchestration dashboard: http://localhost:{port}");
    eprintln!("Press Ctrl+C to stop.");

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let response = match url.as_str() {
            "/" | "/index.html" => Response::from_string(DASHBOARD_HTML).with_header(
                tiny_http::Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8")
                    .unwrap(),
            ),
            "/api/run" => {
                let harness_dir = common::harness_dir();
                let body = match orch::read_run(&harness_dir) {
                    Some(run) => serde_json::to_string(&run).unwrap_or_else(|_| "{}".into()),
                    None => "{}".into(),
                };
                Response::from_string(body).with_header(
                    tiny_http::Header::from_bytes(b"Content-Type", b"application/json").unwrap(),
                )
            }
            url if url.starts_with("/api/agents/") && url.ends_with("/status") => {
                // /api/agents/{id}/status
                let agent_id = url
                    .trim_start_matches("/api/agents/")
                    .trim_end_matches("/status");
                let harness_dir = common::harness_dir();
                if !orch::validate_agent_id(agent_id) {
                    Response::from_string("{\"error\":\"invalid agent id\"}")
                        .with_status_code(400)
                        .with_header(
                            tiny_http::Header::from_bytes(b"Content-Type", b"application/json")
                                .unwrap(),
                        )
                } else {
                    let body = match orch::read_agent_status(&harness_dir, agent_id) {
                        Some(s) => serde_json::to_string(&s).unwrap_or_else(|_| "{}".into()),
                        None => "{}".into(),
                    };
                    Response::from_string(body).with_header(
                        tiny_http::Header::from_bytes(b"Content-Type", b"application/json")
                            .unwrap(),
                    )
                }
            }
            "/api/events" => {
                // SSE endpoint — emit current state snapshot then close
                let harness_dir = common::harness_dir();
                let run = orch::read_run(&harness_dir);
                let data = serde_json::to_string(&run).unwrap_or_else(|_| "null".into());
                let sse_body = format!("data: {data}\n\n");
                Response::from_string(sse_body)
                    .with_header(
                        tiny_http::Header::from_bytes(b"Content-Type", b"text/event-stream")
                            .unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(b"Cache-Control", b"no-cache").unwrap(),
                    )
            }
            _ => Response::from_string("Not Found").with_status_code(404),
        };
        let _ = request.respond(response);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_html_not_empty() {
        assert!(!DASHBOARD_HTML.is_empty());
        assert!(DASHBOARD_HTML.contains("d3js.org"));
        assert!(DASHBOARD_HTML.contains("Epic Harness"));
    }

    #[test]
    fn default_port_is_7700() {
        assert_eq!(DEFAULT_PORT, 7700);
    }
}
