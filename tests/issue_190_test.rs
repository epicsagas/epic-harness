#[path = "common/observation.rs"]
mod observation;
use sqlx::Row;

#[tokio::test]
async fn issue_190_verbose_output_retains_stderr_and_tail_failure_evidence() {
    let fixture = observation::Fixture::new();
    for (session, response) in [
        (
            "stderr-fixture",
            serde_json::json!({"output":"synthetic log\n".repeat(300),"stderr":"TypeError: synthetic diagnostic"}),
        ),
        (
            "explicit-fixture",
            serde_json::json!({"output":"synthetic log\n".repeat(300),"stderr":"TypeError: synthetic diagnostic","exit_code":1}),
        ),
        (
            "tail-fixture",
            serde_json::json!({"output":format!("{}\nTypeError: synthetic diagnostic","synthetic log\n".repeat(300))}),
        ),
    ] {
        fixture.observe(serde_json::json!({"tool_name":"Bash","session_id":session,"tool_input":{"command":"node synthetic.js"},"tool_response":response}));
    }
    let rows = fixture.rows().await;
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert_eq!(
            row.get::<Option<String>, _>("failure_category").as_deref(),
            Some("type_error")
        );
        assert!(
            row.get::<String, _>("error_snippet")
                .contains("TypeError: synthetic diagnostic")
        );
        assert_eq!(row.get::<String, _>("result"), "error");
    }
}
