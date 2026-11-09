#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn issue_271_cold_start_guidance_reaches_first_session() {
    let f = Fixture::new(
        "[model]\nclass=\"light\"\ninject_templates=true\n[evolution]\nattribution_holdout_modulus=0\n",
    );
    std::fs::write(f.work.join("package.json"), "{}").unwrap();
    let output = f.call(&["resume"], serde_json::json!({"hook_event_name":"SessionStart","session_id":"cold-start","source":"startup"}));
    assert!(output.status.success());
    assert!(f.project.join("evolved/evo-ts-care/SKILL.md").exists());
    assert!(
        fixture::context(&output).contains("## Evolved Skills"),
        "Created presets were not injected into first session"
    );
}
