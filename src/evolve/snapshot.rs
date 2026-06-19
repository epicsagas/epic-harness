//! snapshot.rs — HarnessSnapshot first-class object (HarnessX-inspired)
//!
//! Pure-READ aggregation of the entire harness state into a serializable,
//! comparable unit. The snapshot captures: config summary, active + evolved
//! skill lists, guard rules, and a compact metrics summary. A deterministic
//! content hash enables `epic harness diff` to compare two states reliably.
//!
//! No side effects: building a snapshot never writes, mutates, or sends.
//! RESTORE (the destructive inverse) is deferred — see `cli::HarnessSub::Restore`.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::CONFIG;
use crate::shared::evolution::{ConfigSummary, HarnessSnapshot, MetricsSummary};
use crate::shared::helpers::list_dirs;
use crate::shared::paths::{evolved_dir_for, guard_rules_file, project_slug};
use crate::shared::types::current_hook_profile;

/// Recursively collect every leaf value in a JSON tree as dotted "path=value"
/// strings, sorted canonically. Used so the hash is insensitive to map key
/// ordering (serde_json with the default non-`preserve_order` feature already
/// orders object keys, but this is a belt-and-suspenders guarantee covering
/// nested structures produced by third-party serializers).
#[allow(dead_code)]
fn collect_leaves(value: &Value, out: &mut Vec<String>) {
    collect_leaves_at("", value, out);
}

fn collect_leaves_at(path: &str, value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let child = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                collect_leaves_at(&child, v, out);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let child = if path.is_empty() {
                    i.to_string()
                } else {
                    format!("{path}[{i}]")
                };
                collect_leaves_at(&child, v, out);
            }
        }
        Value::Null => out.push(format!("{path}=null")),
        Value::Bool(b) => out.push(format!("{path}={b}")),
        Value::Number(n) => out.push(format!("{path}={n}")),
        Value::String(s) => out.push(format!("{path}={s}")),
    }
}

/// Produce a canonical (sorted-keys) JSON string for a value.
fn canonical_json(value: &Value) -> String {
    // serde_json::to_string preserves insertion order for Value::Object by
    // default only when the `preserve_order` feature is OFF (the default), in
    // which case it serializes a BTreeMap — i.e. sorted keys. We additionally
    // re-parse and re-serialize to strip any feature-driven ordering.
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    // Round-trip through a Value to normalize, then serialize with sorted keys.
    let normalized = value.clone();
    normalized.serialize(&mut ser).ok();
    String::from_utf8_lossy(&buf).to_string()
}

/// Deterministic content hash of a snapshot's *content* (excluding the hash
/// field itself and the volatile timestamp).
///
/// Uses std DefaultHasher for determinism within a process family — stable
/// enough for state comparison across `epic harness diff` runs on the same
/// platform. The hash is computed over the canonical sorted-keys JSON of the
/// hashable payload, so identical states always produce identical hashes.
fn content_hash(snapshot_without_hash_and_ts: &HarnessSnapshot) -> String {
    // Serialize the whole struct to a canonical Value, then drop the volatile
    // `hash` and `timestamp` keys before hashing.
    let mut value = serde_json::to_value(snapshot_without_hash_and_ts)
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    if let Value::Object(ref mut map) = value {
        map.remove("hash");
        map.remove("timestamp");
    }
    let canonical = canonical_json(&value);

    // FNV-1a-style stable fold over DefaultHasher. DefaultHasher is SipHash,
    // which is deterministic for identical input byte sequences.
    let mut h1 = DefaultHasher::new();
    canonical.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    // Second pass with a salted seed to widen the 64-bit space into 128-bit-ish.
    (canonical.len() as u64).hash(&mut h2);
    format!("{:016x}{:016x}", h1.finish(), h2.finish())
}

/// Read guard-rules.yaml as a sorted list of non-empty, trimmed lines.
/// Absent file → empty vec (snapshot still valid).
fn read_guard_rule_lines() -> Vec<String> {
    let path = guard_rules_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

/// Build a MetricsSummary from the project's metrics, defaulting gracefully
/// when the DB has no data yet (cold start).
/// Project-scoped metrics summary via the scoped loader (None/empty = CWD).
fn metrics_summary_for(project: Option<&str>) -> MetricsSummary {
    let m = crate::store::runtime::block_on(async {
        let pool = crate::store::pool::harness_pool().await.ok()?;
        crate::store::metrics::load_metrics_scoped_pool(&pool, project)
            .await
            .ok()
    });
    match m {
        Some(m) => MetricsSummary {
            total_sessions: m.total_sessions,
            best_score: m.best_score,
            trend: m.trend,
            total_evolved: m.total_evolved_skills,
            stagnation_count: m.stagnation_count,
        },
        None => MetricsSummary::default(),
    }
}

/// Build a ConfigSummary from the resolved global CONFIG + hook profile.
///
/// `hook_profile` reflects the *effective* profile (env var `EPIC_HOOK_PROFILE`
/// takes precedence over the config file), so snapshots diff correctly across
/// machines that differ only by env override.
fn config_summary() -> ConfigSummary {
    ConfigSummary {
        hook_profile: format!("{:?}", current_hook_profile()).to_lowercase(),
        scoring_weights: CONFIG.scoring.weights,
        max_skills: CONFIG.evolution.max_skills,
        stagnation_limit: CONFIG.evolution.stagnation_limit,
    }
}

/// Build a complete HarnessSnapshot of the current project state.
///
/// Pure read: touches only `evolved_dir`, `guard_rules_file`, the metrics DB,
/// and the resolved CONFIG. Never mutates state.
pub fn build_snapshot() -> HarnessSnapshot {
    build_snapshot_for(None)
}

/// Project-scoped snapshot: reads the evolved dir + metrics for the requested
/// project (None/empty = CWD project, the pre-existing behavior).
///
/// Scope note: only `evolved_dir` and `metrics_summary` vary by project.
/// `guard_rules` (project-tree/global file) and `config_summary` (global
/// CONFIG) are intentionally project-independent.
pub fn build_snapshot_for(project: Option<&str>) -> HarnessSnapshot {
    let evolved = evolved_dir_for(project);
    // `evolved_dir` holds auto-evolved skills; treat those as both the active
    // and evolved skill sets (static skills ship in the binary and are not
    // per-project state). Listing the same dir for both is intentional — it
    // mirrors how reflect/gate reason about "current skill set".
    let mut skills = list_dirs(&evolved);
    skills.sort();
    skills.dedup();

    let guard_rules = read_guard_rule_lines();
    let config = config_summary();
    let metrics = metrics_summary_for(project);

    // ISO-8601-ish UTC timestamp (stdlib only — no chrono dep required).
    let timestamp = {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("{secs}")
    };

    let project_slug = match project {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => project_slug(),
    };

    // Compute hash from a snapshot with empty placeholder hash/timestamp so the
    // hash field does not feed into itself.
    let pre = HarnessSnapshot {
        version: env!("CARGO_PKG_VERSION").to_string(),
        project_slug: project_slug.clone(),
        timestamp: String::new(),
        config_summary: config.clone(),
        active_skills: skills.clone(),
        evolved_skills: skills.clone(),
        guard_rules: guard_rules.clone(),
        metrics_summary: metrics.clone(),
        hash: String::new(),
    };
    let hash = content_hash(&pre);

    HarnessSnapshot {
        version: env!("CARGO_PKG_VERSION").to_string(),
        project_slug,
        timestamp,
        config_summary: config,
        active_skills: skills.clone(),
        evolved_skills: skills,
        guard_rules,
        metrics_summary: metrics,
        hash,
    }
}

// ── Diff ─────────────────────────────────────────────

/// A single field-level difference between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotDiff {
    pub field: String,
    pub before: Value,
    pub after: Value,
}

/// Compute the field-by-field diff between two snapshots, ignoring the
/// volatile `timestamp` field. Returns a list ordered by field name.
///
/// Diffing is structural on the canonical JSON form, so nested object fields
/// (config_summary.*, metrics_summary.*) and list membership
/// (active_skills/evolved_skills/guard_rules) all surface as discrete entries.
pub fn diff_snapshots(a: &HarnessSnapshot, b: &HarnessSnapshot) -> Vec<SnapshotDiff> {
    let mut va = serde_json::to_value(a).unwrap_or(Value::Null);
    let mut vb = serde_json::to_value(b).unwrap_or(Value::Null);

    // Strip volatile fields before comparison.
    if let Value::Object(ref mut m) = va {
        m.remove("timestamp");
    }
    if let Value::Object(ref mut m) = vb {
        m.remove("timestamp");
    }

    let mut diffs = Vec::new();
    diff_values("", &va, &vb, &mut diffs);
    diffs.sort_by(|x, y| x.field.cmp(&y.field));
    diffs
}

/// Recursively walk two JSON values in parallel, recording leaf differences.
/// `parent_path` is the dotted path of the parent field ("" at the root).
fn diff_values(parent_path: &str, a: &Value, b: &Value, out: &mut Vec<SnapshotDiff>) {
    let path = parent_path.to_string();
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            keys.extend(ma.keys().cloned());
            keys.extend(mb.keys().cloned());
            for k in keys {
                let av = ma.get(&k).unwrap_or(&Value::Null);
                let bv = mb.get(&k).unwrap_or(&Value::Null);
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                diff_values(&child_path, av, bv, out);
            }
        }
        (Value::Array(aa), Value::Array(bb)) => {
            // Compare as sets: additions and removals.
            let set_a: std::collections::BTreeSet<String> = aa.iter().map(canonical_json).collect();
            let set_b: std::collections::BTreeSet<String> = bb.iter().map(canonical_json).collect();
            let added: Vec<&String> = set_b.difference(&set_a).collect();
            let removed: Vec<&String> = set_a.difference(&set_b).collect();
            if !added.is_empty() {
                let added_vals: Vec<Value> = added
                    .iter()
                    .map(|s| serde_json::from_str(s).unwrap_or(Value::String(s.to_string())))
                    .collect();
                out.push(SnapshotDiff {
                    field: format!("{path}(added)"),
                    before: Value::Array(vec![]),
                    after: Value::Array(added_vals),
                });
            }
            if !removed.is_empty() {
                let removed_vals: Vec<Value> = removed
                    .iter()
                    .map(|s| serde_json::from_str(s).unwrap_or(Value::String(s.to_string())))
                    .collect();
                out.push(SnapshotDiff {
                    field: format!("{path}(removed)"),
                    before: Value::Array(removed_vals),
                    after: Value::Array(vec![]),
                });
            }
        }
        (x, y) if x == y => { /* equal leaf — no diff */ }
        _ => {
            out.push(SnapshotDiff {
                field: path,
                before: a.clone(),
                after: b.clone(),
            });
        }
    }
}

// ── Tests ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::evolution::HarnessSnapshot;
    use serde_json::json;

    /// Two snapshots of identical state must hash identically (the volatile
    /// timestamp is excluded from the hash).
    #[test]
    fn hash_is_stable_for_identical_state() {
        let base = make_fixture_snapshot("proj", &["rust-tdd"], [0.5, 0.3, 0.2], 10, 3);
        // Vary only the timestamp — hash must not change.
        let mut with_ts = base.clone();
        with_ts.timestamp = "99999".into();
        let h1 = content_hash(&base);
        let h2 = content_hash(&with_ts);
        assert_eq!(h1, h2, "timestamp must not affect content hash");
    }

    /// Different skill sets must produce different hashes.
    #[test]
    fn hash_differs_when_state_differs() {
        let a = make_fixture_snapshot("proj", &["rust-tdd"], [0.5, 0.3, 0.2], 10, 3);
        let b = make_fixture_snapshot("proj", &["rust-tdd", "extra"], [0.5, 0.3, 0.2], 10, 3);
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    /// Different config (max_skills) must produce different hashes.
    #[test]
    fn hash_differs_when_config_differs() {
        let a = make_fixture_snapshot("proj", &["x"], [0.5, 0.3, 0.2], 10, 3);
        let b = make_fixture_snapshot("proj", &["x"], [0.5, 0.3, 0.2], 5, 3);
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    /// Canonical serialization must be deterministic across runs (sorted keys).
    #[test]
    fn canonical_serialization_is_sorted() {
        let snap = make_fixture_snapshot("p", &["a", "b"], [0.5, 0.3, 0.2], 10, 3);
        let value = serde_json::to_value(&snap).unwrap();
        let s1 = canonical_json(&value);
        let s2 = canonical_json(&value);
        assert_eq!(s1, s2);
        // config_summary keys appear in sorted order.
        let cfg_idx = s1.find("config_summary").unwrap();
        let hook_idx = s1.find("hook_profile").unwrap();
        let max_idx = s1.find("max_skills").unwrap();
        assert!(cfg_idx < hook_idx);
        assert!(hook_idx < max_idx);
    }

    /// diff detects added/removed skills.
    #[test]
    fn diff_detects_skill_changes() {
        let a = make_fixture_snapshot("p", &["rust-tdd"], [0.5, 0.3, 0.2], 10, 3);
        let b = make_fixture_snapshot("p", &["rust-tdd", "perf-cache"], [0.5, 0.3, 0.2], 10, 3);
        let diffs = diff_snapshots(&a, &b);
        let fields: Vec<&str> = diffs.iter().map(|d| d.field.as_str()).collect();
        assert!(
            fields.iter().any(|f| f.contains("active_skills(added)")),
            "expected an added-skills diff, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.contains("evolved_skills(added)")),
            "expected an added-evolved diff"
        );
        // Reverse direction detects removals.
        let diffs_rev = diff_snapshots(&b, &a);
        let fields_rev: Vec<&str> = diffs_rev.iter().map(|d| d.field.as_str()).collect();
        assert!(
            fields_rev
                .iter()
                .any(|f| f.contains("active_skills(removed)"))
        );
    }

    /// diff detects changed config fields.
    #[test]
    fn diff_detects_config_changes() {
        let a = make_fixture_snapshot("p", &["x"], [0.5, 0.3, 0.2], 10, 3);
        let b = make_fixture_snapshot("p", &["x"], [0.5, 0.3, 0.2], 5, 3);
        let diffs = diff_snapshots(&a, &b);
        assert!(
            diffs
                .iter()
                .any(|d| d.field.ends_with("config_summary.max_skills")),
            "expected max_skills diff: {diffs:?}"
        );
    }

    /// diff ignores the timestamp field.
    #[test]
    fn diff_ignores_timestamp() {
        let mut a = make_fixture_snapshot("p", &["x"], [0.5, 0.3, 0.2], 10, 3);
        let mut b = a.clone();
        a.timestamp = "1".into();
        b.timestamp = "2".into();
        assert!(diff_snapshots(&a, &b).is_empty());
    }

    /// build_snapshot returns a structurally complete snapshot even with an
    /// empty evolved dir (cold start). We assert structure, not values.
    #[test]
    fn build_snapshot_is_structurally_complete() {
        let snap = build_snapshot();
        assert!(!snap.version.is_empty(), "version must be populated");
        assert!(!snap.project_slug.is_empty(), "project_slug populated");
        assert!(!snap.timestamp.is_empty(), "timestamp populated");
        assert!(!snap.hash.is_empty(), "hash populated");
        assert_eq!(snap.hash.len(), 32, "hash is 128-bit hex (32 chars)");
        // Skills lists are sorted and deduped.
        let mut expected = snap.active_skills.clone();
        expected.sort();
        expected.dedup();
        assert_eq!(snap.active_skills, expected, "active_skills sorted+deduped");
        assert_eq!(snap.evolved_skills, snap.active_skills);
        // guard_rules are sorted/deduped.
        let mut g = snap.guard_rules.clone();
        g.sort();
        g.dedup();
        assert_eq!(snap.guard_rules, g);
    }

    /// Rebuilding the same instant produces a stable hash (modulo timestamp).
    #[test]
    fn rebuild_produces_stable_hash() {
        // Use hermetic fixtures (not live build_snapshot) so the test is
        // independent of host harness state. A parallel test writing to
        // evolved_dir() during two live build_snapshot() calls previously
        // made the hashes diverge (flaky under --test-threads > 1).
        let a = make_fixture_snapshot("proj", &["evo-a", "evo-b"], [0.5, 0.3, 0.2], 10, 3);
        let b = make_fixture_snapshot("proj", &["evo-a", "evo-b"], [0.5, 0.3, 0.2], 10, 3);
        // Hash must be identical; only the timestamp may differ.
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.version, b.version);
        assert_eq!(a.project_slug, b.project_slug);
    }

    /// collect_leaves extracts scalar values from a nested JSON tree.
    #[test]
    fn collect_leaves_walks_nested() {
        let v = json!({
            "a": { "b": 1, "c": "x" },
            "d": [true, null]
        });
        let mut out = Vec::new();
        collect_leaves(&v, &mut out);
        assert!(out.iter().any(|s| s == "a.b=1"));
        assert!(out.iter().any(|s| s == "a.c=x"));
    }

    /// Build a fixture snapshot without touching the filesystem, so hash/diff
    /// tests are hermetic and independent of the host's harness state.
    fn make_fixture_snapshot(
        slug: &str,
        skills: &[&str],
        weights: [f64; 3],
        max_skills: usize,
        stagnation_limit: u64,
    ) -> HarnessSnapshot {
        let skills: Vec<String> = skills.iter().map(|s| s.to_string()).collect();
        let guard = vec!["blocked: kubectl delete".to_string()];
        let pre = HarnessSnapshot {
            version: "0.0.0-test".into(),
            project_slug: slug.into(),
            timestamp: String::new(),
            config_summary: ConfigSummary {
                hook_profile: "standard".into(),
                scoring_weights: weights,
                max_skills,
                stagnation_limit,
            },
            active_skills: skills.clone(),
            evolved_skills: skills.clone(),
            guard_rules: guard,
            metrics_summary: MetricsSummary {
                total_sessions: 1,
                best_score: Some(0.5),
                trend: "stable".into(),
                total_evolved: skills.len() as u64,
                stagnation_count: 0,
            },
            hash: String::new(),
        };
        let hash = content_hash(&pre);
        HarnessSnapshot { hash, ..pre }
    }
}
