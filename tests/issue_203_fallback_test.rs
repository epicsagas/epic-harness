#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;

#[test]
fn issue_203_partial_backfill_identity_uses_physical_lines() {
    let f = Fixture::new("");
    std::fs::create_dir(f.data.join("harness.db")).unwrap();
    for tool in ["Bash", "BlockedTool", "Bash"] {
        f.call(&["observe"], serde_json::json!({"hook_event_name":"PostToolUse","session_id":"physical-lines","tool_name":tool,"tool_input":{"command":"node synthetic.js"},"tool_response":{"output":"ok","exit_code":0}}));
    }
    let fallback = f.project.join("obs/session_physical-lines.jsonl");
    let valid = std::fs::read_to_string(&fallback).unwrap();
    std::fs::write(&fallback, format!("\nmalformed fixture line\n{valid}")).unwrap();
    std::fs::remove_dir(f.data.join("harness.db")).unwrap();
    f.initialize();
    f.sql("CREATE TRIGGER reject_one_import BEFORE INSERT ON observations WHEN NEW.tool = 'BlockedTool' BEGIN SELECT RAISE(ABORT,'synthetic import failure'); END;");
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    let context = f.call(&["reflect", "--context"], serde_json::json!({}));
    let value: serde_json::Value = serde_json::from_slice(&context.stdout).unwrap();
    assert_eq!(
        value["obs_stats"]["total"], 3,
        "Partial imports must share physical-line identities with context merging"
    );
    f.sql("DROP TRIGGER reject_one_import;");
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(
        fallback.exists(),
        "Malformed source lines must be retained rather than discarded"
    );
    assert_eq!(f.metric()["score_history"][0]["observations"], 3);
}
#[test]
fn issue_203_uuid_fallback_recovers_after_database_returns() {
    let f = Fixture::new("");
    std::fs::create_dir(f.data.join("harness.db")).unwrap();
    for i in 0..3 {
        let output = f.call(&["observe"], serde_json::json!({"hook_event_name":"PostToolUse","session_id":"11111111-2222-4333-8444-555555555555","tool_name":"Bash","tool_input":{"command":format!("node synthetic.js --attempt {i}")},"tool_response":{"output":"TypeError: synthetic failure","exit_code":1}}));
        assert!(output.status.success());
    }
    let fallback = f
        .project
        .join("obs/session_11111111-2222-4333-8444-555555555555.jsonl");
    assert!(fallback.exists());
    std::fs::remove_dir(f.data.join("harness.db")).unwrap();
    let output = f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fallback.exists(), "UUID fallback was not imported");
    assert_eq!(f.metric()["total_sessions"], 1);
}

#[test]
fn issue_203_partial_backfill_retry_is_idempotent() {
    let f = Fixture::new("");
    std::fs::create_dir(f.data.join("harness.db")).unwrap();
    for (i, tool) in ["Bash", "BlockedTool", "Bash"].iter().enumerate() {
        f.call(&["observe"], serde_json::json!({"hook_event_name":"PostToolUse","session_id":"synthetic-uuid-backlog","tool_name":tool,"tool_input":{"command":format!("node synthetic.js --attempt {i}")},"tool_response":{"output":"TypeError: synthetic failure","exit_code":1}}));
    }
    let fallback = f.project.join("obs/session_synthetic-uuid-backlog.jsonl");
    std::fs::remove_dir(f.data.join("harness.db")).unwrap();
    f.initialize();
    f.sql("CREATE TRIGGER reject_one_import BEFORE INSERT ON observations WHEN NEW.tool = 'BlockedTool' BEGIN SELECT RAISE(ABORT,'synthetic import failure'); END;");
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(fallback.exists(), "A partial import must retain its source");
    f.sql("DROP TRIGGER reject_one_import;");
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(!fallback.exists());
    assert_eq!(
        f.metric()["score_history"][0]["observations"],
        3,
        "Retry duplicated the already imported records"
    );
}
