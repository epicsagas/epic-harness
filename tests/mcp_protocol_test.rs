//! mcp_protocol_test.rs: JSON-RPC conformance of `epic-harness mem mcp`.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[test]
fn mcp_server_follows_json_rpc() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["mem", "mcp"])
        .env("HARNESS_ROOT", root.path())
        .env("HOME", root.path())
        .env_remove("HARNESS_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let input = [
        "not json",
        r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","method":"tools/list"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        r#"[{"jsonrpc":"2.0","id":3,"method":"ping"}]"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"nope"}"#,
        r#"{"jsonrpc":"2.0","id":"s","method":42}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    ]
    .join("\n");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{input}").unwrap();
    drop(stdin);

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let err = |id: Value, code: i64, message: &str| json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}});
    let expected = vec![
        err(Value::Null, -32700, "Parse error"),
        err(json!(1), -32600, "Invalid Request"),
        json!({"jsonrpc":"2.0","id":2,"result":{}}),
        err(Value::Null, -32600, "Invalid Request"),
        err(json!(4), -32601, "Method not found"),
        err(json!("s"), -32600, "Invalid Request"),
    ];
    assert_eq!(lines, expected);
}
