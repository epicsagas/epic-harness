//! Wire-level regression tests for the memory MCP subprocess.
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn exchange_lines(messages: &[String]) -> Vec<Value> {
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
fn issue_236_mcp_validates_envelopes_and_silences_notifications() {
    let responses = exchange_lines(&[
        "{".to_string(),
        json!({"jsonrpc":"2.0","id":17}).to_string(),
        json!({"jsonrpc":"1.0","id":18,"method":"tools/list"}).to_string(),
        json!({"jsonrpc":"2.0","method":"tools/list"}).to_string(),
        json!({"jsonrpc":"2.0","method":"ping"}).to_string(),
        json!({"jsonrpc":"2.0","method":"unknown-fixture-method"}).to_string(),
        json!({"jsonrpc":"2.0","id":19,"method":"tools/list"}).to_string(),
        json!({"jsonrpc":"2.0","id":20,"method":"unknown-fixture-method"}).to_string(),
        json!({"jsonrpc":"2.0","id":null,"method":"tools/list"}).to_string(),
        json!({"jsonrpc":"2.0","id":21,"method":"tools/call","params":[]}).to_string(),
    ]);
    assert_eq!(
        responses.len(),
        7,
        "notifications must not receive responses"
    );
    assert!(
        responses
            .iter()
            .any(|r| r["id"].is_null() && r["error"]["code"] == -32700)
    );
    for id in [17, 18] {
        let response = responses.iter().find(|r| r["id"] == id).unwrap();
        assert_eq!(response["error"]["code"], -32600);
    }
    let listed = responses.iter().find(|r| r["id"] == 19).unwrap();
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 6);
    assert_eq!(
        responses.iter().find(|r| r["id"] == 20).unwrap()["error"]["code"],
        -32601
    );
    assert!(responses.iter().any(|r| {
        r["id"].is_null()
            && r["result"]["tools"]
                .as_array()
                .is_some_and(|tools| tools.len() == 6)
    }));
    assert_eq!(
        responses.iter().find(|r| r["id"] == 21).unwrap()["error"]["code"],
        -32602
    );
}
