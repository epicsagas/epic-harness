#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn issue_207_context_counts_current_evolution_schema() {
    let f = Fixture::new("");
    f.initialize();
    f.sql("INSERT INTO evolution_records(timestamp,skills_seeded,failure_patterns,project) VALUES('2026-11-09T00:00:00Z',2,'[{\"pattern_type\":\"repeated_same_error\",\"description\":\"fixture\",\"count\":3,\"involved_files\":[],\"suggested_remediation\":\"inspect\"}]','issue-project');");
    let output = f.call(&["reflect", "--context"], serde_json::json!({}));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["evolution_stats"]["skills_generated"], 2);
    assert_eq!(value["evolution_stats"]["patterns_detected"], 1);
    assert_eq!(
        value["evolution_stats"]["pattern_frequency"]["repeated_same_error"],
        3
    );
}
