//! harness_cli.rs — `epic harness` subcommand group (HarnessX first-class object)
//!
//! Read-only surface over the harness state:
//!   epic harness snapshot           → print current HarnessSnapshot as JSON
//!   epic harness diff <a> <b>       → field-by-field diff of two snapshot JSON files
//!   epic harness restore <file>     → NOT IMPLEMENTED (destructive); deferred
//!
//! Building/diffing snapshots never mutates state. RESTORE would overwrite
//! evolved skills, guard rules, and metrics — that destructive inverse is
//! intentionally deferred until a confirmation/safety contract exists.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::evolve::snapshot::{SnapshotDiff, build_snapshot, diff_snapshots};
use crate::shared::evolution::HarnessSnapshot;

/// Entry point: `args[0]` is `"harness"`, `args[1..]` is the subcommand + flags.
pub fn run(args: &[String]) -> i32 {
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    match sub {
        "snapshot" => run_snapshot(&args[2..]),
        "diff" => run_diff(&args[2..]),
        "restore" => {
            eprintln!(
                "not implemented (destructive); deferred — restore would overwrite \
                 evolved skills, guard rules, and metrics. Use `epic harness snapshot` \
                 and `epic harness diff` for read-only inspection."
            );
            1
        }
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        other => {
            eprintln!("error: unknown harness subcommand '{other}'\n");
            print_help();
            1
        }
    }
}

fn run_snapshot(_flags: &[String]) -> i32 {
    let snap = build_snapshot();
    match serde_json::to_string_pretty(&snap) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(e) => {
            eprintln!("error: failed to serialize snapshot: {e}");
            1
        }
    }
}

fn run_diff(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("Usage: epic harness diff <before.json> <after.json>");
        return 1;
    }
    let a_path = Path::new(&args[0]);
    let b_path = Path::new(&args[1]);
    let (a, b) = match (load_snapshot(a_path), load_snapshot(b_path)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) => {
            eprintln!("error: failed to load {}: {e}", a_path.display());
            return 1;
        }
        (_, Err(e)) => {
            eprintln!("error: failed to load {}: {e}", b_path.display());
            return 1;
        }
    };
    let diffs = diff_snapshots(&a, &b);
    print_diff(&a, &b, &diffs);
    0
}

fn load_snapshot(path: &Path) -> Result<HarnessSnapshot, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str::<HarnessSnapshot>(&content)
        .map_err(|e| format!("invalid snapshot JSON at {}: {e}", path.display()))
}

fn print_diff(a: &HarnessSnapshot, b: &HarnessSnapshot, diffs: &[SnapshotDiff]) {
    let summary = format_diff_summary(diffs);
    if diffs.is_empty() {
        println!("no differences (hashes equal: a={} b={})", a.hash, b.hash);
        return;
    }
    println!(
        "snapshots differ — {} ({} fields changed)",
        if a.hash == b.hash {
            "hashes unexpectedly equal".to_string()
        } else {
            format!("hash a={} b={}", a.hash, b.hash)
        },
        diffs.len()
    );
    println!("{summary}");
    println!("\nField-by-field:");
    for d in diffs {
        let before = compact_json(&d.before);
        let after = compact_json(&d.after);
        // Truncate long values for terminal readability.
        let before = truncate(&before, 80);
        let after = truncate(&after, 80);
        println!("  {field}", field = d.field);
        println!("    - {before}");
        println!("    + {after}");
    }
}

fn format_diff_summary(diffs: &[SnapshotDiff]) -> String {
    let mut added_skills = 0;
    let mut removed_skills = 0;
    let mut config_changes = 0;
    let mut metrics_changes = 0;
    let mut guard_changes = 0;
    for d in diffs {
        let f = &d.field;
        if f.contains("active_skills(added)") || f.contains("evolved_skills(added)") {
            added_skills += 1;
        } else if f.contains("active_skills(removed)") || f.contains("evolved_skills(removed)") {
            removed_skills += 1;
        } else if f.starts_with("config_summary") {
            config_changes += 1;
        } else if f.starts_with("metrics_summary") {
            metrics_changes += 1;
        } else if f.starts_with("guard_rules") {
            guard_changes += 1;
        }
    }
    let mut parts = Vec::new();
    if added_skills > 0 {
        parts.push(format!("{added_skills} skill(s) added"));
    }
    if removed_skills > 0 {
        parts.push(format!("{removed_skills} skill(s) removed"));
    }
    if config_changes > 0 {
        parts.push(format!("{config_changes} config field(s) changed"));
    }
    if metrics_changes > 0 {
        parts.push(format!("{metrics_changes} metrics field(s) changed"));
    }
    if guard_changes > 0 {
        parts.push(format!("{guard_changes} guard rule change(s)"));
    }
    if parts.is_empty() {
        "no categorized changes".into()
    } else {
        format!("summary: {}", parts.join(", "))
    }
}

fn compact_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| format!("{v:?}"))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    // Cut at a character boundary to stay UTF-8 safe for arbitrary JSON values.
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}

fn print_help() {
    eprintln!("epic harness — harness state as a first-class object\n");
    eprintln!("USAGE:");
    eprintln!("  epic harness snapshot");
    eprintln!(
        "    Print the current harness state as JSON (config, skills, guard rules, metrics)."
    );
    eprintln!("    Pure read — no side effects.\n");
    eprintln!("  epic harness diff <before.json> <after.json>");
    eprintln!("    Compare two snapshot JSON files field-by-field.\n");
    eprintln!("  epic harness restore <file.json>");
    eprintln!("    (not implemented — destructive; deferred)\n");
    eprintln!("EXAMPLE:");
    eprintln!("  epic harness snapshot > before.json");
    eprintln!("  # ...make changes...");
    eprintln!("  epic harness snapshot > after.json");
    eprintln!("  epic harness diff before.json after.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_subcommand_returns_error() {
        let code = run(&["harness".to_string(), "bogus".to_string()]);
        assert_eq!(code, 1);
    }

    #[test]
    fn restore_is_deferred() {
        let code = run(&[
            "harness".to_string(),
            "restore".to_string(),
            "x.json".to_string(),
        ]);
        assert_eq!(code, 1);
    }

    #[test]
    fn diff_missing_args_returns_usage_error() {
        let code = run_diff(&[]);
        assert_eq!(code, 1);
        let code = run_diff(&["only-one.json".to_string()]);
        assert_eq!(code, 1);
    }

    #[test]
    fn truncate_short_value_unchanged() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn truncate_long_value_ellipsis() {
        let out = truncate("abcdefghij", 5);
        assert!(out.ends_with('…'));
        // 5 ASCII chars + 1 ellipsis char (… is multi-byte, so count chars not bytes).
        assert_eq!(out.chars().count(), 6);
    }

    #[test]
    fn compact_json_arrays() {
        let v = serde_json::json!([1, 2, 3]);
        assert_eq!(compact_json(&v), "[1,2,3]");
    }

    #[test]
    fn diff_summary_categorizes_skill_changes() {
        use crate::shared::evolution::{ConfigSummary, MetricsSummary};
        let mk = |skills: &[&str]| HarnessSnapshot {
            version: "0".into(),
            project_slug: "p".into(),
            timestamp: String::new(),
            config_summary: ConfigSummary {
                hook_profile: "standard".into(),
                scoring_weights: [0.5, 0.3, 0.2],
                max_skills: 10,
                stagnation_limit: 3,
            },
            active_skills: skills.iter().map(|s| s.to_string()).collect(),
            evolved_skills: skills.iter().map(|s| s.to_string()).collect(),
            guard_rules: vec![],
            metrics_summary: MetricsSummary::default(),
            hash: "x".into(),
        };
        let a = mk(&["rust-tdd"]);
        let b = mk(&["rust-tdd", "perf"]);
        let diffs = diff_snapshots(&a, &b);
        let summary = format_diff_summary(&diffs);
        assert!(summary.contains("skill(s) added"), "summary was: {summary}");
    }
}
