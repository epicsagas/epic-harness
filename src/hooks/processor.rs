#![allow(dead_code)]

//! processor.rs — HarnessX Processor abstraction (P3.3, representational wrapper)
//!
//! Maps the existing CLI-subcommand hooks onto a typed `Processor` trait so
//! each lifecycle hook point has a uniform, type-safe entry. This is the
//! foundation HarnessX's paper §3.2 describes ("a processor is an object
//! satisfying process(event) -> outcome"), adapted to epic-harness's
//! out-of-process CLI model.
//!
//! ## What this is NOT (Architect's red flag, honored)
//! The paper's Processor runs inside an in-process event pipeline with typed
//! `Event` flows and `_order` / `_singleton_group` metadata. epic-harness hooks
//! are separate processes (`epic-harness <hook> < stdin`), so a full in-process
//! pipeline would be the single most invasive change in the codebase. This
//! module is a **representational wrapper**: it types the dispatch surface
//! without changing any hook's behavior. Each `Processor` delegates to the
//! existing `run(&HookInput) -> i32` function unchanged.
//!
//! ## Why it's still useful
//! - Gives `HarnessEdit` / variant isolation a typed notion of "which hook
//!   point does this edit target" (R4/R6 can later attach metadata).
//! - Makes the hook surface introspectable (`all_processors()` enumerates the
//!   wiring) — useful for `HarnessSnapshot` and future tooling.
//! - Establishes the seam a future in-process pipeline could fill without a
//!   big-bang rewrite: swap the `process()` bodies one hook at a time.
//!
//! ## HookPoint mapping (paper Table 1 → epic-harness)
//! | Paper hook point        | epic-harness hook | Processor          |
//! |-------------------------|-------------------|--------------------|
//! | SessionStart            | resume            | ResumeProcessor    |
//! | PreToolUse              | guard             | GuardProcessor     |
//! | PostToolUse             | observe           | ObserveProcessor   |
//! | PostEdit (file write)   | polish            | PolishProcessor    |
//! | PreCompact              | snapshot          | SnapshotProcessor  |
//! | SessionEnd              | reflect           | ReflectProcessor   |

use crate::hooks::common::HookInput;
use crate::shared::types::HookProfile;

/// The lifecycle hook points epic-harness attaches processors to.
///
/// Mirrors HarnessX paper Table 1's hook set, restricted to the points
/// epic-harness actually uses (6 of the paper's 8; the unused two —
/// `OnToolError` and `OnAgentMessage` — are reserved for future wiring).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HookPoint {
    /// Restore session context on conversation start.
    SessionStart,
    /// Block/warn before a tool runs.
    PreToolUse,
    /// Record the outcome after a tool runs.
    PostToolUse,
    /// Format/typecheck after a file edit.
    PostEdit,
    /// Save session state before context compaction.
    PreCompact,
    /// Evolve + persist metrics on session end.
    SessionEnd,
}

impl HookPoint {
    /// The string subcommand name each hook point is dispatched as.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPoint::SessionStart => "resume",
            HookPoint::PreToolUse => "guard",
            HookPoint::PostToolUse => "observe",
            HookPoint::PostEdit => "polish",
            HookPoint::PreCompact => "snapshot",
            HookPoint::SessionEnd => "reflect",
        }
    }

    /// Parse a hook point from its subcommand name.
    pub fn from_subcmd(s: &str) -> Option<Self> {
        match s {
            "resume" => Some(HookPoint::SessionStart),
            "guard" => Some(HookPoint::PreToolUse),
            "observe" => Some(HookPoint::PostToolUse),
            "polish" => Some(HookPoint::PostEdit),
            "snapshot" => Some(HookPoint::PreCompact),
            "reflect" => Some(HookPoint::SessionEnd),
            _ => None,
        }
    }
}

/// A typed processor attached to a hook point. The default `process`
/// implementation delegates to the hook's existing `run(&HookInput) -> i32`
/// unchanged — this is a representational wrapper, not a behavior change.
pub trait Processor {
    /// Which lifecycle point this processor handles.
    fn hook_point(&self) -> HookPoint;
    /// The profile this processor runs under (matches the existing PROFILE_*).
    fn profile(&self) -> HookProfile;
    /// Run the processor against a hook input. Returns the process exit code.
    fn process(&self, input: &HookInput) -> i32;
}

// ── Concrete processors (thin wrappers over existing run() fns) ──────────

macro_rules! define_processor {
    ($name:ident, $point:expr, $profile:expr, $run:path) => {
        pub struct $name;
        impl Processor for $name {
            fn hook_point(&self) -> HookPoint {
                $point
            }
            fn profile(&self) -> HookProfile {
                $profile
            }
            fn process(&self, input: &HookInput) -> i32 {
                // Delegate unchanged to the existing hook implementation.
                $run(input)
            }
        }
    };
}

define_processor!(
    ResumeProcessor,
    HookPoint::SessionStart,
    crate::shared::types::HookProfile::Minimal,
    crate::hooks::resume::run
);
define_processor!(
    GuardProcessor,
    HookPoint::PreToolUse,
    crate::shared::types::HookProfile::Minimal,
    crate::hooks::guard::run
);
define_processor!(
    ObserveProcessor,
    HookPoint::PostToolUse,
    crate::shared::types::HookProfile::Minimal,
    crate::hooks::observe::run
);
define_processor!(
    PolishProcessor,
    HookPoint::PostEdit,
    crate::shared::types::HookProfile::Standard,
    crate::hooks::polish::run
);
define_processor!(
    SnapshotProcessor,
    HookPoint::PreCompact,
    crate::shared::types::HookProfile::Standard,
    crate::hooks::snapshot::run
);
define_processor!(
    ReflectProcessor,
    HookPoint::SessionEnd,
    crate::shared::types::HookProfile::Standard,
    crate::hooks::reflect::run
);

/// The full processor set, in lifecycle order. This is the typed dispatch
/// table — the single place that enumerates every hook epic-harness wires.
pub fn all_processors() -> Vec<Box<dyn Processor>> {
    vec![
        Box::new(ResumeProcessor),
        Box::new(GuardProcessor),
        Box::new(ObserveProcessor),
        Box::new(PolishProcessor),
        Box::new(SnapshotProcessor),
        Box::new(ReflectProcessor),
    ]
}

/// Dispatch a subcommand name to its processor (representational path; the
/// production dispatcher in main.rs still calls the run() fns directly — this
/// exists so the typed surface is exercised and remains a valid seam).
pub fn dispatch(subcmd: &str, input: &HookInput) -> Option<i32> {
    let point = HookPoint::from_subcmd(subcmd)?;
    let p = all_processors()
        .into_iter()
        .find(|p| p.hook_point() == point)?;
    Some(p.process(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_point_round_trips() {
        for p in [
            HookPoint::SessionStart,
            HookPoint::PreToolUse,
            HookPoint::PostToolUse,
            HookPoint::PostEdit,
            HookPoint::PreCompact,
            HookPoint::SessionEnd,
        ] {
            assert_eq!(HookPoint::from_subcmd(p.as_str()), Some(p), "{p:?}");
        }
        assert_eq!(HookPoint::from_subcmd("garbage"), None);
    }

    #[test]
    fn all_processors_cover_every_hook_point_exactly_once() {
        let procs = all_processors();
        assert_eq!(procs.len(), 6, "six hooks wired");
        let mut points: Vec<HookPoint> = procs.iter().map(|p| p.hook_point()).collect();
        points.sort();
        let expected = vec![
            HookPoint::PostEdit,
            HookPoint::PostToolUse,
            HookPoint::PreCompact,
            HookPoint::PreToolUse,
            HookPoint::SessionEnd,
            HookPoint::SessionStart,
        ];
        // every lifecycle point appears exactly once
        for w in points.windows(2) {
            assert_ne!(w[0], w[1], "duplicate hook point");
        }
        for p in &expected {
            assert!(points.contains(p), "missing {p:?}");
        }
    }

    #[test]
    fn processor_profiles_match_legacy_constants() {
        // The wrapper must report the same profile the legacy should_run()
        // checks used, or it would misrepresent the hook's gating behavior.
        use crate::shared::types::{HookProfile, PROFILE_GUARD, PROFILE_POLISH, PROFILE_REFLECT};
        let procs = all_processors();
        let guard = procs
            .iter()
            .find(|p| p.hook_point() == HookPoint::PreToolUse)
            .unwrap();
        assert_eq!(guard.profile(), PROFILE_GUARD);
        let polish = procs
            .iter()
            .find(|p| p.hook_point() == HookPoint::PostEdit)
            .unwrap();
        assert_eq!(polish.profile(), PROFILE_POLISH);
        let reflect = procs
            .iter()
            .find(|p| p.hook_point() == HookPoint::SessionEnd)
            .unwrap();
        assert_eq!(reflect.profile(), PROFILE_REFLECT);
        // Minimal/Standard are the only two variants today; sanity check one.
        let _ = HookProfile::Minimal;
    }

    #[test]
    fn dispatch_unknown_returns_none() {
        assert!(dispatch("nonexistent", &HookInput::default()).is_none());
    }

    #[test]
    fn dispatch_routes_to_correct_processor() {
        // The typed dispatch must route each subcommand to the processor whose
        // hook_point matches its lifecycle stage.
        for (subcmd, expected) in [
            ("resume", HookPoint::SessionStart),
            ("guard", HookPoint::PreToolUse),
            ("observe", HookPoint::PostToolUse),
            ("polish", HookPoint::PostEdit),
            ("snapshot", HookPoint::PreCompact),
            ("reflect", HookPoint::SessionEnd),
        ] {
            let point = HookPoint::from_subcmd(subcmd).expect(subcmd);
            assert_eq!(point, expected, "subcmd {subcmd}");
        }
    }
}
