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
//! Today only `AddSkill` and `ModifySkill` (auto-tune) are produced by the
//! evolution loop. `ModifyConfig`, `AddGuardRule`, and `AddInstinct` are
//! declared so the Planner's edit-type coverage analysis has the full taxonomy
//! to compare against (exposing under-exploration), with `apply()` returning
//! `NotImplemented` until a concrete editor lands.
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
    /// Append a tuning section to an existing skill's SKILL.md.
    ModifySkill {
        skill_name: String,
        section: String,
        new_content: String,
    },
    /// Promote a high-confidence pattern to global memory (instinct).
    AddInstinct {
        trigger: String,
        body: String,
        confidence: f64,
    },
    /// Change a config.toml threshold (not yet implemented; reserved for coverage).
    ModifyConfig {
        key: String,
        old_value: String,
        new_value: String,
    },
    /// Add a guard-rules.yaml pattern (not yet implemented; reserved for coverage).
    AddGuardRule {
        pattern: String,
        level: String,
        msg: String,
    },
}

/// The behavioral effect of applying an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    /// Edit applied successfully.
    Applied,
    /// Edit type is reserved but has no concrete editor yet.
    NotImplemented,
    /// Edit was a no-op (e.g. target missing).
    Skipped(String),
}

/// A falsifiable change manifest (HarnessX paper Table 9 / §10.3).
///
/// Every shipped edit should carry one so the Critic (R-P2) can later verify
/// the next round's trace matches the predicted effect.
#[derive(Debug, Clone)]
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
            HarnessEdit::ModifySkill { .. } => EditType::ModifySkill,
            HarnessEdit::AddInstinct { .. } => EditType::AddInstinct,
            HarnessEdit::ModifyConfig { .. } => EditType::ModifyConfig,
            HarnessEdit::AddGuardRule { .. } => EditType::AddGuardRule,
        }
    }

    /// Human-readable target (skill name, config key, or guard pattern).
    pub fn target(&self) -> &str {
        match self {
            HarnessEdit::AddSkill { name, .. } => name,
            HarnessEdit::ModifySkill { skill_name, .. } => skill_name,
            HarnessEdit::AddInstinct { trigger, .. } => trigger,
            HarnessEdit::ModifyConfig { key, .. } => key,
            HarnessEdit::AddGuardRule { pattern, .. } => pattern,
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
                    "Reduce failures of {origin} (confidence {confidence:.2})"
                ),
            },
            HarnessEdit::ModifySkill {
                skill_name,
                section,
                ..
            } => EditManifest {
                edit_type: EditType::ModifySkill,
                target: skill_name.clone(),
                intended_effect: format!("Auto-tune {section} of {skill_name}"),
                predicted_impact: format!("Lift avg_score_with for {skill_name}"),
            },
            HarnessEdit::AddInstinct { trigger, .. } => EditManifest {
                edit_type: EditType::AddInstinct,
                target: trigger.clone(),
                intended_effect: "Promote pattern to global memory".into(),
                predicted_impact: format!("Recall {trigger} cross-project"),
            },
            HarnessEdit::ModifyConfig {
                key,
                old_value,
                new_value,
            } => EditManifest {
                edit_type: EditType::ModifyConfig,
                target: key.clone(),
                intended_effect: format!("Adjust {key}: {old_value} -> {new_value}"),
                predicted_impact: format!("Shift {key} threshold"),
            },
            HarnessEdit::AddGuardRule { pattern, level, .. } => EditManifest {
                edit_type: EditType::AddGuardRule,
                target: pattern.clone(),
                intended_effect: format!("Block/warn on /{pattern}/ at {level}"),
                predicted_impact: "Prevent observed dangerous pattern".into(),
            },
        }
    }

    /// Apply the edit. Only SKILL.md-touching variants are concrete today;
    /// config/guard edits are reserved and report `NotImplemented`.
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
            HarnessEdit::ModifySkill {
                skill_name,
                section,
                new_content,
            } => {
                // Combine the section heading and body into the single tuning
                // section string the underlying editor expects.
                let combined = if section.is_empty() {
                    new_content.clone()
                } else {
                    format!("## {section}\n{new_content}")
                };
                super::skills::append_tuning_section_pub(skill_name, &combined);
                EditOutcome::Applied
            }
            HarnessEdit::AddInstinct { .. }
            | HarnessEdit::ModifyConfig { .. }
            | HarnessEdit::AddGuardRule { .. } => EditOutcome::NotImplemented,
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
            HarnessEdit::ModifySkill {
                skill_name,
                new_content,
                ..
            } => {
                if !super::skills::sanitize_skill_name(skill_name) {
                    return Err(format!("invalid skill name: {skill_name}"));
                }
                if new_content.trim().is_empty() {
                    return Err("tuning content is empty".into());
                }
                Ok(())
            }
            // Reserved edits pass validation (they no-op on apply).
            HarnessEdit::AddInstinct { .. }
            | HarnessEdit::ModifyConfig { .. }
            | HarnessEdit::AddGuardRule { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_type_maps_each_variant() {
        assert_eq!(
            HarnessEdit::AddSkill {
                name: "x".into(),
                content: "c".into(),
                origin: "o".into(),
                confidence: 0.5
            }
            .edit_type(),
            EditType::AddSkill
        );
        assert_eq!(
            HarnessEdit::ModifySkill {
                skill_name: "x".into(),
                section: "s".into(),
                new_content: "c".into()
            }
            .edit_type(),
            EditType::ModifySkill
        );
        assert_eq!(
            HarnessEdit::AddInstinct {
                trigger: "t".into(),
                body: "b".into(),
                confidence: 0.5
            }
            .edit_type(),
            EditType::AddInstinct
        );
        assert_eq!(
            HarnessEdit::ModifyConfig {
                key: "k".into(),
                old_value: "a".into(),
                new_value: "b".into()
            }
            .edit_type(),
            EditType::ModifyConfig
        );
        assert_eq!(
            HarnessEdit::AddGuardRule {
                pattern: "p".into(),
                level: "warn".into(),
                msg: "m".into()
            }
            .edit_type(),
            EditType::AddGuardRule
        );
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

    #[test]
    fn validate_accepts_reserved_edits() {
        let edit = HarnessEdit::ModifyConfig {
            key: "k".into(),
            old_value: "a".into(),
            new_value: "b".into(),
        };
        assert!(edit.validate().is_ok());
    }

    #[test]
    fn reserved_edits_report_not_implemented() {
        let edit = HarnessEdit::AddGuardRule {
            pattern: "rm -rf".into(),
            level: "block".into(),
            msg: "no".into(),
        };
        assert_eq!(edit.apply(), EditOutcome::NotImplemented);
    }

    #[test]
    fn target_exposes_relevant_field() {
        assert_eq!(
            HarnessEdit::ModifyConfig {
                key: "stagnation_limit".into(),
                old_value: "3".into(),
                new_value: "5".into()
            }
            .target(),
            "stagnation_limit"
        );
    }
}
