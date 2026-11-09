use epic_harness::orchestrate::state::{
    AgentStatus, agent_dir, dismiss_agent, read_agent_status, read_run, upsert_agent_to_run,
};

#[test]
fn issue238_dismissal_hides_only_selected_agent_and_preserves_history() {
    let temp = tempfile::tempdir().unwrap();
    for id in ["selected", "other"] {
        upsert_agent_to_run(
            temp.path(),
            id,
            "synthetic task",
            "tester",
            AgentStatus::Running,
        )
        .unwrap();
    }
    let history = agent_dir(temp.path(), "selected").join("stream.jsonl");
    std::fs::write(&history, "synthetic history\n").unwrap();
    assert!(dismiss_agent(temp.path(), "selected").unwrap());
    let run = read_run(temp.path()).unwrap();
    assert_eq!(run.agents.len(), 1);
    assert_eq!(run.agents[0].id, "other");
    assert_eq!(run.agents[0].status, AgentStatus::Running);
    assert!(read_agent_status(temp.path(), "selected").is_none());
    assert!(read_agent_status(temp.path(), "other").is_some());
    assert_eq!(
        std::fs::read_to_string(&history).unwrap(),
        "synthetic history\n"
    );
    assert!(
        std::fs::read_dir(history.parent().unwrap())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("dismissed-"))
    );
    assert!(!dismiss_agent(temp.path(), "selected").unwrap());
    assert_eq!(
        dismiss_agent(temp.path(), "../other").unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
}
