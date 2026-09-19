//! PR merge outcome ledger (#129 P0).
//!
//! Orbit measures success at PR creation; this ledger tracks what actually
//! happened to the PR afterwards. Append-only JSONL in
//! `$HARNESS_DIR/projects/{slug}/orbit/outcomes.jsonl` — one record per state
//! transition, latest record per `pr_url` wins. Terminal states
//! (merged/closed/stale) are never re-queried unless `--all`, so gh API cost
//! stays bounded. Network access lives only in these CLI subcommands; hooks
//! stay offline (same precedent as the removed synchronous `claude -p`).

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::helpers::{append_jsonl, now_iso, read_jsonl_typed};
use crate::shared::paths::orbit_dir;

const EXIT_OK: i32 = 0;
const EXIT_GH: i32 = 7;
const EXIT_USAGE: i32 = 64;

pub const DEFAULT_STALE_DAYS: i64 = 14;

/// One ledger record. A PR's current state is the last record with its
/// `pr_url`; earlier records are history.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Outcome {
    pub pr_url: String,
    pub repo: String,
    pub number: u64,
    /// "orbit" (recovered from PIPELINE-*.json) or "manual" (record-pr).
    pub source: String,
    #[serde(default)]
    pub pipeline_id: Option<String>,
    #[serde(default)]
    pub goal_slug: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    pub recorded_at: String,
    /// pending | merged | closed | stale
    pub state: String,
    #[serde(default)]
    pub state_changed_at: Option<String>,
    #[serde(default)]
    pub checks: u32,
}

/// GitHub-side facts about a PR, as far as the ledger cares.
struct GhPr {
    /// gh's uppercase state: OPEN | MERGED | CLOSED | REOPENED(?)
    state: String,
    created_at: Option<String>,
    checks: u32,
}

// ── pure functions (unit-tested; no filesystem, no gh) ────────────────

/// `https://github.com/{owner}/{repo}/pull/{number}` → (repo, number).
/// Other hosts and malformed paths are rejected: gh can only ever query
/// github.com, so anything else would silently never reconcile.
pub fn parse_pr_url(url: &str) -> Option<(String, u64)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (repo, num) = rest.rsplit_once("/pull/")?;
    if repo.is_empty() || repo.split('/').count() != 2 {
        return None;
    }
    num.parse::<u64>().ok().map(|n| (repo.to_string(), n))
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.fff]Z` to epoch seconds. Mirrors `now_iso()`
/// output; tolerates the fractional variant gh sometimes emits.
pub fn iso_to_epoch(s: &str) -> Option<i64> {
    let s = s.trim().strip_suffix('Z').unwrap_or(s.trim());
    let s = s.split('.').next()?; // drop fractional seconds
    let (date, time) = s.split_once('T')?;
    let (y, mo, d) = scan_ints(date, '-')?;
    let (h, mi, sec) = scan_ints(time, ':')?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + sec)
}

fn scan_ints(s: &str, sep: char) -> Option<(i64, i64, i64)> {
    let mut it = s.split(sep);
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    Some((a, b, c))
}

/// Howard Hinnant's days_from_civil — proleptic Gregorian, stdlib-only
/// (this crate has no chrono/time dependency).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn is_terminal(state: &str) -> bool {
    state == "merged" || state == "closed" || state == "stale"
}

/// Next ledger state for a PR, or `None` when nothing changed.
///
/// Terminal states only move on a genuine gh MERGED/CLOSED report (used by
/// `--all` to correct a wrong ledger entry); stale applies only to PRs that
/// are still pending. Whether terminal PRs get queried at all is the
/// caller's `--all` gating.
pub fn reconcile_decision(
    current: &str,
    gh_state: &str,
    age_days: i64,
    stale_days: i64,
) -> Option<String> {
    let set = |s: &str| (current != s).then(|| s.to_string());
    match gh_state {
        "MERGED" => set("merged"),
        "CLOSED" => set("closed"),
        _ if current == "pending" || current.is_empty() => {
            if age_days >= stale_days {
                set("stale")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Latest record per `pr_url` (last occurrence wins — the file is
/// append-only, so later records supersede earlier ones).
pub fn latest_by_pr(records: Vec<Outcome>) -> BTreeMap<String, Outcome> {
    let mut map = BTreeMap::new();
    for r in records {
        map.insert(r.pr_url.clone(), r);
    }
    map
}

#[derive(Default, PartialEq, Debug)]
pub struct Aggregate {
    pub merged: u32,
    pub closed: u32,
    pub stale: u32,
    pub pending: u32,
}

impl Aggregate {
    pub fn terminal(&self) -> u32 {
        self.merged + self.closed + self.stale
    }

    /// merged / terminal; `None` when no PR has reached a terminal state
    /// (pending never enters the denominator).
    pub fn merge_rate(&self) -> Option<f64> {
        let t = self.terminal();
        (t > 0).then(|| self.merged as f64 / t as f64)
    }
}

pub fn aggregate(latest: &BTreeMap<String, Outcome>) -> Aggregate {
    let mut agg = Aggregate::default();
    for r in latest.values() {
        match r.state.as_str() {
            "merged" => agg.merged += 1,
            "closed" => agg.closed += 1,
            "stale" => agg.stale += 1,
            _ => agg.pending += 1,
        }
    }
    agg
}

/// Format the status line: `merge_rate: 71% (5/7 terminal, 3 pending)`.
pub fn status_line(agg: &Aggregate) -> String {
    let rate = match agg.merge_rate() {
        Some(r) => format!("{}%", (r * 100.0).round() as i64),
        None => "n/a".to_string(),
    };
    format!(
        "merge_rate: {rate} ({}/{} terminal, {} pending)",
        agg.merged,
        agg.terminal(),
        agg.pending
    )
}

/// Baseline ledger entry for a PR discovered in pipeline state — appended
/// as `pending` at first sight; reconcile then queries gh for reality.
pub fn outcome_from_pipeline(
    pr_url: &str,
    pipeline_id: Option<String>,
    goal_slug: Option<String>,
    branch: Option<String>,
) -> Option<Outcome> {
    let (repo, number) = parse_pr_url(pr_url)?;
    Some(Outcome {
        pr_url: pr_url.to_string(),
        repo,
        number,
        source: "orbit".to_string(),
        pipeline_id,
        goal_slug,
        branch,
        recorded_at: now_iso(),
        state: "pending".to_string(),
        state_changed_at: None,
        checks: 0,
    })
}

/// Recover PR URLs from one PIPELINE-*.json. Both shapes exist in the wild:
/// top-level `pr_url` and `phase_history[].pr` (plain URL string).
pub fn pipeline_pr_urls(pipeline: &Value) -> Vec<(String, Option<String>, Option<String>)> {
    let mut out: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    let mut push = |url: &str| {
        if parse_pr_url(url).is_some() && !out.iter().any(|(u, _, _)| u == url) {
            out.push((url.to_string(), None, None));
        }
    };
    if let Some(u) = pipeline.get("pr_url").and_then(|v| v.as_str()) {
        push(u);
    }
    if let Some(history) = pipeline.get("phase_history").and_then(|v| v.as_array()) {
        for h in history {
            if let Some(u) = h.get("pr").and_then(|v| v.as_str()) {
                push(u);
            }
        }
    }
    // Attach goal_slug/branch from the pipeline to whatever we found.
    let goal = pipeline
        .get("goal_slug")
        .and_then(|v| v.as_str())
        .map(String::from);
    let branch = pipeline
        .get("branch")
        .and_then(|v| v.as_str())
        .map(String::from);
    out.into_iter()
        .map(|(u, _, _)| (u, goal.clone(), branch.clone()))
        .collect()
}

// ── gh wrapper (thin, untested — boundary is filesystem + network) ────

fn gh_pr_view(url: &str) -> Result<GhPr, String> {
    let out = Command::new("gh")
        .args([
            "pr",
            "view",
            url,
            "--json",
            "state,createdAt,statusCheckRollup",
        ])
        .output()
        .map_err(|e| format!("gh not runnable: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.lines().next().unwrap_or("gh pr view failed");
        return Err(err.to_string());
    }
    let v: Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("gh output not JSON: {e}"))?;
    Ok(GhPr {
        state: v
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("UNKNOWN")
            .to_string(),
        created_at: v
            .get("createdAt")
            .and_then(|s| s.as_str())
            .map(String::from),
        checks: v
            .get("statusCheckRollup")
            .and_then(|c| c.as_array())
            .map(|a| a.len() as u32)
            .unwrap_or(0),
    })
}

// ── subcommands ────────────────────────────────────────────────────────

pub fn run_reconcile(args: &[String]) -> i32 {
    let force = args.iter().any(|a| a == "--all");
    let stale_days = flag_value(args, "--stale-days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_STALE_DAYS);

    let ledger_path = orbit_dir().join("outcomes.jsonl");
    let ledger = read_jsonl_typed::<Outcome>(&ledger_path);
    let mut latest = latest_by_pr(ledger);

    // Candidates = pipeline-discovered PRs + every PR already in the ledger.
    let mut candidates: BTreeMap<String, Outcome> = latest.clone();
    for path in pipeline_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let pid = path.file_name().map(|n| {
            n.to_string_lossy()
                .trim_start_matches("PIPELINE-")
                .trim_end_matches(".json")
                .to_string()
        });
        for (url, goal, branch) in pipeline_pr_urls(&v) {
            if !candidates.contains_key(&url) {
                candidates.insert(
                    url.clone(),
                    outcome_from_pipeline(&url, pid.clone(), goal, branch)
                        .expect("parse_pr_url already vetted this URL"),
                );
            }
        }
    }

    // Query phase: collect transitions before any write so a gh failure
    // leaves the ledger untouched.
    let now = iso_to_epoch(&now_iso()).unwrap_or(0);
    let mut transitions: Vec<Outcome> = Vec::new();
    for (url, baseline) in &candidates {
        let current = latest.get(url).map(|o| o.state.as_str()).unwrap_or("");
        if !force && is_terminal(current) {
            continue;
        }
        let gh = match gh_pr_view(url) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("reconcile aborted, ledger untouched: {e}");
                return EXIT_GH;
            }
        };
        let anchor = gh
            .created_at
            .as_deref()
            .and_then(iso_to_epoch)
            .or_else(|| iso_to_epoch(&baseline.recorded_at))
            .unwrap_or(now);
        let age_days = (now - anchor).max(0) / 86400;
        if let Some(next) = reconcile_decision(current, &gh.state, age_days, stale_days) {
            let mut rec = baseline.clone();
            rec.recorded_at = now_iso();
            rec.state = next.clone();
            rec.state_changed_at = Some(now_iso());
            rec.checks = gh.checks;
            transitions.push(rec);
        }
    }

    // Write phase.
    if transitions.is_empty() {
        println!("reconcile: no state changes");
    } else {
        for rec in &transitions {
            append_jsonl(&ledger_path, rec);
            println!("reconcile: {} -> {}", rec.pr_url, rec.state);
        }
    }
    latest.extend(transitions.into_iter().map(|r| (r.pr_url.clone(), r)));
    let agg = aggregate(&latest);
    println!("{}", status_line(&agg));
    EXIT_OK
}

pub fn run_record_pr(args: &[String]) -> i32 {
    let url = match args.first() {
        Some(u) if !u.starts_with("--") => u.clone(),
        _ => {
            eprintln!("usage: epic-harness evolve record-pr <url> [--goal S] [--branch B]");
            return EXIT_USAGE;
        }
    };
    let (repo, number) = match parse_pr_url(&url) {
        Some(x) => x,
        None => {
            eprintln!("not a github PR URL: '{url}'");
            return EXIT_USAGE;
        }
    };
    let goal = flag_value(args, "--goal");
    let branch = flag_value(args, "--branch");

    let ledger_path = orbit_dir().join("outcomes.jsonl");
    if read_jsonl_typed::<Outcome>(&ledger_path)
        .iter()
        .any(|o| o.pr_url == url)
    {
        println!("record-pr: already tracked: {url}");
        return EXIT_OK;
    }
    let rec = Outcome {
        pr_url: url,
        repo,
        number,
        source: "manual".to_string(),
        pipeline_id: None,
        goal_slug: goal,
        branch,
        recorded_at: now_iso(),
        state: "pending".to_string(),
        state_changed_at: None,
        checks: 0,
    };
    append_jsonl(&ledger_path, &rec);
    println!("record-pr: registered as pending: {}", rec.pr_url);
    EXIT_OK
}

pub fn run_status() -> i32 {
    let ledger_path = orbit_dir().join("outcomes.jsonl");
    let latest = latest_by_pr(read_jsonl_typed::<Outcome>(&ledger_path));
    println!("{}", status_line(&aggregate(&latest)));
    EXIT_OK
}

// ── small local helpers ────────────────────────────────────────────────

fn flag_value(args: &[String], name: &str) -> Option<String> {
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

fn pipeline_files() -> Vec<std::path::PathBuf> {
    let dir = orbit_dir();
    let mut files: Vec<std::path::PathBuf> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| {
                            let n = n.to_string_lossy();
                            n.starts_with("PIPELINE-") && n.ends_with(".json")
                        })
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(url: &str, state: &str) -> Outcome {
        Outcome {
            pr_url: url.to_string(),
            repo: "o/r".to_string(),
            number: 1,
            source: "orbit".to_string(),
            pipeline_id: None,
            goal_slug: None,
            branch: None,
            recorded_at: "2026-09-01T00:00:00Z".to_string(),
            state: state.to_string(),
            state_changed_at: None,
            checks: 0,
        }
    }

    // ── parse_pr_url ──────────────────────────────────────────────

    #[test]
    fn parse_pr_url_accepts_canonical() {
        let (repo, n) = parse_pr_url("https://github.com/epicsagas/epic-harness/pull/129").unwrap();
        assert_eq!(repo, "epicsagas/epic-harness");
        assert_eq!(n, 129);
    }

    #[test]
    fn parse_pr_url_tolerates_trailing_slash() {
        assert_eq!(
            parse_pr_url("https://github.com/o/r/pull/4/").unwrap(),
            ("o/r".to_string(), 4)
        );
    }

    #[test]
    fn parse_pr_url_rejects_non_github_host() {
        assert!(parse_pr_url("https://gitlab.com/o/r/pull/4").is_none());
        assert!(parse_pr_url("https://github.com.evil.io/o/r/pull/4").is_none());
    }

    #[test]
    fn parse_pr_url_rejects_malformed() {
        assert!(parse_pr_url("https://github.com/o/r/issues/4").is_none());
        assert!(parse_pr_url("https://github.com/o/pull/4").is_none());
        assert!(parse_pr_url("https://github.com/o/r/pull/abc").is_none());
        assert!(parse_pr_url("not a url").is_none());
    }

    // ── iso_to_epoch ──────────────────────────────────────────────

    #[test]
    fn iso_to_epoch_known_value() {
        assert_eq!(iso_to_epoch("2026-09-19T00:00:00Z"), Some(1_789_776_000));
    }

    #[test]
    fn iso_to_epoch_tolerates_fractional_seconds() {
        assert_eq!(
            iso_to_epoch("2026-09-19T00:00:00.123456Z"),
            iso_to_epoch("2026-09-19T00:00:00Z")
        );
    }

    #[test]
    fn iso_to_epoch_round_trips_now_iso_shape() {
        let now = now_iso();
        let e = iso_to_epoch(&now).expect("now_iso must parse");
        assert!(e > 1_700_000_000, "sanity: post-2023 epoch, got {e}");
    }

    #[test]
    fn iso_to_epoch_rejects_garbage() {
        assert!(iso_to_epoch("").is_none());
        assert!(iso_to_epoch("yesterday").is_none());
        assert!(iso_to_epoch("2026-13-40T99:00:00Z").is_none());
    }

    // ── latest_by_pr / aggregate / status_line ────────────────────

    #[test]
    fn latest_record_wins_per_pr() {
        let latest = latest_by_pr(vec![
            outcome("https://github.com/o/r/pull/1", "pending"),
            outcome("https://github.com/o/r/pull/1", "merged"),
            outcome("https://github.com/o/r/pull/2", "pending"),
        ]);
        assert_eq!(latest.len(), 2);
        assert_eq!(latest["https://github.com/o/r/pull/1"].state, "merged");
    }

    #[test]
    fn aggregate_counts_and_pending_stays_out_of_denominator() {
        let latest = latest_by_pr(vec![
            outcome("a", "merged"),
            outcome("b", "merged"),
            outcome("c", "closed"),
            outcome("d", "stale"),
            outcome("e", "pending"),
            outcome("f", "pending"),
            outcome("g", "pending"),
        ]);
        let agg = aggregate(&latest);
        assert_eq!(agg.merged, 2);
        assert_eq!(agg.terminal(), 4);
        assert_eq!(agg.pending, 3);
        assert_eq!(agg.merge_rate(), Some(0.5));
    }

    #[test]
    fn merge_rate_none_when_no_terminal() {
        let latest = latest_by_pr(vec![outcome("a", "pending")]);
        let agg = aggregate(&latest);
        assert_eq!(agg.merge_rate(), None);
        assert_eq!(
            status_line(&agg),
            "merge_rate: n/a (0/0 terminal, 1 pending)"
        );
    }

    #[test]
    fn status_line_formats_percentage() {
        let latest = latest_by_pr(vec![
            outcome("a", "merged"),
            outcome("b", "merged"),
            outcome("c", "merged"),
            outcome("d", "merged"),
            outcome("e", "merged"),
            outcome("f", "closed"),
            outcome("g", "stale"),
            outcome("h", "pending"),
            outcome("i", "pending"),
            outcome("j", "pending"),
        ]);
        assert_eq!(
            status_line(&aggregate(&latest)),
            "merge_rate: 71% (5/7 terminal, 3 pending)"
        );
    }

    // ── reconcile_decision (state machine) ────────────────────────

    #[test]
    fn pending_transitions_on_gh_state() {
        assert_eq!(
            reconcile_decision("pending", "MERGED", 0, 14).as_deref(),
            Some("merged")
        );
        assert_eq!(
            reconcile_decision("pending", "CLOSED", 0, 14).as_deref(),
            Some("closed")
        );
        assert_eq!(reconcile_decision("pending", "OPEN", 1, 14), None);
        assert_eq!(reconcile_decision("", "OPEN", 1, 14), None);
    }

    #[test]
    fn stale_at_boundary_and_beyond() {
        assert_eq!(reconcile_decision("pending", "OPEN", 13, 14), None);
        assert_eq!(
            reconcile_decision("pending", "OPEN", 14, 14).as_deref(),
            Some("stale")
        );
        assert_eq!(
            reconcile_decision("pending", "OPEN", 40, 14).as_deref(),
            Some("stale")
        );
    }

    #[test]
    fn terminal_states_only_move_on_real_gh_transition() {
        // A merged PR is merged forever: gh OPEN (impossible for merged, but
        // defensive) and gh MERGED both leave the ledger untouched.
        assert_eq!(reconcile_decision("merged", "OPEN", 99, 14), None);
        assert_eq!(reconcile_decision("merged", "MERGED", 99, 14), None);
        assert_eq!(reconcile_decision("stale", "OPEN", 99, 14), None);
        // --all corrects a wrong ledger entry when gh reports the real state.
        assert_eq!(
            reconcile_decision("closed", "MERGED", 0, 14).as_deref(),
            Some("merged")
        );
        // Terminal PRs never go stale — stale is a pending-only outcome.
        assert_eq!(reconcile_decision("merged", "OPEN", 99, 1), None);
    }

    // ── pipeline recovery ─────────────────────────────────────────

    #[test]
    fn pipeline_urls_found_in_both_shapes() {
        let v: Value = serde_json::from_str(
            r#"{
                "pr_url": "https://github.com/o/r/pull/1",
                "goal_slug": "g",
                "branch": "b",
                "phase_history": [
                    {"phase": "ship", "pr": "https://github.com/o/r/pull/2"},
                    {"phase": "go", "note": "no pr here"}
                ]
            }"#,
        )
        .unwrap();
        let urls = pipeline_pr_urls(&v);
        let got: Vec<&str> = urls.iter().map(|(u, _, _)| u.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "https://github.com/o/r/pull/1",
                "https://github.com/o/r/pull/2"
            ]
        );
        assert_eq!(urls[0].1.as_deref(), Some("g"));
        assert_eq!(urls[0].2.as_deref(), Some("b"));
    }

    #[test]
    fn pipeline_entry_converts_to_baseline_outcome() {
        let o = outcome_from_pipeline(
            "https://github.com/o/r/pull/9",
            Some("20260919T133830Z".to_string()),
            Some("goal".to_string()),
            Some("orbit-goal".to_string()),
        )
        .unwrap();
        assert_eq!(o.state, "pending");
        assert_eq!(o.source, "orbit");
        assert_eq!(o.repo, "o/r");
        assert_eq!(o.number, 9);
        assert_eq!(o.pipeline_id.as_deref(), Some("20260919T133830Z"));
        assert!(iso_to_epoch(&o.recorded_at).is_some());
    }

    #[test]
    fn pipeline_entry_rejects_bad_url() {
        assert!(outcome_from_pipeline("junk", None, None, None).is_none());
    }
}
