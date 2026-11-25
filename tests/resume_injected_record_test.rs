//! `resume` records which evolved skills it really injected (session_start.json)
//! so attribution never credits a skill the character budget dropped. Every run
//! uses a temp HOME, HARNESS_DIR and cwd.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const SLUG: &str = "fixture_project";

#[test]
fn resume_records_only_skills_that_survived_the_budget() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cwd = root.join(SLUG);
    let harness = root.join("harness-root");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(root.join("home/.config/epic-harness")).unwrap();
    fs::write(
        root.join("home/.config/epic-harness/telemetry-consent"),
        "off",
    )
    .unwrap();
    // No holdout rotation, tiny total budget: only the first skills fit.
    fs::create_dir_all(&harness).unwrap();
    fs::write(
        harness.join("config.toml"),
        "[dashboard]\nport = 0\nauto_open = false\n[evolution]\nattribution_holdout_modulus = 0\n\
         [model]\ninject_templates = true\ninject_per_skill_chars = 200\ninject_total_chars = 600\n",
    )
    .unwrap();
    let names: Vec<String> = (0..6).map(|i| format!("evo-skill-{i}")).collect();
    for n in &names {
        let dir = harness.join("projects").join(SLUG).join("evolved").join(n);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {n}\n---\n\n{}", "x".repeat(300)),
        )
        .unwrap();
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("resume")
        .current_dir(&cwd)
        .env("HOME", root.join("home"))
        .env("HARNESS_DIR", &harness)
        .env_remove("HARNESS_ROOT")
        .env("GIT_CEILING_DIRECTORIES", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"session_id":"s1"}"#)
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let rec: Value = serde_json::from_str(
        &fs::read_to_string(
            harness
                .join("projects")
                .join(SLUG)
                .join("session_start.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(rec["session_id"], "s1", "record names the writing session");
    let injected: Vec<&str> = rec["injected"]
        .as_array()
        .expect("injected list recorded")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        !injected.is_empty() && injected.len() < names.len(),
        "{injected:?}"
    );
    for n in &names {
        assert_eq!(
            injected.contains(&n.as_str()),
            stdout.contains(&format!("### {n}\n")),
            "{n} record must match what reached stdout"
        );
    }

    // The stderr hint names exactly the injected skills.
    let stderr = String::from_utf8_lossy(&out.stderr);
    let hint = stderr
        .lines()
        .find_map(|l| l.strip_prefix("[resume] Evolved skills injected: "))
        .expect("injected hint printed");
    let mut hinted: Vec<&str> = hint.split(", ").collect();
    let mut recorded = injected.clone();
    hinted.sort();
    recorded.sort();
    assert_eq!(hinted, recorded);
}
