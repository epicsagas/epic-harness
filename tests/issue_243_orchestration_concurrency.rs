use epic_harness::orchestrate::state::{
    AgentDef, AgentStatus, OrchestrationRun, RunStatus, init_run, read_run, upsert_agent_to_run,
};
use std::collections::HashMap;
use std::sync::{Arc, Barrier};

#[test]
fn issue243_concurrent_successful_updates_preserve_every_agent() {
    let temp = tempfile::tempdir().unwrap();
    let now = "2026-01-01T00:00:00Z".to_owned();
    let run = OrchestrationRun {
        id: "concurrency-fixture".into(),
        // Reopening completed runs is covered separately by #242.
        status: RunStatus::Running,
        agents: (0..1000)
            .map(|i| AgentDef {
                id: format!("old-{i}"),
                role: "tester".into(),
                task: "completed synthetic task".into(),
                satisfies: vec![],
                status: AgentStatus::Done,
                started_at: Some(now.clone()),
                completed_at: Some(now.clone()),
            })
            .collect(),
        dependency_graph: HashMap::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    init_run(temp.path(), &run).unwrap();
    let barrier = Arc::new(Barrier::new(8));
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|worker| {
                let barrier = Arc::clone(&barrier);
                let base = temp.path();
                scope.spawn(move || {
                    barrier.wait();
                    for event in 0..8 {
                        upsert_agent_to_run(
                            base,
                            &format!("worker-{worker}-{event}"),
                            "unique synthetic task",
                            "tester",
                            AgentStatus::Running,
                        )?;
                    }
                    Ok::<_, std::io::Error>(())
                })
            })
            .collect();
        for worker in workers {
            worker
                .join()
                .unwrap()
                .expect("every successful event must persist");
        }
    });
    let actual = read_run(temp.path()).unwrap();
    assert_eq!(
        actual.agents.len(),
        1064,
        "a concurrent writer lost an event"
    );
    assert_eq!(actual.status, RunStatus::Running);
}
