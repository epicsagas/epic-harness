#![allow(dead_code)]

//! edits.rs — HarnessX-inspired typed edit operations
//!
//! The `HarnessEdit` API and its manifest are not yet invoked from the reflect
//! loop (that wiring lands when typed edits replace the direct SKILL.md
//! writes in `seed_smart_skills`). They are exposed now so the Planner's
//! edit-type coverage analysis has the full taxonomy to compare against and
//! so R6 variant isolation can branch on edit kind.
//!
//! Each candidate harness adaptation is a typed [`HarnessEdit`] value rather
//! than an opaque "write SKILL.md" action (HarnessX paper §4.3 Evolver: "each
//! candidate is specified as a typed builder operation… with a change
//! manifest"). An enum (not a trait object) is used for dispatch — per the
//! architecture review this is more idiomatic in Rust, cheaper to clone when
//! variants fork, and lets the variant-isolation gate branch on edit kind.
//!
//! Today only `AddSkill` is produced by the evolution loop. The other
//! `EditType` taxonomy buckets (modify_skill, add_instinct, modify_config,
//! add_guard_rule) survive in `shared::evolution::EditType` for parsing
//! persisted history and computing the Planner's edit-type coverage; no
//! concrete editors exist for them — dead variants were removed rather than
//! kept as unexercised scaffolding.
//!
//! ## Why enum over trait
//! The paper uses a trait-like abstraction, but epic-harness edits are
//! infrequent (≤ a handful per session), so dynamic dispatch buys nothing and
//! loses exhaustive matching. The variant-isolation gate (R6) must branch on
//! edit kind; an enum makes that a `match` rather than a downcast.

use crate::shared::evolution::EditType;

/// A typed, manifest-carrying harness edit.
#[derive(Debug, Clone)]
pub enum HarnessEdit {
    /// Create a new evolved skill (SKILL.md + meta.json).
    AddSkill {
        name: String,
        content: String,
        origin: String,
        confidence: f64,
    },
}

/// The behavioral effect of applying an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    /// Edit applied successfully.
    Applied,
    /// Edit was a no-op (e.g. target missing).
    Skipped(String),
}

/// A falsifiable change manifest (HarnessX paper Table 9 / §10.3).
///
/// Every shipped edit carries one. They are PERSISTED (reflect writes them to
/// the EvolutionRecord + the sidecar manifests.jsonl). The consumer that
/// verifies a prior round's prediction against the current trace is a
/// DEFERRED follow-up — today the Critic only consults the in-round
/// reward-hacking flag, so manifests accumulate as a ledger without yet
/// closing the falsifiability loop.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditManifest {
    pub edit_type: EditType,
    pub target: String,
    pub intended_effect: String,
    pub predicted_impact: String,
}

impl HarnessEdit {
    /// The taxonomy bucket this edit belongs to.
    pub fn edit_type(&self) -> EditType {
        match self {
            HarnessEdit::AddSkill { .. } => EditType::AddSkill,
        }
    }

    /// Human-readable target (skill name, config key, or guard pattern).
    pub fn target(&self) -> &str {
        match self {
            HarnessEdit::AddSkill { name, .. } => name,
        }
    }

    /// Build a falsifiable manifest for this edit.
    pub fn manifest(&self) -> EditManifest {
        match self {
            HarnessEdit::AddSkill {
                name,
                origin,
                confidence,
                ..
            } => EditManifest {
                edit_type: EditType::AddSkill,
                target: name.clone(),
                intended_effect: format!("New evolved skill from {origin} pattern"),
                predicted_impact: format!(
                    "Lift avg_score_with by reducing {origin} failures (confidence {confidence:.2})"
                ),
            },
        }
    }

    /// Apply the edit: write the evolved skill via the shared writer.
    pub fn apply(&self) -> EditOutcome {
        match self {
            HarnessEdit::AddSkill {
                name,
                content,
                origin,
                confidence,
            } => {
                super::skills::write_skill_with_meta(name, content, origin, *confidence);
                EditOutcome::Applied
            }
        }
    }

    /// Validate the edit without applying (name safety, non-empty content).
    pub fn validate(&self) -> Result<(), String> {
        match self {
            HarnessEdit::AddSkill { name, content, .. } => {
                if !super::skills::sanitize_skill_name(name) {
                    return Err(format!("invalid skill name: {name}"));
                }
                if content.trim().is_empty() {
                    return Err("skill content is empty".into());
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_type_maps_addskill() {
        let edit = HarnessEdit::AddSkill {
            name: "x".into(),
            content: "c".into(),
            origin: "o".into(),
            confidence: 0.5,
        };
        assert_eq!(edit.edit_type(), EditType::AddSkill);
        assert_eq!(edit.target(), "x");
    }

    #[test]
    fn manifest_is_falsifiable() {
        let edit = HarnessEdit::AddSkill {
            name: "rust-borrow-checker".into(),
            content: "c".into(),
            origin: "type_error".into(),
            confidence: 0.8,
        };
        let m = edit.manifest();
        assert_eq!(m.edit_type, EditType::AddSkill);
        assert_eq!(m.target, "rust-borrow-checker");
        assert!(m.intended_effect.contains("type_error"));
        assert!(m.predicted_impact.contains("0.80"));
    }

    #[test]
    fn validate_rejects_bad_skill_name() {
        let edit = HarnessEdit::AddSkill {
            name: "../escape".into(),
            content: "c".into(),
            origin: "o".into(),
            confidence: 0.5,
        };
        assert!(edit.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_content() {
        let edit = HarnessEdit::AddSkill {
            name: "ok-name".into(),
            content: "   ".into(),
            origin: "o".into(),
            confidence: 0.5,
        };
        assert!(edit.validate().is_err());
    }
}
