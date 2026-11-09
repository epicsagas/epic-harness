#[path = "common/observation.rs"]
mod observation;
use sqlx::Row;

#[tokio::test]
async fn issue_188_host_id_replays_are_idempotent_without_dropping_anonymous_calls() {
    let f = observation::Fixture::new();
    let payload = serde_json::json!({"hook_event_name":"PostToolUse","tool_name":"Bash","session_id":"identity-fixture","tool_use_id":"stable-call","tool_input":{"command":"node synthetic.js"},"tool_response":{"output":"ok","exit_code":0}});
    f.observe(payload.clone());
    f.observe(payload);
    for exit_code in [0, 1, 0, 1] {
        f.observe(serde_json::json!({"hook_event_name":"PostToolUse","tool_name":"Bash","session_id":"identity-fixture","tool_input":{"command":"node synthetic.js"},"tool_response":{"output":if exit_code == 0 { "ok" } else { "TypeError: synthetic fixture" }}}));
    }
    let rows = f.rows().await;
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows.iter()
            .filter(|row| row.get::<Option<String>, _>("result").as_deref() == Some("error"))
            .count(),
        2
    );
}
