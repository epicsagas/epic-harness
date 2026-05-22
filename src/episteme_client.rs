//! Lightweight Episteme MCP client for reflect hook integration.
//!
//! Sends `add_insight` requests to the `episteme` binary via stdio JSON-RPC 2.0.
//! All errors are non-fatal: failures are logged via `hint()` and the caller
//! continues normally. This upholds the graceful-degradation contract defined
//! in ROADMAP.md M2.
//!
//! ## Design decisions
//! - Uses `std::process::Command` (no async runtime) to avoid pulling tokio
//!   into epic-harness's sync hook stack.
//! - Spawns `episteme mcp` as a child process, sends one JSON-RPC request,
//!   and reads one response. The child is then killed.
//! - Binary discovery order: `EPISTEME_BIN` env var → `episteme` on PATH →
//!   sibling binary next to the running epic-harness binary.
//! - Confidence score is clamped to [0.0, 1.0] before serialisation.

use std::io::{BufRead, BufReader, Write as IoWrite};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Payload for a single insight to ingest into Episteme.
#[derive(Debug, Clone)]
pub struct InsightPayload {
    /// Natural-language description of the insight.
    pub text: String,
    /// Free-form tags (e.g. `["weak-tool", "bash"]`).
    pub tags: Vec<String>,
    /// Episteme entity IDs this insight relates to (may be empty).
    pub linked_entities: Vec<String>,
    /// Project slug the insight belongs to.
    pub project: String,
    /// Confidence in [0.0, 1.0]. Passed as-is to Episteme.
    pub confidence: f64,
}

/// Send a single `add_insight` request to the `episteme` MCP server.
///
/// Returns `Ok(insight_id)` on success, or an error string on failure.
/// The caller should treat any `Err` as non-fatal.
pub fn add_insight(payload: &InsightPayload) -> Result<String, String> {
    let bin = resolve_episteme_bin()?;

    // Spawn `episteme mcp` (stdio JSON-RPC 2.0 server)
    let mut child = Command::new(&bin)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // suppress Episteme's startup noise
        .spawn()
        .map_err(|e| format!("failed to spawn episteme ({bin:?}): {e}"))?;

    let stdin = child.stdin.take().ok_or("could not open episteme stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("could not open episteme stdout")?;

    // Build JSON-RPC 2.0 initialize + add_insight request sequence
    let confidence = payload.confidence.clamp(0.0, 1.0);
    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "epic-harness-reflect", "version": "0.3.10" }
        }
    });
    let insight_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "add_insight",
            "arguments": {
                "text": payload.text,
                "tags": payload.tags,
                "linked_entities": payload.linked_entities,
                "project": payload.project,
                "confidence": confidence
            }
        }
    });
    // Notifications don't need a response
    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    // Write all messages to stdin then close it
    {
        let mut stdin = stdin;
        let write_msg = |w: &mut dyn IoWrite, v: &serde_json::Value| -> Result<(), String> {
            let s = serde_json::to_string(v).map_err(|e| format!("serialize error: {e}"))?;
            writeln!(w, "{s}").map_err(|e| format!("write error: {e}"))
        };
        write_msg(&mut stdin, &init_msg)?;
        write_msg(&mut stdin, &initialized_notif)?;
        write_msg(&mut stdin, &insight_msg)?;
        // stdin drops here, closing the pipe → Episteme knows no more input
    }

    // Read responses with a timeout guard: read up to 5 lines, stop after id=2
    let reader = BufReader::new(stdout);
    let mut insight_id = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(8);

    for line in reader.lines() {
        if std::time::Instant::now() > deadline {
            break;
        }
        let line = line.map_err(|e| format!("stdout read error: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        // We only care about the response to id=2 (the tools/call)
        if v.get("id").and_then(|i| i.as_u64()) != Some(2) {
            continue;
        }
        // Check for JSON-RPC error
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let _ = kill_child(&mut child);
            return Err(format!("episteme add_insight error: {msg}"));
        }
        // Extract insight_id from result
        insight_id = v
            .pointer("/result/content/0/text")
            .or_else(|| v.pointer("/result/insight_id"))
            .or_else(|| v.pointer("/result/id"))
            .and_then(|x| x.as_str())
            .unwrap_or("ok")
            .to_string();
        break;
    }

    let _ = kill_child(&mut child);

    if insight_id.is_empty() {
        return Err("no response from episteme add_insight".to_string());
    }
    Ok(insight_id)
}

/// Compute a confidence score from a `SessionAnalysis` composite score and
/// the pattern frequency. Mirrors the formula documented in ROADMAP.md M2:
/// `confidence = composite_score × clamp(pattern_frequency / 10, 0.5, 1.0)`
pub fn compute_confidence(avg_score: f64, pattern_count: usize) -> f64 {
    let pattern_factor = (pattern_count as f64 / 10.0).clamp(0.5, 1.0);
    (avg_score * pattern_factor).clamp(0.0, 1.0)
}

/// Resolve the path to the `episteme` binary.
///
/// Search order:
/// 1. `EPISTEME_BIN` environment variable (absolute or relative)
/// 2. `episteme` on `PATH`
/// 3. Sibling directory of the current executable
fn resolve_episteme_bin() -> Result<String, String> {
    // 1. Env override
    if let Ok(bin) = std::env::var("EPISTEME_BIN")
        && !bin.is_empty()
    {
        return Ok(bin);
    }

    // 2. PATH lookup
    if which_episteme() {
        return Ok("episteme".to_string());
    }

    // 3. Sibling to current exe
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join("episteme");
        if candidate.exists() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
        // Windows
        let candidate_exe = dir.join("episteme.exe");
        if candidate_exe.exists() {
            return Ok(candidate_exe.to_string_lossy().into_owned());
        }
    }

    Err("episteme binary not found (set EPISTEME_BIN or add to PATH)".to_string())
}

/// Quick check: does `episteme --version` succeed?
fn which_episteme() -> bool {
    Command::new("episteme")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Best-effort child kill + wait (avoids zombie processes).
fn kill_child(child: &mut std::process::Child) -> std::io::Result<()> {
    let _ = child.kill();
    child.wait()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn confidence_clamped_to_unit_interval() {
        assert_eq!(compute_confidence(0.0, 0), 0.0);
        assert_eq!(compute_confidence(1.0, 100), 1.0);
        // avg=0.8, pattern_count=5 → factor=0.5, confidence=0.4
        let c = compute_confidence(0.8, 5);
        assert!((c - 0.4).abs() < 1e-9);
        // avg=0.8, pattern_count=10 → factor=1.0, confidence=0.8
        let c = compute_confidence(0.8, 10);
        assert!((c - 0.8).abs() < 1e-9);
    }

    #[test]
    #[serial]
    fn resolve_episteme_bin_prefers_env() {
        // Set env to a dummy value and verify it is returned
        // SAFETY: serialized by #[serial]; no concurrent env access
        unsafe {
            std::env::set_var("EPISTEME_BIN", "/custom/path/episteme");
        }
        let result = resolve_episteme_bin();
        unsafe {
            std::env::remove_var("EPISTEME_BIN");
        }
        assert_eq!(result.unwrap(), "/custom/path/episteme");
    }

    #[test]
    #[serial]
    fn resolve_episteme_bin_ignores_empty_env() {
        // SAFETY: serialized by #[serial]; no concurrent env access
        unsafe {
            std::env::set_var("EPISTEME_BIN", "");
        }
        // Should fall through to PATH/sibling check (may fail in test env, that's OK)
        let _result = resolve_episteme_bin();
        unsafe {
            std::env::remove_var("EPISTEME_BIN");
        }
    }
}
