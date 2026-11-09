#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;

#[test]
fn issue_218_legacy_watermark_migration_keeps_delayed_fallback() {
    let f = Fixture::new("");
    f.initialize();
    for i in 0..3 {
        f.sql(&format!("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,project) VALUES('2024-01-01T00:00:00Z','already-analyzed','Bash','bash','old-{i}','success',1.0,'issue-project')"));
    }
    fixture::write(
        f.project.join("reflect_watermark.txt"),
        "2025-01-01T00:00:00Z",
    );
    let delayed: String = (0..3).map(|i| {
        serde_json::json!({"timestamp":"2024-01-01T00:00:00Z","tool":"Bash","tool_category":"bash","action":format!("delayed-{i}"),"result":"success","score":1.0}).to_string() + "\n"
    }).collect();
    fixture::write(f.project.join(format!("obs/session_{}_delayed.jsonl", epic_harness::shared::today())), &delayed);
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert_eq!(f.metric()["total_sessions"], 1);
    assert_eq!(
        f.metric()["score_history"][0]["observations"],
        3,
        "Legacy rows stay consumed but delayed fallback stays eligible"
    );
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert_eq!(f.metric()["total_sessions"], 1);
}
#[test]
fn issue_218_new_same_second_rows_are_consumed_once() {
    let f = Fixture::new("");
    f.initialize();
    for i in 0..3 {
        f.sql(&format!("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,project) VALUES(strftime('%Y-%m-%dT%H:%M:%SZ','now'),'initial','Bash','bash','initial-{i}','success',1.0,'issue-project')"));
    }
    let first = f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(first.status.success());
    assert_eq!(f.metric()["total_sessions"], 1);
    for i in 0..3 {
        f.sql(&format!("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,project) SELECT timestamp,'late','Bash','bash','late-{i}','success',1.0,'issue-project' FROM observations ORDER BY id LIMIT 1"));
    }
    let second = f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(second.status.success());
    assert_eq!(
        f.metric()["total_sessions"],
        2,
        "Late rows were lost behind a second-precision watermark"
    );
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert_eq!(
        f.metric()["total_sessions"],
        2,
        "Same IDs must not be analyzed twice"
    );
}

#[test]
fn issue_218_failed_state_save_leaves_observations_pending() {
    let f = Fixture::new("");
    f.initialize();
    for i in 0..3 {
        f.sql(&format!("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,project) VALUES('2026-01-01T00:00:00Z','retry','Bash','bash','retry-{i}','success',1.0,'issue-project')"));
    }
    f.sql("CREATE TRIGGER reject_metrics BEFORE INSERT ON metrics_state WHEN NEW.key='total_sessions' BEGIN SELECT RAISE(ABORT,'synthetic save failure'); END");
    let failed = f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("observations remain pending"));
    f.sql("CREATE TEMP TABLE rollback_assertion(value INTEGER CHECK(value=0)); INSERT INTO rollback_assertion SELECT COUNT(*) FROM evolution_records; INSERT INTO rollback_assertion SELECT COUNT(*) FROM metrics_state WHERE key='reflection_cursor' AND value!='0'; DROP TABLE rollback_assertion; DROP TRIGGER reject_metrics;");
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert_eq!(f.metric()["total_sessions"], 1);
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    assert_eq!(f.metric()["total_sessions"], 1);
}
