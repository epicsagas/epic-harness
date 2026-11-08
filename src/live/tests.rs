use super::*;

const NOW: u64 = 1794139200; // 2026-11-08T12:00:00Z

async fn pool() -> AnyPool {
    let pool = crate::store::pool::test_memory_pool().await;
    crate::store::schema::init_schema_pool(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn sessions_use_activity_timestamps_and_expire_without_orbit() {
    let pool = pool().await;
    for (project, id, timestamp) in [
        ("one", "uuid-session-one", "2026-11-08T11:59:00Z"),
        ("two", "uuid-session-two", "2026-11-08T11:58:00Z"),
        ("one", "old-session", "2026-11-08T11:00:00Z"),
        ("one", "future-session", "2026-11-09T11:00:00Z"),
    ] {
        sqlx::query("INSERT INTO observations (timestamp, session_id, tool, tool_category, project) VALUES (?, ?, 'Bash', 'bash', ?)")
            .bind(timestamp).bind(id).bind(project).execute(&pool).await.unwrap();
    }
    let temp = tempfile::tempdir().unwrap();
    let all = snapshot(&pool, temp.path(), None, NOW).await.unwrap();
    assert_eq!(all["sessions"].as_array().unwrap().len(), 2);
    assert_eq!(all["sessions"][0]["date"], "2026-11-08");
    assert!(all["runs"].as_array().unwrap().is_empty());
    let scoped = snapshot(&pool, temp.path(), Some("one"), NOW)
        .await
        .unwrap();
    assert_eq!(scoped["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(scoped["sessions"][0]["session_id"], "uuid-session-one");
    assert!(
        snapshot(&pool, temp.path(), Some("../outside"), NOW)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn lifecycle_file_runs_are_discovered_across_projects_with_same_agent_ids() {
    let pool = pool().await;
    let temp = tempfile::tempdir().unwrap();
    for project in ["one", "two"] {
        let base = temp.path().join(project);
        std::fs::create_dir(&base).unwrap();
        crate::orchestrate::state::upsert_agent_to_run(
            &base,
            "same-id",
            "test task",
            "tester",
            crate::orchestrate::state::AgentStatus::Running,
        )
        .unwrap();
    }
    let all = snapshot(&pool, temp.path(), None, NOW).await.unwrap();
    assert_eq!(all["runs"].as_array().unwrap().len(), 2);
    for run in all["runs"].as_array().unwrap() {
        assert_eq!(run["agents"][0]["id"], "same-id");
        assert_eq!(run["agent_statuses"]["same-id"]["status"], "running");
    }
    let scoped = snapshot(&pool, temp.path(), Some("two"), NOW)
        .await
        .unwrap();
    assert_eq!(scoped["runs"].as_array().unwrap().len(), 1);
    assert_eq!(scoped["runs"][0]["project"], "two");
}

#[tokio::test]
async fn sqlite_run_and_agent_rows_have_the_same_dashboard_shape() {
    let pool = pool().await;
    sqlx::query("INSERT INTO orch_runs (id, project, agents_json, dep_graph_json, created_at, updated_at) VALUES ('db-run', 'one', '[]', '{}', '2026-11-08T12:00:00Z', '2026-11-08T12:00:00Z')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO orch_agents (id, run_id, role, task, status, progress, last_heartbeat) VALUES ('db-agent', 'db-run', 'tester', 'synthetic task', 'running', 0.5, '2026-11-08T12:00:00Z')")
        .execute(&pool).await.unwrap();
    let temp = tempfile::tempdir().unwrap();
    let all = snapshot(&pool, temp.path(), None, NOW).await.unwrap();
    assert_eq!(all["runs"][0]["agents"][0]["id"], "db-agent");
    assert_eq!(
        all["runs"][0]["agent_statuses"]["db-agent"]["progress"],
        0.5
    );
    assert!(all["runs"][0]["agents_json"].is_null());
}

#[test]
fn malformed_timestamps_are_not_live() {
    for timestamp in [
        "",
        "not-a-timestamp",
        "2026-00-00T12:00:00Z",
        "invalid Unicode 🦀 timestamp",
    ] {
        assert!(!recent(timestamp, NOW, SESSION_WINDOW));
    }
}

#[tokio::test]
async fn project_directory_links_cannot_expose_outside_runs() {
    let pool = pool().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("projects");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    crate::orchestrate::state::upsert_agent_to_run(
        &outside,
        "outside-agent",
        "outside task",
        "tester",
        crate::orchestrate::state::AgentStatus::Running,
    )
    .unwrap();
    let link = root.join("escape");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    {
        let result = std::process::Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(result.status.success());
    }
    let result = snapshot(&pool, &root, None, NOW).await.unwrap();
    assert!(result["runs"].as_array().unwrap().is_empty());
}
