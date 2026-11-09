#[path = "support/evolution.rs"]
mod fixture;
use fixture::Fixture;

#[test]
fn issue_204_acceptance_persistence_errors_are_retryable() {
    for obstruction in ["pending_synth.jsonl.tmp", "manifests.jsonl"] {
        let f = Fixture::new("");
        f.initialize();
        let body = f.seed_pending("evo-synthetic");
        let blocked = f.project.join(obstruction);
        std::fs::create_dir(&blocked).unwrap();
        let args = [
            "evolve",
            "accept-synth",
            "--skill",
            "evo-synthetic",
            "--file",
            body.to_str().unwrap(),
        ];
        let failed = f.call(&args, serde_json::json!({}));
        assert_eq!(
            failed.status.code(),
            Some(6),
            "{obstruction}: {}",
            String::from_utf8_lossy(&failed.stderr)
        );
        let pending: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(f.project.join("pending_synth.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(pending["status"], "pending");
        assert!(!String::from_utf8_lossy(&failed.stderr).contains("body accepted"));
        std::fs::remove_dir(&blocked).unwrap();
        let retry = f.call(&args, serde_json::json!({}));
        assert!(
            retry.status.success(),
            "{}",
            String::from_utf8_lossy(&retry.stderr)
        );
        assert_eq!(f.call(&args, serde_json::json!({})).status.code(), Some(2));
        assert_eq!(
            std::fs::read_to_string(f.project.join("manifests.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}
#[test]
fn issue_204_failed_synthesis_preserves_pending_and_existing_content() {
    let f = Fixture::new("");
    f.initialize();
    let body = f.seed_pending("evo-synthetic");
    let sentinel = f
        .project
        .join("evolved/evo-synthetic/SKILL.md/sentinel.txt");
    fixture::write(&sentinel, "retain existing content");
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
    assert!(
        !output.status.success(),
        "Failed write was reported as accepted"
    );
    assert_eq!(
        std::fs::read_to_string(sentinel).unwrap(),
        "retain existing content"
    );
    let pending: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(f.project.join("pending_synth.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(pending["status"], "pending");
    assert!(pending["consumed"].is_null());
}

#[test]
fn issue_204_successful_synthesis_preserves_skill_attachments() {
    let f = Fixture::new("");
    f.initialize();
    let body = f.seed_pending("evo-synthetic");
    let dir = f.project.join("evolved/evo-synthetic");
    fixture::write(dir.join("SKILL.md"), "old skill");
    fixture::write(
        dir.join("notes/retained.md"),
        "synthetic retained attachment",
    );
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(dir.join("SKILL.md"))
            .unwrap()
            .contains("## Process")
    );
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["synthesized"], true);
    assert_eq!(
        std::fs::read_to_string(dir.join("notes/retained.md")).unwrap(),
        "synthetic retained attachment"
    );
}
