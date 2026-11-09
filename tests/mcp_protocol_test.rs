//! Wire-level regression tests for the memory MCP subprocess.
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn exchange(messages: &[Value]) -> Vec<Value> {
    let fixture = tempfile::tempdir().expect("temporary MCP fixture");
    let home = fixture.path().join("home");
    let data = fixture.path().join("data");
    let cwd = fixture.path().join("project");
    fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
    fs::write(
        data.join("config.toml"),
        "[dashboard]\nport=0\nauto_open=false\n",
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
    command
        .args(["mem", "mcp"])
        .current_dir(cwd)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("HARNESS_DIR", &data)
        .env("HARNESS_ROOT", fixture.path())
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env_remove("HARNESS_MEM_URL")
        .env_remove("HARNESS_DB_URL")
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("memory MCP child");
    let mut stdin = child.stdin.take().unwrap();
    for message in messages {
        writeln!(stdin, "{message}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "MCP child: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON-RPC response"))
        .collect()
}

#[test]
fn issue_235_mcp_ping_returns_empty_result() {
    let responses = exchange(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":"alive","method":"ping"}),
    ]);
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[1],
        json!({"jsonrpc":"2.0","id":"alive","result":{}})
    );
}
