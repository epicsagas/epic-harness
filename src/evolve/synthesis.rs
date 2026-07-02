//! LLM-backed skill synthesis (Ring 3, the step templates could never do).
//!
//! The template builders in `skills.rs` emit the same generic advice for any
//! failure. This module replaces a planned skill's body with one synthesized
//! by a headless `claude -p` call from the session's REAL failure evidence
//! (error snippets, category counts, detected patterns). The synthesized body
//! flows through the exact same gates as template content: `HarnessEdit::
//! validate()`, the Critic falsifiability gate, and `gate_skills()`.
//!
//! Failure of any kind — CLI missing, timeout, empty or malformed output —
//! falls back to the template content the planner already produced. Synthesis
//! can only improve a skill, never block seeding.
//!
//! Recursion guard: the child process runs with `EPIC_SYNTH_CHILD=1` (blocks
//! nested synthesis) and `EPIC_HOOK_PROFILE=minimal` (skips reflect/polish/
//! snapshot hooks inside the child session).

use std::io::Read;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::CONFIG;
use crate::evolve::edits::HarnessEdit;
use crate::shared::evolution::SessionAnalysis;
use crate::shared::helpers::hint;
use crate::shared::sanitize::sanitize_skill_content;

/// Set in the synthesis child's environment; its presence disables synthesis.
pub const SYNTH_CHILD_ENV: &str = "EPIC_SYNTH_CHILD";
/// Force-enable synthesis in debug builds (manual testing).
pub const SYNTH_FORCE_ENV: &str = "EPIC_SYNTH_FORCE";

/// Hard bounds on an accepted synthesized body.
const BODY_MIN_CHARS: usize = 80;
const BODY_MAX_CHARS: usize = 6_000;

/// Whether synthesis may run in this process.
///
/// - Never inside a synthesis child (recursion guard).
/// - Never in debug builds (test determinism) unless EPIC_SYNTH_FORCE=1.
/// - Otherwise governed by `[evolution] llm_synthesis` (default: on).
pub fn synthesis_enabled() -> bool {
    if std::env::var(SYNTH_CHILD_ENV).is_ok() {
        return false;
    }
    if std::env::var(SYNTH_FORCE_ENV).is_ok() {
        return true;
    }
    if cfg!(debug_assertions) {
        return false;
    }
    CONFIG.evolution.llm_synthesis
}

/// Upgrade up to `llm_synthesis_max_per_session` AddSkill edits in-place with
/// LLM-synthesized bodies. Returns how many were upgraded. Edits keep their
/// template content when synthesis is unavailable or fails.
pub fn upgrade_edits(edits: &mut [HarnessEdit], analysis: &SessionAnalysis) -> usize {
    if !synthesis_enabled() {
        return 0;
    }
    let budget = CONFIG.evolution.llm_synthesis_max_per_session;
    let mut upgraded = 0usize;
    for edit in edits.iter_mut() {
        if upgraded >= budget {
            break;
        }
        if let HarnessEdit::AddSkill {
            name,
            content,
            origin,
            ..
        } = edit
        {
            match synthesize_skill(name, origin, analysis) {
                Some(new_content) => {
                    *content = new_content;
                    upgraded += 1;
                }
                None => hint(
                    "reflect",
                    &format!("LLM synthesis failed for '{name}' — keeping template body"),
                ),
            }
        }
    }
    upgraded
}

/// Synthesize a complete SKILL.md (frontmatter + body) for one skill.
/// Returns None on any failure; the caller keeps the template content.
fn synthesize_skill(name: &str, origin: &str, analysis: &SessionAnalysis) -> Option<String> {
    let prompt = build_prompt(name, analysis);
    let raw = run_synthesis_command(
        &prompt,
        Duration::from_secs(CONFIG.evolution.llm_synthesis_timeout_secs),
    )?;
    let body = validate_body(&raw)?;
    Some(assemble_skill(name, origin, &body))
}

/// The synthesis prompt: real evidence in, a bounded skill body out.
fn build_prompt(name: &str, analysis: &SessionAnalysis) -> String {
    let mut evidence = String::new();

    if !analysis.error_snippets.is_empty() {
        evidence.push_str("Error snippets from the session (category, count, latest message):\n");
        for s in analysis.error_snippets.iter().take(8) {
            evidence.push_str("- ");
            evidence.push_str(s);
            evidence.push('\n');
        }
    }

    let mut cats: Vec<_> = analysis.per_error_stats.iter().collect();
    cats.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    if !cats.is_empty() {
        evidence.push_str("\nFailure category counts:\n");
        for (cat, count) in cats.iter().take(8) {
            evidence.push_str(&format!("- {cat}: {count}\n"));
        }
    }

    for p in analysis.failure_patterns.iter().take(4) {
        evidence.push_str(&format!(
            "\nDetected pattern: {} ({}x) — {}. Files: {}\n",
            p.pattern_type,
            p.count,
            p.description,
            if p.involved_files.is_empty() {
                "various".to_string()
            } else {
                p.involved_files.join(", ")
            },
        ));
    }

    format!(
        "You are writing the body of a Claude Code skill named '{name}'. It will be \
injected into future coding sessions in THIS project to prevent the failures below \
from recurring.\n\n{evidence}\n\
Write concrete, project-specific guidance grounded in the errors above — name the \
actual error messages, files, and commands involved. Generic advice (\"read the \
error carefully\") is worthless and will be rejected.\n\n\
Output ONLY markdown body text (no YAML frontmatter, no code fences around the \
whole output, no preamble) with exactly these sections:\n\
## Process\n(numbered steps tied to the specific failures above)\n\
## Anti-Rationalization\n(markdown table: Excuse | Rebuttal)\n\
## Evidence Required\n(checklist proving the failure class is fixed)\n\
## Red Flags\n(bullet list of early warnings specific to these errors)\n\n\
Hard limit: 60 lines."
    )
}

/// Run the synthesis command with the prompt on stdin and a wall-clock
/// deadline. Returns captured stdout on clean exit, None otherwise.
fn run_synthesis_command(prompt: &str, timeout: Duration) -> Option<String> {
    let cmd = &CONFIG.evolution.llm_synthesis_cmd;
    let mut command = Command::new(cmd);
    command.arg("-p");
    let model = &CONFIG.evolution.llm_synthesis_model;
    if !model.is_empty() {
        command.arg("--model").arg(model);
    }
    command
        .env(SYNTH_CHILD_ENV, "1")
        .env("EPIC_HOOK_PROFILE", "minimal")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = command.spawn().ok()?;
    {
        let mut stdin = child.stdin.take()?;
        stdin.write_all(prompt.as_bytes()).ok()?;
        // stdin drops here → EOF, the CLI starts generating.
    }

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    }

    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Validate and normalize a synthesized body. Rejects output that is too
/// short/long, carries its own frontmatter, or lacks the required sections.
pub(crate) fn validate_body(raw: &str) -> Option<String> {
    let mut body = raw.trim();

    // Strip a single wrapping code fence if the model ignored instructions.
    if body.starts_with("```") {
        let after_open = body.split_once('\n')?.1;
        body = after_open.strip_suffix("```").unwrap_or(after_open).trim();
    }

    // The body must not smuggle in frontmatter — we assemble our own.
    if body.starts_with("---") {
        return None;
    }
    if body.chars().count() < BODY_MIN_CHARS || body.chars().count() > BODY_MAX_CHARS {
        return None;
    }
    if !body.contains("## Process") || !body.contains("## Red Flags") {
        return None;
    }
    Some(sanitize_skill_content(body))
}

/// Wrap a validated body in canonical frontmatter. The `llm` origin marker
/// distinguishes synthesized skills in meta.json and the rejected buffer.
fn assemble_skill(name: &str, origin: &str, body: &str) -> String {
    sanitize_skill_content(&format!(
        "---\nname: {name}\ndescription: \"Auto-evolved ({origin}, LLM-synthesized from session failure evidence).\"\n---\n\n# {name}\n\n{body}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_body() -> String {
        format!(
            "## Process\n1. Run cargo check before editing src/store.\n\n\
## Anti-Rationalization\n| Excuse | Rebuttal |\n|---|---|\n| It compiled locally | CI uses --all-features |\n\n\
## Evidence Required\n- [ ] cargo test passes\n\n\
## Red Flags\n- E0308 mismatched types in store/metrics.rs\n{}",
            " ".repeat(0)
        )
    }

    #[test]
    fn validate_accepts_well_formed_body() {
        let body = valid_body();
        let out = validate_body(&body).expect("valid body accepted");
        assert!(out.contains("## Process"));
        assert!(out.contains("## Red Flags"));
    }

    #[test]
    fn validate_strips_wrapping_code_fence() {
        let fenced = format!("```markdown\n{}\n```", valid_body());
        let out = validate_body(&fenced).expect("fenced body accepted");
        assert!(!out.starts_with("```"));
        assert!(out.contains("## Process"));
    }

    #[test]
    fn validate_rejects_frontmatter_smuggling() {
        let smuggled = format!("---\nname: evil\n---\n{}", valid_body());
        assert!(validate_body(&smuggled).is_none());
    }

    #[test]
    fn validate_rejects_too_short() {
        assert!(validate_body("## Process\n## Red Flags").is_none());
    }

    #[test]
    fn validate_rejects_missing_sections() {
        let no_sections = "x".repeat(200);
        assert!(validate_body(&no_sections).is_none());
    }

    #[test]
    fn assemble_produces_canonical_frontmatter() {
        let skill = assemble_skill("evo-fix-test-fail", "high_freq_error", &valid_body());
        assert!(skill.starts_with("---\nname: evo-fix-test-fail\n"));
        assert!(skill.contains("LLM-synthesized"));
        assert!(skill.contains("## Process"));
    }

    #[test]
    fn prompt_embeds_evidence() {
        let analysis = crate::shared::evolution::SessionAnalysis {
            error_snippets: vec!["[test_fail x4] assertion failed: left == right".into()],
            per_error_stats: [("test_fail".to_string(), 4u64)].into_iter().collect(),
            ..Default::default()
        };
        let prompt = build_prompt("evo-fix-test-fail", &analysis);
        assert!(prompt.contains("assertion failed"));
        assert!(prompt.contains("test_fail: 4"));
        assert!(prompt.contains("## Process"));
    }

    #[test]
    fn synthesis_disabled_in_child_env() {
        // SAFETY: EPIC_SYNTH_CHILD is only touched by this single test, so
        // the mutation cannot race with other tests.
        unsafe {
            std::env::set_var(SYNTH_CHILD_ENV, "1");
            assert!(!synthesis_enabled());
            std::env::remove_var(SYNTH_CHILD_ENV);
        }
    }
}
