//! `epic evolve accept-synth` through the real binary. Every run uses a temp
//! HOME, HARNESS_DIR and cwd; manifests and bodies are synthetic.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SLUG: &str = "fixture_project";
const SKILL: &str = "evo-synthetic";
const EXIT_IO: i32 = 6;

const BODY: &str = "## Process\n1. Run the synthetic check before editing the fixture module.\n\n\
## Anti-Rationalization\n| Excuse | Rebuttal |\n|---|---|\n| It passed once | Run it again |\n\n\
## Evidence Required\n- [ ] synthetic check passes\n\n\
## Red Flags\n- the same synthetic error repeats\n";

struct Env {
    _temp: tempfile::TempDir,
    cwd: PathBuf,
    home: PathBuf,
    harness: PathBuf,
}

impl Env {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let cwd = root.join(SLUG);
        let home = root.join("home");
        let harness = root.join("harness-root");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(home.join(".config/epic-harness")).unwrap();
        fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        fs::create_dir_all(harness.join("projects").join(SLUG)).unwrap();
        fs::write(cwd.join("body.md"), BODY).unwrap();
        let env = Self {
            _temp: temp,
            cwd,
            home,
            harness,
        };
        let pending = serde_json::json!({
            "schema_version": 1,
            "skill_name": SKILL,
            "origin": "weak_tool",
            "confidence": 0.6,
            "evidence": {},
            "template_content": "template",
            "prompt_guidance": "guidance",
            "created": "2026-01-01T00:00:00Z",
        });
        fs::write(
            env.project().join("pending_synth.jsonl"),
            format!("{pending}\n"),
        )
        .unwrap();
        env
    }

    fn project(&self) -> PathBuf {
        self.harness.join("projects").join(SLUG)
    }

    fn accept(&self) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args([
                "evolve",
                "accept-synth",
                "--skill",
                SKILL,
                "--file",
                "body.md",
            ])
            .current_dir(&self.cwd)
            .env("HOME", &self.home)
            .env("HARNESS_DIR", &self.harness)
            .env("GIT_CEILING_DIRECTORIES", self.cwd.parent().unwrap())
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn pending_status(&self) -> String {
        let text = fs::read_to_string(self.project().join("pending_synth.jsonl")).unwrap();
        let rec: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        rec["status"].as_str().unwrap_or("pending").to_string()
    }

    fn ledger_lines(&self) -> usize {
        fs::read_to_string(self.project().join("manifests.jsonl"))
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }
}

#[test]
fn accepted_body_is_installed_and_consumed() {
    let env = Env::new();
    let (code, err) = env.accept();
    assert_eq!(code, 0, "{err}");
    assert!(
        env.project()
            .join("evolved")
            .join(SKILL)
            .join("SKILL.md")
            .is_file()
    );
    assert_eq!(env.pending_status(), "synthesized");
    assert_eq!(env.ledger_lines(), 1);
}

#[test]
fn failed_skill_write_keeps_work_pending_and_deletes_nothing() {
    let env = Env::new();
    // A directory where SKILL.md belongs makes the write fail.
    let skill_md = env.project().join("evolved").join(SKILL).join("SKILL.md");
    fs::create_dir_all(&skill_md).unwrap();
    fs::write(skill_md.join("sentinel.txt"), "keep").unwrap();

    let (code, err) = env.accept();
    assert_eq!(code, EXIT_IO, "{err}");
    assert!(!err.contains("accepted"), "{err}");
    assert_eq!(env.pending_status(), "pending");
    assert_eq!(env.ledger_lines(), 0);
    assert!(
        skill_md.join("sentinel.txt").is_file(),
        "skill dir was deleted"
    );
}

#[test]
fn failed_consume_is_not_reported_as_success() {
    let env = Env::new();
    // The pending file is rewritten through a temp file; a directory there blocks it.
    fs::create_dir(env.project().join("pending_synth.jsonl.tmp")).unwrap();

    let (code, err) = env.accept();
    assert_eq!(code, EXIT_IO, "{err}");
    assert!(!err.contains("accepted"), "{err}");
    assert_eq!(env.pending_status(), "pending");
    assert_eq!(env.ledger_lines(), 0);
}
