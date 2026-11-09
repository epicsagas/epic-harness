#[path = "common/observation.rs"]
mod observation;
use sqlx::Row;

#[tokio::test]
async fn issue_192_silent_explicit_failure_is_scored_without_response() {
    let fixture = observation::Fixture::new();
    fixture.observe(serde_json::json!({"hook_event_name":"PostToolUse","tool_name":"Bash","session_id":"silent-fixture","tool_input":{"command":"node synthetic.js"},"exit_code":2}));
    fixture.observe(serde_json::json!({"hook_event_name":"PostToolUse","tool_name":"Read","session_id":"silent-read-fixture","tool_input":{"file_path":"src/a.rs"},"is_error":true}));
    fixture.observe(serde_json::json!({"tool_name":"Bash","session_id":"uncompleted-fixture","tool_input":{"command":"node pending.js"}}));
    let rows = fixture.rows().await;
    assert_eq!(rows.len(), 3);
    for row in &rows[..2] {
        assert_eq!(
            row.get::<Option<String>, _>("result").as_deref(),
            Some("error")
        );
        assert_eq!(
            row.get::<Option<String>, _>("failure_category").as_deref(),
            Some("runtime_error")
        );
        assert!(row.get::<Option<f64>, _>("score").is_some());
    }
    assert!(rows[2].get::<Option<f64>, _>("score").is_none());
}
