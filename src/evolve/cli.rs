//! evolve CLI — host-agent skill synthesis handshake.
//!
//! `epic-harness evolve accept-synth` is the consume side of the
//! pending-synthesis manifest protocol (`src/evolve/synthesis.rs`). A host
//! agent (claude/codex/agy, using its own subagent mechanism with no model
//! specified) reads a pending manifest, synthesizes a better skill body from
//! the evidence, and pipes the body here. We validate it, re-run the Critic
//! falsifiability gate, overwrite the template skill, and mark the manifest
//! consumed. Host-agnostic: this code never names a CLI or model.

use std::fs;
use std::io::{IsTerminal, Read};

use crate::evolve::critic::{Critic, CriticVerdict};
use crate::evolve::edits::EditManifest;
use crate::evolve::skills::{gate_skills, write_skill_with_meta};
use crate::evolve::synthesis::{assemble_skill, find_pending, mark_consumed, validate_body};
use crate::shared::evolution::{EditType, Metrics};
use crate::shared::helpers::{append_jsonl, hint, read_json};
use crate::shared::paths::{evolved_dir, manifests_file, metrics_file};

const EXIT_OK: i32 = 0;
const EXIT_NO_MANIFEST: i32 = 2;
const EXIT_VALIDATE: i32 = 3;
const EXIT_DOWNGRADE: i32 = 4;
const EXIT_CRITIC: i32 = 5;
const EXIT_IO: i32 = 6;
const EXIT_USAGE: i32 = 64;

/// Entry point. `args[0]` is `"evolve"`.
pub fn run(args: &[String]) -> i32 {
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("");
    match sub {
        "accept-synth" => run_accept_synth(&args[2..]),
        "" => {
            eprintln!(
                "usage: epic-harness evolve accept-synth --skill <name> [--file <path> | --stdin]"
            );
            EXIT_USAGE
        }
        other => {
            eprintln!("unknown evolve subcommand: '{other}'");
            eprintln!("available: accept-synth");
            EXIT_USAGE
        }
    }
}

fn run_accept_synth(args: &[String]) -> i32 {
    let skill = match flag(args, "--skill") {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!("missing required --skill <name>");
            return EXIT_USAGE;
        }
    };
    let file = flag(args, "--file");
    let stdin_flag = args.iter().any(|a| a == "--stdin");

    if file.is_some() && stdin_flag {
        eprintln!("pass either --file or --stdin, not both");
        return EXIT_USAGE;
    }

    // 1. Read the synthesized body.
    let raw = if let Some(path) = file {
        match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read '{path}': {e}");
                return EXIT_IO;
            }
        }
    } else if stdin_flag || !std::io::stdin().is_terminal() {
        let mut s = String::new();
        if std::io::stdin().read_to_string(&mut s).is_err() {
            eprintln!("cannot read body from stdin");
            return EXIT_IO;
        }
        s
    } else {
        eprintln!("provide the synthesized body via --file <path> or --stdin");
        return EXIT_USAGE;
    };

    // 2. Resolve the pending manifest (join key = skill name).
    let pending = match find_pending(&skill) {
        Some(p) => p,
        None => {
            eprintln!("no pending-synthesis manifest for skill '{skill}'");
            return EXIT_NO_MANIFEST;
        }
    };

    // 3. Validate the body.
    let body = match validate_body(&raw) {
        Some(b) => b,
        None => {
            eprintln!(
                "body rejected: must be 80-6000 chars, no smuggled frontmatter, and contain '## Process' and '## Red Flags'"
            );
            return EXIT_VALIDATE;
        }
    };

    // 4. Reassemble canonical frontmatter.
    let assembled = assemble_skill(&skill, &pending.origin, &body);

    // 5. Higher-confidence guard: refuse to downgrade a skill that a later
    //    reflect re-seeded at higher confidence.
    let existing_conf = fs::read_to_string(evolved_dir().join(&skill).join("meta.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("confidence").and_then(|c| c.as_f64()));
    if let Some(c) = existing_conf {
        if c > pending.confidence + 1e-9 {
            eprintln!(
                "existing skill '{skill}' has higher confidence ({c:.2} > {:.2}); refusing to downgrade",
                pending.confidence
            );
            return EXIT_DOWNGRADE;
        }
    }

    // 6. Critic falsifiability gate — same gate as seed time. reward hacking
    //    suppresses a claimed score lift.
    let manifest = EditManifest {
        edit_type: EditType::AddSkill,
        target: skill.clone(),
        intended_effect: format!(
            "Upgrade evolved skill {skill} with a synthesized body ({})",
            pending.origin
        ),
        predicted_impact: format!(
            "Lift avg_score_with by reducing {} failures (confidence {:.2})",
            pending.origin, pending.confidence
        ),
    };
    let metrics: Metrics = read_json(&metrics_file(), Metrics::default());
    if let CriticVerdict::Reject(reason) = Critic::verify_against_evidence(&manifest, &[], &metrics)
    {
        eprintln!("critic rejected: {reason}");
        return EXIT_CRITIC;
    }

    // 7. Apply, gate, mark consumed, ledger.
    write_skill_with_meta(&skill, &assembled, &pending.origin, pending.confidence);
    gate_skills();
    mark_consumed(&skill);
    append_jsonl(&manifests_file(), &manifest);
    hint(
        "evolve",
        &format!("synthesized body accepted for skill '{skill}'"),
    );
    EXIT_OK
}

/// Minimal `--flag value` / `--flag=value` parser (mirrors main.rs).
fn flag(args: &[String], name: &str) -> Option<String> {
    let eq = format!("{name}=");
    args.iter()
        .find(|a| a.starts_with(&eq))
        .map(|a| a[eq.len()..].to_string())
        .or_else(|| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        })
}
