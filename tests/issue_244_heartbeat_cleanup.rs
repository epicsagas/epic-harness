use epic_harness::orchestrate::state::{
    AgentDef, AgentStatus, AgentStatusFile, OrchestrationRun, RunStatus, auto_cleanup_stale_runs,
    init_run, read_agent_status, read_run, write_agent_status,
};
use std::collections::HashMap;

fn fixture(heartbeat: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let now = epic_harness::shared::helpers::now_iso();
    init_run(
        temp.path(),
        &OrchestrationRun {
            id: "auto-heartbeat-regression".into(),
            status: RunStatus::Running,
            agents: vec![AgentDef {
                id: "worker".into(),
                role: "tester".into(),
                task: "synthetic".into(),
                satisfies: vec![],
                status: AgentStatus::Running,
                started_at: Some("2000-01-01T00:00:00Z".into()),
                completed_at: None,
            }],
            dependency_graph: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        },
    )
    .unwrap();
    write_agent_status(
        temp.path(),
        "worker",
        &AgentStatusFile {
            agent_id: "worker".into(),
            phase: "testing".into(),
            progress: 0.5,
            last_heartbeat: heartbeat.into(),
            status: AgentStatus::Running,
        },
    )
    .unwrap();
    temp
}

#[test]
fn issue244_recent_heartbeat_preserves_long_running_agent() {
    let temp = fixture(&epic_harness::shared::helpers::now_iso());
    auto_cleanup_stale_runs(temp.path());
    let run = read_run(temp.path()).unwrap();
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(run.agents[0].status, AgentStatus::Running);
    assert!(run.agents[0].completed_at.is_none());
    assert_eq!(
        read_agent_status(temp.path(), "worker").unwrap().status,
        AgentStatus::Running
    );
}

#[test]
fn issue244_stale_activity_is_unknown_not_successful_completion() {
    let temp = fixture("2000-01-01T00:00:00Z");
    auto_cleanup_stale_runs(temp.path());
    let run = read_run(temp.path()).unwrap();
    assert_eq!(run.agents[0].status, AgentStatus::Blocked);
    assert!(run.agents[0].completed_at.is_none());
    let status =
        read_agent_status(temp.path(), "worker").expect("stale evidence must remain available");
    assert_eq!(status.status, AgentStatus::Blocked);
    assert_eq!(status.phase, "stale");
}
