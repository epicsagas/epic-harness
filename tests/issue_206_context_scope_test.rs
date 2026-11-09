#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn issue_206_context_excludes_other_projects_history() {
    let f = Fixture::new("");
    f.initialize();
    f.sql("INSERT INTO sessions(timestamp,summary,created_at_millis,project) VALUES('2026-11-09T00:00:00Z','FOREIGN_CONTEXT',1,'other-project'); INSERT INTO evolution_records(timestamp,project) VALUES('2026-11-09T00:00:00Z','other-project');");
    let output = f.call(
        &["reflect", "--context", "--project", "issue-project"],
        serde_json::json!({}),
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["evolution_stats"]["total_sessions"], 0);
    assert_eq!(value["session_snapshots"].as_array().unwrap().len(), 0);
    f.sql("INSERT INTO sessions(timestamp,summary,created_at_millis,project) VALUES('2026-11-09T00:00:00Z','OWN_CONTEXT',2,'issue-project');");
    for _ in 0..6 {
        f.sql("INSERT INTO sessions(timestamp,summary,created_at_millis,project) VALUES('2026-11-09T00:00:00Z','FOREIGN_CONTEXT',3,'other-project');");
    }
    let output = f.call(
        &["reflect", "--context", "--project", "issue-project"],
        serde_json::json!({}),
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["session_snapshots"].as_array().unwrap().len(), 1);
    assert_eq!(value["session_snapshots"][0]["summary"], "OWN_CONTEXT");
}
