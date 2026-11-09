#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;

fn seed(f: &Fixture) {
    fixture::write(
        f.project.join("evolved/evo-fixture/SKILL.md"),
        "---\nname: evo-fixture\n---\n## Process\nEXPOSURE_SENTINEL: inspect the diagnostic.\n## Red Flags\nDo not guess.\n",
    );
}

fn observations(f: &Fixture, session: &str, score: u8) {
    for i in 0..3 {
        f.sql(&format!("INSERT INTO observations(timestamp,session_id,tool,tool_category,action,result,score,project) VALUES('2026-11-09T00:00:00Z','{session}','Bash','bash','fixture-{i}','success',{score},'issue-project')"));
    }
}

#[test]
fn issue_208_active_and_holdout_have_real_exposure_controls() {
    for (modulus, active, holdout) in [(0, 1, 0), (1, 0, 1)] {
        let f = Fixture::new(&format!(
            "[model]\nclass=\"light\"\n[evolution]\nattribution_holdout_modulus={modulus}\n"
        ));
        seed(&f);
        let output = f.call(&["resume"], serde_json::json!({"hook_event_name":"SessionStart","session_id":"control","source":"startup"}));
        assert_eq!(
            fixture::context(&output).contains("EXPOSURE_SENTINEL"),
            active == 1
        );
        observations(&f, "control", 1);
        f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
        let attribution = &f.metric()["skill_attribution"]["evo-fixture"];
        assert_eq!(attribution["sessions_active"], active);
        assert_eq!(attribution["sessions_holdout"], holdout);
    }
}

#[test]
fn issue_208_missing_start_and_zero_context_budget_are_not_exposure() {
    for (start, budget) in [(false, 10000), (true, 0)] {
        let f = Fixture::new(&format!(
            "[model]\nclass=\"light\"\ninject_total_chars={budget}\n[evolution]\nattribution_holdout_modulus=0\n"
        ));
        seed(&f);
        f.initialize();
        if start {
            let output = f.call(&["resume"], serde_json::json!({"hook_event_name":"SessionStart","session_id":"unexposed","source":"startup"}));
            assert!(!fixture::context(&output).contains("EXPOSURE_SENTINEL"));
        }
        observations(&f, "unexposed", 1);
        f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
        assert_eq!(
            f.metric()["skill_attribution"]["evo-fixture"]["sessions_active"]
                .as_u64()
                .unwrap_or(0),
            0
        );
    }
}

#[test]
fn issue_208_mixed_sessions_are_attributed_individually() {
    let f = Fixture::new("[model]\nclass=\"light\"\n[evolution]\nattribution_holdout_modulus=0\n");
    seed(&f);
    for (session, score) in [("first", 1), ("second", 0)] {
        f.call(&["resume"], serde_json::json!({"hook_event_name":"SessionStart","session_id":session,"source":"startup"}));
        observations(&f, session, score);
    }
    f.call(&["reflect"], serde_json::json!({"hook_event_name":"Stop"}));
    let metrics = f.metric();
    let attribution = &metrics["skill_attribution"]["evo-fixture"];
    assert_eq!(attribution["sessions_active"], 2);
    assert_eq!(attribution["avg_score_with"], 0.5);
}
#[test]
fn issue_208_withheld_template_is_not_credited_as_active() {
    let f = Fixture::new(
        "[model]\nclass=\"frontier\"\ninject_templates=false\n[evolution]\nattribution_holdout_modulus=0\n",
    );
    fixture::write(
        f.project.join("evolved/evo-fixture/SKILL.md"),
        "---\nname: evo-fixture\n---\n## Process\nWITHHELD_SENTINEL: inspect a diagnostic before retrying.\n## Red Flags\nDo not guess.\n",
    );
    let resume = f.call(&["resume"], serde_json::json!({"hook_event_name":"SessionStart","session_id":"exposure-session","source":"startup"}));
    assert!(!fixture::context(&resume).contains("WITHHELD_SENTINEL"));
    for i in 0..3 {
        f.call(&["observe"], serde_json::json!({"hook_event_name":"PostToolUse","session_id":"exposure-session","tool_name":"Bash","tool_input":{"command":format!("node synthetic.js --attempt {i}")},"tool_response":{"output":"ok","exit_code":0}}));
    }
    f.call(
        &["reflect"],
        serde_json::json!({"hook_event_name":"Stop","session_id":"exposure-session"}),
    );
    assert_eq!(
        f.metric()["skill_attribution"]["evo-fixture"]["sessions_active"]
            .as_u64()
            .unwrap_or(0),
        0
    );
}
