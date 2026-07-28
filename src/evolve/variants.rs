#![allow(dead_code)]

//! variants.rs — HarnessX variant isolation via ensemble routing (§4.5)
//!
//! Maintains up to K skill variants and routes each task to the variant with
//! the highest estimated success rate on that task's cluster. This is the
//! mechanism that prevents catastrophic forgetting on heterogeneous work
//! (paper §6.3): when an edit improves one cluster but regresses another, the
//! system forks a new variant rather than rejecting the edit.
//!
//! ## Critic-driven design
//! The paper validates variant isolation on GAIA (103 fixed task clusters,
//! stable pass@2). epic-harness has no equivalent — tasks are ad-hoc dev
//! requests. So:
//! - **Cold-start routing** is stack-based (file extensions in the task
//!   context), the only stable signal available without a benchmark.
//! - **Warm routing** upgrades to cluster prior success once enough history
//!   accumulates.
//! - **Fork-on-regression** is the core safety property and works regardless
//!   of cluster definition quality: an edit that helps one variant's tasks and
//!   hurts another never overwrites — it spawns a sibling.
//!
//! Per-variant seesaw (R5) is scoped: a candidate targeting variant k is
//! tested only against tasks routed to k.

use std::collections::{HashMap, HashSet};
use std::io;

use serde::{Deserialize, Serialize};

use crate::shared::evolution::SkillVariant;

/// Maximum number of concurrent variants. Architect red-flag B: keep this
/// separate from MAX_EVOLVED_SKILLS so variant forking does not starve the
/// base skill cap.
pub const MAX_VARIANTS: usize = 3;

/// Minimum samples before stack-based cold-start yields to cluster prior.
const WARM_ROUTING_MIN_SAMPLES: u32 = 4;
/// Session replay markers only need to cover the retained reflection window.
const MAX_VARIANT_SESSION_MARKERS: usize = 256;

/// A pool of skill variants plus per-variant routing stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariantPool {
    pub variants: Vec<SkillVariant>,
    /// variant_id → (successes, total) for routed tasks.
    pub routing_stats: HashMap<String, (u32, u32)>,
    /// Reflection session ids already included in `routing_stats`. This makes
    /// recovery after a worker crash idempotent instead of double-counting.
    #[serde(default)]
    pub applied_sessions: HashSet<String>,
}

impl VariantPool {
    /// Load the variant pool from disk, or an empty pool on missing/corrupt
    /// file. Variant routing is a hot path (called from dispatch); a corrupt
    /// `variants.json` must NOT panic — it resets to an empty pool so the
    /// session degrades gracefully rather than crashing every dispatch.
    pub fn load() -> VariantPool {
        VariantPool::load_for(None)
    }

    /// Project-scoped load: reads `variants.json` from the requested project's
    /// dir (None/empty = CWD project, the pre-existing behavior).
    pub fn load_for(project: Option<&str>) -> VariantPool {
        let path = crate::shared::paths::variant_pool_path_for(project);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => VariantPool::default(),
        }
    }

    /// Persist the pool atomically (tmp + rename). A reflect hook killed
    /// mid-save must not leave a truncated file that breaks every future
    /// `load()`/`route()` call.
    pub fn save(&self) -> io::Result<()> {
        let path = crate::shared::paths::variant_pool_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        crate::team::codex::atomic_replace_file(&tmp, &path)?;
        Ok(())
    }
}

impl VariantPool {
    /// Route a task context to the best variant.
    ///
    /// Warm path: if any variant has >= WARM_ROUTING_MIN_SAMPLES routed tasks,
    /// pick the highest observed success rate whose domain_tags match the
    /// task's detected stack. Cold path: fall back to stack-tag matching, then
    /// to the globally highest-scoring variant.
    pub fn route(&self, task_stack: &[&str]) -> Option<&SkillVariant> {
        if self.variants.is_empty() {
            return None;
        }

        // Warm: variants with enough samples, matching the stack, ranked by success.
        let warm: Vec<&SkillVariant> = self
            .variants
            .iter()
            .filter(|v| {
                let stats = self.routing_stats.get(&v.id).copied().unwrap_or((0, 0));
                stats.1 >= WARM_ROUTING_MIN_SAMPLES && stack_matches(v, task_stack)
            })
            .collect();
        if let Some(best) = warm
            .into_iter()
            .max_by(|a, b| success_rate(self, a).total_cmp(&success_rate(self, b)))
        {
            return Some(best);
        }

        // Cold: stack-tag match by avg_score.
        let stack_match: Vec<&SkillVariant> = self
            .variants
            .iter()
            .filter(|v| stack_matches(v, task_stack))
            .collect();
        if !stack_match.is_empty() {
            return stack_match
                .into_iter()
                .max_by(|a, b| a.avg_score.total_cmp(&b.avg_score));
        }

        // Fallback: highest avg_score overall.
        self.variants
            .iter()
            .max_by(|a, b| a.avg_score.total_cmp(&b.avg_score))
    }

    /// Fork-on-regression: given an edit improves variant `target` but would
    /// regress others, create a sibling variant that receives the edit instead.
    ///
    /// Returns the id of the variant that should receive the edit (existing
    /// `target` if no fork needed, or the new sibling id if forking). When the
    /// pool is full, the lowest-scoring variant is retired to make room, and
    /// the new sibling is inserted in its place — so pool size stays bounded.
    pub fn fork_if_needed(&mut self, target_id: &str, would_regress: bool) -> String {
        if !would_regress {
            return target_id.to_string();
        }
        if self.variants.len() >= MAX_VARIANTS {
            // Pool full: retire the lowest-scoring variant to make room.
            if let Some(idx) = self.lowest_scoring_index() {
                let retired = self.variants.remove(idx);
                self.routing_stats.remove(&retired.id);
            }
        }
        // Clone the target's shape (tags + skills) so the sibling starts where
        // the parent left off, then it diverges as the edit is applied.
        let parent = self.variants.iter().find(|v| v.id == target_id).cloned();
        let new_id = format!("{target_id}-fork-{}", self.variants.len() + 1);
        let sibling = match parent {
            Some(p) => SkillVariant {
                id: new_id.clone(),
                domain_tags: p.domain_tags.clone(),
                skills: p.skills.clone(),
                avg_score: p.avg_score,
                task_routing: p.task_routing.clone(),
            },
            None => SkillVariant {
                id: new_id.clone(),
                domain_tags: vec![],
                skills: vec![],
                avg_score: 0.0,
                task_routing: vec![],
            },
        };
        self.variants.push(sibling);
        new_id
    }

    /// Record a routing outcome for warm-routing stats.
    pub fn record_outcome(&mut self, variant_id: &str, success: bool) {
        let entry = self
            .routing_stats
            .entry(variant_id.to_string())
            .or_insert((0, 0));
        entry.1 += 1;
        if success {
            entry.0 += 1;
        }
    }

    /// Record a session's routing result at most once across process retries.
    pub fn record_outcome_once(
        &mut self,
        session_id: &str,
        variant_id: &str,
        success: bool,
    ) -> bool {
        if !self.applied_sessions.insert(session_id.to_string()) {
            return false;
        }
        while self.applied_sessions.len() > MAX_VARIANT_SESSION_MARKERS {
            let Some(oldest) = self.applied_sessions.iter().min().cloned() else {
                break;
            };
            self.applied_sessions.remove(&oldest);
        }
        self.record_outcome(variant_id, success);
        true
    }

    pub fn has_session_outcome(&self, session_id: &str) -> bool {
        self.applied_sessions.contains(session_id)
    }

    fn lowest_scoring_index(&self) -> Option<usize> {
        self.variants
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.avg_score.total_cmp(&b.avg_score))
            .map(|(i, _)| i)
    }
}

/// Does a variant's domain tags match any of the task's detected stacks?
fn stack_matches(variant: &SkillVariant, task_stack: &[&str]) -> bool {
    if variant.domain_tags.is_empty() {
        return false; // a tagless variant never matches on stack.
    }
    task_stack
        .iter()
        .any(|s| variant.domain_tags.iter().any(|t| t == s))
}

/// Observed success rate for a variant from routing stats, falling back to
/// its declared avg_score.
fn success_rate(pool: &VariantPool, v: &SkillVariant) -> f64 {
    match pool.routing_stats.get(&v.id).copied() {
        Some((succ, total)) if total > 0 => succ as f64 / total as f64,
        _ => v.avg_score,
    }
}

/// Detect stack tags from a task context string (heuristic on file extensions).
pub fn detect_stack(task_context: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let ctx = task_context.to_lowercase();
    if ctx.contains(".rs") || ctx.contains("cargo") {
        tags.push("rust".into());
    }
    if ctx.contains(".py") || ctx.contains("python") {
        tags.push("python".into());
    }
    if ctx.contains(".ts") || ctx.contains(".tsx") || ctx.contains("typescript") {
        tags.push("typescript".into());
    }
    if ctx.contains(".go") || ctx.contains("golang") {
        tags.push("go".into());
    }
    if ctx.contains(".java") || ctx.contains("kotlin") {
        tags.push("jvm".into());
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(id: &str, tags: &[&str], score: f64) -> SkillVariant {
        SkillVariant {
            id: id.into(),
            domain_tags: tags.iter().map(|t| (*t).to_string()).collect(),
            skills: vec![],
            avg_score: score,
            task_routing: vec![],
        }
    }

    #[test]
    fn empty_pool_routes_to_none() {
        let pool = VariantPool::default();
        assert!(pool.route(&["rust"]).is_none());
    }

    #[test]
    fn cold_start_matches_on_stack() {
        let pool = VariantPool {
            variants: vec![
                variant("rust-backend", &["rust"], 0.7),
                variant("python-ml", &["python"], 0.9),
            ],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        // No warm samples → cold path picks the rust variant for a rust task.
        let chosen = pool.route(&["rust"]).unwrap();
        assert_eq!(chosen.id, "rust-backend");
    }

    #[test]
    fn warm_routing_uses_observed_success() {
        let mut pool = VariantPool {
            variants: vec![
                variant("rust-a", &["rust"], 0.9), // declared high
                variant("rust-b", &["rust"], 0.5), // declared low
            ],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        // rust-b actually performs better in practice (warm samples).
        for _ in 0..5 {
            pool.record_outcome("rust-b", true);
        }
        for _ in 0..4 {
            pool.record_outcome("rust-a", false);
        }
        pool.record_outcome("rust-a", true);
        let chosen = pool.route(&["rust"]).unwrap();
        assert_eq!(chosen.id, "rust-b");
    }

    #[test]
    fn fallback_to_highest_score_when_no_stack_match() {
        let pool = VariantPool {
            variants: vec![
                variant("rust-backend", &["rust"], 0.7),
                variant("python-ml", &["python"], 0.9),
            ],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        // No stack signal → fall back to highest avg_score.
        let chosen = pool.route(&["unknown"]).unwrap();
        assert_eq!(chosen.id, "python-ml");
    }

    #[test]
    fn fork_creates_sibling_id_when_regression() {
        let mut pool = VariantPool {
            variants: vec![variant("rust-backend", &["rust"], 0.7)],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        let id = pool.fork_if_needed("rust-backend", true);
        assert!(id.starts_with("rust-backend-fork-"));
    }

    #[test]
    fn no_fork_when_no_regression() {
        let mut pool = VariantPool {
            variants: vec![variant("rust-backend", &["rust"], 0.7)],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        let id = pool.fork_if_needed("rust-backend", false);
        assert_eq!(id, "rust-backend");
    }

    #[test]
    fn fork_retires_lowest_when_pool_full() {
        let mut pool = VariantPool {
            variants: vec![
                variant("v1", &["rust"], 0.9),
                variant("v2", &["python"], 0.5), // lowest
                variant("v3", &["go"], 0.8),
            ],
            routing_stats: HashMap::new(),
            applied_sessions: HashSet::new(),
        };
        // Pool at MAX (3); forking must retire the lowest (v2) first.
        let id = pool.fork_if_needed("v1", true);
        assert_eq!(pool.variants.len(), 3);
        assert!(!pool.variants.iter().any(|v| v.id == "v2"));
        assert!(id.starts_with("v1-fork-"));
    }

    #[test]
    fn detect_stack_basic() {
        assert_eq!(
            detect_stack("fix src/auth/login.rs"),
            vec!["rust".to_string()]
        );
        assert_eq!(detect_stack("run train.py"), vec!["python".to_string()]);
        let mixed = detect_stack("port utils.ts to main.rs");
        assert!(mixed.contains(&"rust".to_string()));
        assert!(mixed.contains(&"typescript".to_string()));
        assert!(detect_stack("no code here").is_empty());
    }

    #[test]
    fn record_outcome_accumulates() {
        let mut pool = VariantPool::default();
        pool.record_outcome("v1", true);
        pool.record_outcome("v1", false);
        pool.record_outcome("v1", true);
        assert_eq!(pool.routing_stats.get("v1"), Some(&(2, 3)));
    }

    #[test]
    fn session_outcome_is_idempotent_across_retries() {
        let mut pool = VariantPool::default();
        assert!(pool.record_outcome_once("session-a", "variant-a", true));
        assert!(!pool.record_outcome_once("session-a", "variant-a", true));
        assert_eq!(pool.routing_stats.get("variant-a"), Some(&(1, 1)));
    }

    #[test]
    fn session_outcome_markers_keep_only_the_retained_window() {
        let mut pool = VariantPool::default();
        for index in 0..=MAX_VARIANT_SESSION_MARKERS {
            let session = format!("20260728-{index:04}");
            assert!(pool.record_outcome_once(&session, "variant-a", true));
        }

        assert_eq!(pool.applied_sessions.len(), MAX_VARIANT_SESSION_MARKERS);
        assert!(!pool.has_session_outcome("20260728-0000"));
        assert!(pool.has_session_outcome("20260728-0256"));
        assert_eq!(pool.routing_stats.get("variant-a"), Some(&(257, 257)));
    }
}
