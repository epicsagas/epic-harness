#[path = "common/observation.rs"]
mod observation;
use sqlx::Row;

#[tokio::test]
async fn issue_191_path_alias_keeps_relative_files_distinct() {
    let fixture = observation::Fixture::new();
    for path in ["src/a.rs", "src/b.rs"] {
        fixture.observe(serde_json::json!({"tool_name":"Read","session_id":"alias-fixture","tool_input":{"path":path},"tool_response":{"output":"synthetic source"}}));
    }
    let rows = fixture.rows().await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("action"), "src/a.rs");
    assert_eq!(rows[1].get::<String, _>("action"), "src/b.rs");
}
