#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn issue_205_primary_critic_flag_blocks_synthesis_without_json_mirror() {
    let f = Fixture::new("");
    f.initialize();
    f.sql("INSERT INTO metrics_state(key,value,project) VALUES('reward_hacking_suspected','true','issue-project')");
    let body = f.seed_pending("evo-synthetic");
    let output = f.call(
        &[
            "evolve",
            "accept-synth",
            "--skill",
            "evo-synthetic",
            "--file",
            body.to_str().unwrap(),
        ],
        serde_json::json!({}),
    );
    assert_eq!(
        output.status.code(),
        Some(5),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!f.project.join("evolved/evo-synthetic/SKILL.md").exists());
    f.sql("UPDATE metrics_state SET value='invalid' WHERE key='reward_hacking_suspected'");
    let invalid = f.call(
        &[
            "evolve",
            "accept-synth",
            "--skill",
            "evo-synthetic",
            "--file",
            body.to_str().unwrap(),
        ],
        serde_json::json!({}),
    );
    assert_eq!(invalid.status.code(), Some(6));
    f.sql("UPDATE metrics_state SET value='false' WHERE key='reward_hacking_suspected'");
    let mut stale =
        serde_json::to_value(epic_harness::shared::evolution::Metrics::default()).unwrap();
    stale["reward_hacking_suspected"] = serde_json::json!(true);
    std::fs::write(f.project.join("metrics.json"), stale.to_string()).unwrap();
    let allowed = f.call(
        &[
            "evolve",
            "accept-synth",
            "--skill",
            "evo-synthetic",
            "--file",
            body.to_str().unwrap(),
        ],
        serde_json::json!({}),
    );
    assert!(
        allowed.status.success(),
        "Stale mirror overruled primary: {}",
        String::from_utf8_lossy(&allowed.stderr)
    );
}
