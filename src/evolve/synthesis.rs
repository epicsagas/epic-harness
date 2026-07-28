//! Host-agnostic skill synthesis (Ring 3, the step templates could never do).
//!
//! The template builders in `skills.rs` emit the same generic advice for any
//! failure. This module records a **pending-synthesis manifest** for each
//! seeded skill, carrying the session's REAL failure evidence (error snippets,
//! category counts, detected patterns) plus the template body. A host agent
//! (claude/codex/agy) — using ITS OWN subagent mechanism, with no model
//! specified — reads the manifest, synthesizes a better body, and hands it back
//! via `epic-harness evolve accept-synth`, which runs the synthesized body
//! through the exact same gates as template content: `validate_body`, the
//! Critic falsifiability gate, and `gate_skills()`.
//!
//! If no host ever runs `accept-synth`, the template body persists — synthesis
//! can only improve a skill, never block seeding.
//!
//! Host-agnostic by construction: this module references no CLI binary and no
//! model name. The previous design spawned a synchronous `claude -p` subprocess
//! from the SessionEnd `reflect` hook; that hung under slow/remote hosts and
//! coupled the harness to one CLI. The manifest protocol removes the subprocess
//! entirely — Rust only reads and writes JSONL.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::evolve::edits::HarnessEdit;
use crate::shared::evolution::{DetectedPattern, SessionAnalysis};
use crate::shared::helpers::now_iso;
use crate::shared::paths::pending_synth_file;
use crate::shared::sanitize::sanitize_skill_content;

/// Hard bounds on an accepted synthesized body.
const BODY_MIN_CHARS: usize = 80;
const BODY_MAX_CHARS: usize = 6_000;
/// A bounded durable synthesis backlog. We reject new work at capacity rather
/// than silently losing an unconsumed pending request.
const MAX_PENDING_SYNTH_RECORDS: usize = 256;
const MAX_PENDING_SYNTH_LINE_BYTES: usize = 16 * 1024;
static SYNTHESIS_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Failure evidence packaged for a host agent. All fields are already
/// secret-masked and sanitized at analysis time (`collect_error_snippets`),
/// so they are safe to hand to any host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthEvidence {
    /// Representative error snippets (one per failure category), cap 8.
    #[serde(default)]
    pub error_snippets: Vec<String>,
    /// Failure category → count, highest-count first, cap 8.
    #[serde(default)]
    pub failure_category_counts: HashMap<String, u64>,
    /// Detected failure patterns, cap 4.
    #[serde(default)]
    pub failure_patterns: Vec<DetectedPattern>,
}

/// One pending-synthesis record. Emitted by `reflect`'s `upgrade_edits`,
/// consumed by `epic-harness evolve accept-synth`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSynth {
    /// Reflection session identity. Legacy records deserialize with no id.
    #[serde(default)]
    pub session_id: String,
    pub schema_version: u32,
    /// Join key — matches `AddSkill.name`.
    pub skill_name: String,
    pub origin: String,
    /// From `AddSkill.confidence`; used by the higher-confidence guard.
    pub confidence: f64,
    pub evidence: SynthEvidence,
    /// Full SKILL.md (frontmatter + body) the planner produced — the fallback
    /// if no host ever synthesizes.
    pub template_content: String,
    /// Host-neutral instructions the synthesizing subagent follows.
    pub prompt_guidance: String,
    pub created: String,
    /// "pending" | "synthesized".
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub consumed: Option<String>,
}

fn default_status() -> String {
    "pending".to_string()
}

/// Whether synthesis may run in this process — governed solely by config.
/// (No subprocess anymore, so no recursion guard or debug-disable is needed;
/// manifest emission is deterministic and testable.)
pub fn synthesis_enabled() -> bool {
    CONFIG.evolution.llm_synthesis
}

/// Emit a pending-synthesis manifest for up to `llm_synthesis_max_per_session`
/// AddSkill edits. The edits keep their template `content` unchanged — the
/// manifest is the upgrade channel, not the seed. Returns how many manifests
/// were emitted.
pub fn upgrade_edits(
    edits: &mut [HarnessEdit],
    analysis: &SessionAnalysis,
    reflection_session_id: &str,
) -> std::io::Result<usize> {
    if !synthesis_enabled() {
        return Ok(0);
    }
    let budget = CONFIG.evolution.llm_synthesis_max_per_session;
    let evidence = extract_evidence(analysis);
    let mut candidates = Vec::new();
    for edit in edits.iter_mut() {
        if candidates.len() >= budget {
            break;
        }
        if let HarnessEdit::AddSkill {
            name,
            content,
            origin,
            confidence,
        } = edit
        {
            let pending = PendingSynth {
                session_id: reflection_session_id.to_string(),
                schema_version: 1,
                skill_name: name.clone(),
                origin: origin.clone(),
                confidence: *confidence,
                evidence: evidence.clone(),
                template_content: content.clone(),
                prompt_guidance: prompt_guidance(name),
                created: now_iso(),
                status: "pending".into(),
                consumed: None,
            };
            candidates.push(pending);
        }
    }
    merge_pending_once(&pending_synth_file(), candidates)
}

fn merge_pending_once(path: &Path, candidates: Vec<PendingSynth>) -> std::io::Result<usize> {
    if candidates.is_empty() {
        return Ok(0);
    }
    let lock = path.with_extension("jsonl.lock");
    let _lock = crate::orchestrate::state::acquire_lock(&lock)?;
    let records = read_pending_records(path)?;
    let mut merged = records.clone();
    let mut added = 0;
    for candidate in candidates {
        if !records.iter().any(|record| {
            record.session_id == candidate.session_id && record.skill_name == candidate.skill_name
        }) {
            merged.push(candidate);
            added += 1;
        }
    }
    if merged.len() > MAX_PENDING_SYNTH_RECORDS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "pending synthesis backlog is full; consume pending manifests before adding more",
        ));
    }
    if added > 0 {
        rewrite_jsonl_checked(path, &merged)?;
    }
    Ok(added)
}

/// Pull the bounded evidence a host agent needs out of a session analysis.
/// Caps mirror the old `build_prompt` so manifests stay compact.
pub fn extract_evidence(analysis: &SessionAnalysis) -> SynthEvidence {
    let error_snippets = analysis.error_snippets.iter().take(8).cloned().collect();

    let mut counts: Vec<(&String, &u64)> = analysis.per_error_stats.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let failure_category_counts = counts
        .into_iter()
        .take(8)
        .map(|(k, v)| (k.clone(), *v))
        .collect();

    let failure_patterns = analysis.failure_patterns.iter().take(4).cloned().collect();

    SynthEvidence {
        error_snippets,
        failure_category_counts,
        failure_patterns,
    }
}

/// Host-neutral synthesis instructions. No CLI or model name — the host uses
/// its own subagent mechanism.
pub fn prompt_guidance(name: &str) -> String {
    format!(
        "You are writing the body of an agent skill named '{name}'. It will be \
injected into future coding sessions in THIS project to prevent the failures \
recorded in the `evidence` field from recurring.\n\n\
Using the `evidence` (error snippets, failure category counts, detected \
patterns) from the accompanying manifest, write concrete, project-specific \
guidance grounded in those errors — name the actual error messages, files, and \
commands involved. Generic advice (\"read the error carefully\") is worthless \
and will be rejected.\n\n\
Output ONLY markdown body text (no YAML frontmatter, no code fences around the \
whole output, no preamble) with exactly these sections:\n\
## Process\n(numbered steps tied to the specific failures)\n\
## Anti-Rationalization\n(markdown table: Excuse | Rebuttal)\n\
## Evidence Required\n(checklist proving the failure class is fixed)\n\
## Red Flags\n(bullet list of early warnings specific to these errors)\n\n\
Hard limit: 60 lines."
    )
}

/// Find the most recent pending manifest for a skill (the join key). If
/// `reflect` ran more than once before `accept-synth` consumed the backlog,
/// several pending records can accumulate for the same skill — the newest
/// carries the freshest failure evidence, so it wins. `mark_consumed` still
/// marks every pending record for the skill as consumed, so older records
/// never resurface as separate synthesis targets.
pub fn find_pending(skill_name: &str) -> Option<PendingSynth> {
    read_pending_records(&pending_synth_file())
        .ok()?
        .into_iter()
        .filter(|r| r.status == "pending" && r.skill_name == skill_name)
        .max_by(|a, b| a.created.cmp(&b.created))
}

/// Mark every pending manifest for a skill as synthesized. Idempotent — a
/// second call finds no pending record and does nothing.
pub fn mark_consumed(skill_name: &str) -> std::io::Result<usize> {
    mark_consumed_at(&pending_synth_file(), skill_name)
}

fn mark_consumed_at(path: &Path, skill_name: &str) -> std::io::Result<usize> {
    let lock = path.with_extension("jsonl.lock");
    let _lock = crate::orchestrate::state::acquire_lock(&lock)?;
    let mut records = read_pending_records(path)?;
    if records.is_empty() {
        return Ok(0);
    }
    let now = now_iso();
    let mut changed = 0usize;
    for r in records.iter_mut() {
        if r.status == "pending" && r.skill_name == skill_name {
            r.status = "synthesized".into();
            r.consumed = Some(now.clone());
            changed += 1;
        }
    }
    if changed > 0 {
        rewrite_jsonl_checked(path, &records)?;
    }
    Ok(changed)
}

fn read_pending_records(path: &Path) -> std::io::Result<Vec<PendingSynth>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut records = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_PENDING_SYNTH_LINE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "pending synthesis line exceeds limit",
            ));
        }
        records.push(serde_json::from_str(&line).map_err(std::io::Error::other)?);
        if records.len() > MAX_PENDING_SYNTH_RECORDS {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "pending synthesis backlog exceeds limit",
            ));
        }
    }
    Ok(records)
}

fn rewrite_jsonl_checked(path: &Path, records: &[PendingSynth]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pending synthesis path has no parent",
        )
    })?;
    let tmp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        SYNTHESIS_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    for r in records {
        let json = serde_json::to_string(r).map_err(std::io::Error::other)?;
        writeln!(f, "{json}")?;
    }
    f.sync_all()?;
    if let Err(error) = crate::team::codex::atomic_replace_file(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(())
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

/// Wrap a validated body in canonical frontmatter. The origin marker
/// distinguishes synthesized skills in meta.json and the rejected buffer.
pub(crate) fn assemble_skill(name: &str, origin: &str, body: &str) -> String {
    sanitize_skill_content(&format!(
        "---\nname: {name}\ndescription: \"Auto-evolved ({origin}, synthesized from session failure evidence).\"\n---\n\n# {name}\n\n{body}\n"
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

    fn analysis_with(snippets: usize, patterns: usize) -> SessionAnalysis {
        let error_snippets = (0..snippets)
            .map(|i| format!("[cat{i} x9] error message {i}"))
            .collect();
        let per_error_stats = (0..snippets).map(|i| (format!("cat{i}"), 9u64)).collect();
        let failure_patterns = (0..patterns)
            .map(|i| DetectedPattern {
                pattern_type: "repeated_same_error".into(),
                description: format!("desc {i}"),
                count: i as u64 + 1,
                involved_files: vec![format!("src/{i}.rs")],
                suggested_remediation: "fix root cause".into(),
                implicated_components: vec![],
            })
            .collect();
        SessionAnalysis {
            error_snippets,
            per_error_stats,
            failure_patterns,
            ..Default::default()
        }
    }

    #[test]
    fn pending_synth_retry_after_atomic_write_is_not_duplicated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending_synth.jsonl");
        let pending = PendingSynth {
            session_id: "session-a".into(),
            schema_version: 1,
            skill_name: "skill-a".into(),
            origin: "test".into(),
            confidence: 1.0,
            evidence: SynthEvidence {
                error_snippets: vec![],
                failure_category_counts: HashMap::new(),
                failure_patterns: vec![],
            },
            template_content: "content".into(),
            prompt_guidance: "guidance".into(),
            created: "2026-07-28T00:00:00Z".into(),
            status: "pending".into(),
            consumed: None,
        };
        assert_eq!(merge_pending_once(&path, vec![pending.clone()]).unwrap(), 1);
        assert_eq!(merge_pending_once(&path, vec![pending]).unwrap(), 0);
        assert_eq!(read_pending_records(&path).unwrap().len(), 1);
    }

    #[test]
    fn mark_consumed_is_locked_and_reports_the_persisted_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending_synth.jsonl");
        let pending = PendingSynth {
            session_id: "session-a".into(),
            schema_version: 1,
            skill_name: "skill-a".into(),
            origin: "test".into(),
            confidence: 1.0,
            evidence: SynthEvidence {
                error_snippets: vec![],
                failure_category_counts: HashMap::new(),
                failure_patterns: vec![],
            },
            template_content: "content".into(),
            prompt_guidance: "guidance".into(),
            created: "2026-07-28T00:00:00Z".into(),
            status: "pending".into(),
            consumed: None,
        };
        merge_pending_once(&path, vec![pending]).unwrap();

        assert_eq!(mark_consumed_at(&path, "skill-a").unwrap(), 1);
        assert_eq!(mark_consumed_at(&path, "skill-a").unwrap(), 0);
        assert_eq!(
            read_pending_records(&path).unwrap()[0].status,
            "synthesized"
        );
    }

    #[test]
    fn mark_consumed_reports_every_persisted_record_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending_synth.jsonl");
        let mut first = PendingSynth {
            session_id: "session-a".into(),
            schema_version: 1,
            skill_name: "skill-a".into(),
            origin: "test".into(),
            confidence: 1.0,
            evidence: SynthEvidence {
                error_snippets: vec![],
                failure_category_counts: HashMap::new(),
                failure_patterns: vec![],
            },
            template_content: "content".into(),
            prompt_guidance: "guidance".into(),
            created: "2026-01-01T00:00:00Z".into(),
            status: "pending".into(),
            consumed: None,
        };
        let mut second = first.clone();
        let mut other = first.clone();
        other.skill_name = "skill-b".into();
        first.created = "2026-01-01T00:00:00Z".into();
        second.created = "2026-01-01T00:01:00Z".into();
        rewrite_jsonl_checked(&path, &[first, second, other]).unwrap();

        assert_eq!(mark_consumed_at(&path, "skill-a").unwrap(), 2);
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
        assert!(skill.contains("synthesized"));
        assert!(skill.contains("## Process"));
    }

    #[test]
    fn extract_evidence_caps_snippets_and_patterns() {
        let analysis = analysis_with(12, 7);
        let ev = extract_evidence(&analysis);
        assert_eq!(ev.error_snippets.len(), 8);
        assert_eq!(ev.failure_patterns.len(), 4);
        // category counts cap at 8 and are sorted by count desc then name
        assert_eq!(ev.failure_category_counts.len(), 8);
    }

    #[test]
    fn extract_evidence_preserves_category_order() {
        let analysis = SessionAnalysis {
            per_error_stats: [("low".to_string(), 1u64), ("high".to_string(), 9u64)]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let ev = extract_evidence(&analysis);
        assert_eq!(ev.failure_category_counts.get("high"), Some(&9));
        assert_eq!(ev.failure_category_counts.get("low"), Some(&1));
    }

    #[test]
    fn prompt_guidance_is_host_neutral() {
        let g = prompt_guidance("evo-fix-test-fail");
        // Names the skill and required sections...
        assert!(g.contains("evo-fix-test-fail"));
        assert!(g.contains("## Process"));
        assert!(g.contains("## Red Flags"));
        // ...but never a host CLI or model.
        assert!(!g.to_lowercase().contains("claude"));
        assert!(!g.to_lowercase().contains("haiku"));
    }
}
