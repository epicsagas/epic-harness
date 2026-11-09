#[path = "common/observation.rs"]
mod observation;
use std::process::Command;

fn context(f: &observation::Fixture) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["reflect", "--context", "--project", "fixture-project"])
        .current_dir(&f.cwd)
        .env("HOME", &f.home)
        .env("USERPROFILE", &f.home)
        .env("HARNESS_DIR", f.data.canonicalize().unwrap())
        .env("HARNESS_ROOT", &f.root)
        .env("XDG_CONFIG_HOME", f.home.join(".config"))
        // The canonical Windows harness root contains `?`; keep the SQLite
        // URL on the equivalent ordinary path so it is not parsed as options.
        .env(
            "HARNESS_DB_URL",
            format!("sqlite:{}", f.data.join("harness.db").display()),
        )
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[tokio::test]
async fn issue_188_context_preserves_distinct_fallbacks_and_deduplicates_known_imports_only() {
    let f = observation::Fixture::new();
    f.observe(serde_json::json!({"tool_name":"Bash","session_id":"context-fixture","tool_input":{"command":"node synthetic.js"},"tool_response":{"output":"ok","exit_code":0}}));
    sqlx::any::install_default_drivers();
    let pool = sqlx::AnyPool::connect(&format!("sqlite:{}", f.data.join("harness.db").display()))
        .await
        .unwrap();
    let existing = epic_harness::store::observations::query_obs_for_date_range_pool(
        &pool,
        "2020-01-01",
        "2099-12-31",
        "fixture-project",
    )
    .await
    .unwrap()
    .remove(0);
    let mut failed = existing.clone();
    failed.result = Some("error".into());
    failed.score = Some(0.32);
    failed.failure_category = Some("type_error".into());
    failed.sequence_id = Some(7);
    let filename = "session_20990101_fixture.jsonl";
    let dir = f.data.join("projects/fixture-project/obs");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(filename),
        format!(
            "{}\n{}\n",
            serde_json::to_string(&existing).unwrap(),
            serde_json::to_string(&failed).unwrap()
        ),
    )
    .unwrap();
    assert_eq!(
        context(&f)["obs_stats"]["total"],
        3,
        "even identical anonymous fallback calls are distinct"
    );
    let identity = format!(
        "jsonl:{filename}:1:{}",
        serde_json::to_string(&failed).unwrap()
    );
    epic_harness::store::observations::insert_observation_with_identity_pool(
        &pool,
        &failed,
        "context-fixture",
        "fixture-project",
        Some(&identity),
    )
    .await
    .unwrap();
    assert_eq!(
        context(&f)["obs_stats"]["total"],
        3,
        "a known backfilled record must not be counted twice while its file remains"
    );
    pool.close().await;
}
