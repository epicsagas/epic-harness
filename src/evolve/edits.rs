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
//! evolution loop. `AddGuardRule` has a concrete editor (appends to
//! `guard-rules.yaml`) but is not yet emitted by the Planner — the Planner
//! still lists `add_guard_rule` as an untried edit type; once a follow-up
//! wires auto-emission, the editor is ready. `ModifyConfig` and `AddInstinct`
//! remain reserved so the Planner's edit-type coverage analysis has the full
//! taxonomy to compare against (exposing under-exploration), with `apply()`
//! returning `NotImplemented` until a concrete editor lands.
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
    /// Add a `guard-rules.yaml` pattern. Concrete editor (appends a blocked or
    /// warned rule via [`crate::hooks::guard::append_guard_rule`]); not yet
    /// auto-emitted by the Planner.
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
                    "Lift avg_score_with by reducing {origin} failures (confidence {confidence:.2})"
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

    /// Apply the edit. Only SKILL.md-touching variants and `AddGuardRule` are
    /// concrete today; config/instinct edits are reserved and report
    /// `NotImplemented`.
    ///
    /// `AddGuardRule` appends a pattern to the project's `guard-rules.yaml`
    /// (resolved via [`crate::shared::paths::guard_rules_file`], with the same
    /// project-local-then-harness-dir fallback the guard hook itself uses).
    /// It does NOT auto-wire from the Planner yet — the Planner still lists
    /// `add_guard_rule` as an untried edit type; making the editor concrete
    /// first lets a follow-up emit it safely.
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
            HarnessEdit::AddGuardRule {
                pattern,
                level,
                msg,
            } => {
                let path = crate::shared::paths::guard_rules_file();
                match crate::hooks::guard::append_guard_rule(&path, level, pattern, msg) {
                    Ok(()) => EditOutcome::Applied,
                    Err(e) => EditOutcome::Skipped(format!("guard-rules.yaml append failed: {e}")),
                }
            }
            HarnessEdit::AddInstinct { .. } | HarnessEdit::ModifyConfig { .. } => {
                EditOutcome::NotImplemented
            }
        }
    }

    /// Validate the edit without applying (name safety, non-empty content,
    /// guard pattern sanity).
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
            HarnessEdit::AddGuardRule { pattern, .. } => {
                if pattern.trim().is_empty() {
                    return Err("guard rule pattern is empty".into());
                }
                if pattern.len() > crate::hooks::guard::GUARD_PATTERN_MAX_LEN {
                    return Err(format!(
                        "guard rule pattern too long ({} > {} chars) — \
                         pathological regex would slow every guard run",
                        pattern.len(),
                        crate::hooks::guard::GUARD_PATTERN_MAX_LEN
                    ));
                }
                Ok(())
            }
            // Reserved edits pass validation (they no-op on apply).
            HarnessEdit::AddInstinct { .. } | HarnessEdit::ModifyConfig { .. } => Ok(()),
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
    fn validate_rejects_empty_guard_pattern() {
        let edit = HarnessEdit::AddGuardRule {
            pattern: "   ".into(),
            level: "block".into(),
            msg: "no".into(),
        };
        assert!(edit.validate().is_err());
    }

    #[test]
    fn validate_rejects_pathologically_long_guard_pattern() {
        let edit = HarnessEdit::AddGuardRule {
            pattern: "a".repeat(crate::hooks::guard::GUARD_PATTERN_MAX_LEN + 1),
            level: "block".into(),
            msg: "no".into(),
        };
        assert!(edit.validate().is_err());
    }

    #[test]
    fn validate_accepts_well_formed_guard_rule() {
        let edit = HarnessEdit::AddGuardRule {
            pattern: r"rm\s+-rf".into(),
            level: "warn".into(),
            msg: "careful".into(),
        };
        assert!(edit.validate().is_ok());
    }

    /// `AddGuardRule::apply()` is concrete: it appends to `guard-rules.yaml`.
    /// We isolate the filesystem by pointing HOME at a tempdir and seeding a
    /// local `.harness/guard-rules.yaml` so `guard_rules_file()` resolves to a
    /// path inside the tempdir regardless of the cached `harness_dir()` slug.
    #[test]
    #[serial_test::serial]
    fn add_guard_rule_apply_writes_rule_to_file() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        // Seed a project-local guard-rules.yaml so guard_rules_file() returns
        // this path (local-file-wins branch), keeping the write inside tempdir.
        let local_harness = dir.path().join(".harness");
        std::fs::create_dir_all(&local_harness).unwrap();
        let rules_path = local_harness.join("guard-rules.yaml");
        let mut f = std::fs::File::create(&rules_path).unwrap();
        writeln!(f, "warned:\n  - pattern: existing | msg: pre-existing rule").unwrap();
        drop(f);

        // Point cwd at the tempdir so local_harness_dir() resolves inside it.
        let saved_cwd = std::env::current_dir().ok();
        let _ = std::env::set_current_dir(dir.path());

        let edit = HarnessEdit::AddGuardRule {
            pattern: r"kubectl\s+delete".into(),
            level: "block".into(),
            msg: "kubectl delete blocked".into(),
        };
        let outcome = edit.apply();
        assert_eq!(outcome, EditOutcome::Applied);

        // The new rule landed in the file alongside the pre-existing one.
        let content = std::fs::read_to_string(&rules_path).unwrap();
        assert!(
            content.contains(r"kubectl\s+delete"),
            "new rule missing: {content}"
        );
        assert!(content.contains("kubectl delete blocked"));
        assert!(
            content.contains("pre-existing rule"),
            "existing rule clobbered: {content}"
        );

        if let Some(cwd) = saved_cwd {
            let _ = std::env::set_current_dir(cwd);
        }
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
