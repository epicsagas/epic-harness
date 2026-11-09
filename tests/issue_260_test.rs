#[path = "common/observation.rs"]
mod observation;
use sqlx::Row;

#[tokio::test]
async fn issue_260_successful_empty_search_preserves_usefulness_penalty() {
    let fixture = observation::Fixture::new();
    for (session, output) in [
        ("empty-search", ""),
        ("no-match-search", "No matches found"),
    ] {
        fixture.observe(serde_json::json!({"tool_name":"Grep","session_id":session,"tool_input":{"pattern":"synthetic"},"tool_response":{"output":output,"exit_code":0}}));
    }
    let rows = fixture.rows().await;
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.get::<String, _>("result"), "success");
        assert_eq!(row.get::<f64, _>("dim_success"), 1.0);
        assert_eq!(row.get::<f64, _>("dim_quality"), 0.5);
        assert!((row.get::<f64, _>("score") - 0.85).abs() < 0.0001);
    }
}
