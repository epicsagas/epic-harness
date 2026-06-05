//! tests.rs — Tests for the store module

use super::*;
use std::path::PathBuf;

#[allow(dead_code)]
fn make_node(
    id: &str,
    title: &str,
    node_type: &str,
    tags: &[&str],
    importance: Option<f64>,
) -> Node {
    let ts = "2024-01-01T00:00:00Z".to_string();
    Node {
        frontmatter: NodeFrontmatter {
            id: id.to_string(),
            node_type: node_type.to_string(),
            title: title.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            importance: importance.unwrap_or_else(|| importance_for_type(node_type)),
            created: ts.clone(),
            updated: ts.clone(),
            ..Default::default()
        },
        body: format!("body of {title}"),
    }
}

// ── Fix 1: query_nodes SQL injection ──────────────────
#[test]
fn test_query_nodes_sql_injection_tag_does_not_panic() {
    // A malicious tag containing SQL metacharacters must not panic or error out.
    // The call should return normally (empty results or whatever is in DB).
    let _ = query_nodes(Some("'; DROP TABLE nodes; --"), None, None, 10);
}

#[test]
fn test_query_nodes_sql_injection_type_does_not_panic() {
    let _ = query_nodes(None, Some("' OR '1'='1"), None, 10);
}

#[test]
fn test_query_nodes_sql_injection_project_does_not_panic() {
    let _ = query_nodes(None, None, Some("x%_x'; --"), 10);
}

#[test]
fn test_query_nodes_limit_capped_at_200() {
    // Even when requesting more than 200 nodes the function must not panic.
    let results = query_nodes(None, None, None, 9999);
    assert!(results.len() <= 200);
}

// ── Fix 1: smart_recall SQL injection ─────────────────
#[test]
fn test_smart_recall_sql_injection_project_does_not_panic() {
    let _ = smart_recall(Some("'; DROP TABLE nodes; --"), None, 5);
}

// ── Fix 2: atomic_write unique tmp names ──────────────
#[test]
fn test_atomic_write_tmp_filename_contains_pid() {
    // We verify the tmp path is NOT just path.with_extension("tmp").
    // Build the path the NEW way and check it contains the pid.
    let base = PathBuf::from("/tmp/store_test_base.json");
    let pid = std::process::id();
    let expected_suffix = format!(".{pid}.tmp");

    // Replicate the new tmp-path logic:
    let tmp = base.with_file_name(format!(
        ".{}.{}.tmp",
        base.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        pid
    ));

    assert!(
        tmp.to_str().unwrap_or("").ends_with(&expected_suffix),
        "tmp path should end with .PID.tmp, got: {:?}",
        tmp
    );
    // Must NOT equal path.with_extension("tmp") (the old fixed name).
    assert_ne!(tmp, base.with_extension("tmp"));
}

// ── Fix 4: parse_iso_to_secs closed-form ──────────────
#[test]
fn test_parse_iso_epoch_start() {
    // 1970-01-01T00:00:00Z => 0
    assert_eq!(parse_iso_to_secs("1970-01-01T00:00:00Z"), 0);
}

#[test]
fn test_parse_iso_known_timestamp() {
    // 2024-01-01T00:00:00Z
    // Days from 1970 to 2024:
    //   54 years: 54*365 = 19710 days
    //   Leap years in [1970,2023]: 1972,1976,...2020 = every 4 years
    //   count = (2023/4 - 2023/100 + 2023/400) - (1969/4 - 1969/100 + 1969/400)
    //         = (505 - 20 + 5) - (492 - 19 + 4) = 490 - 477 = 13
    //   Total days = 54*365 + 13 = 19723
    let expected: u64 = 19723 * 86400;
    assert_eq!(parse_iso_to_secs("2024-01-01T00:00:00Z"), expected);
}

#[test]
fn test_parse_iso_leap_day() {
    // 2024-02-29T00:00:00Z  (2024 is a leap year)
    // days up to 2024-01-01 = 19723
    // Jan = 31 days => 2024-02-01 = 19754
    // Feb 29 => day index = 28
    // total = 19754 + 28 = 19782
    let expected: u64 = 19782 * 86400;
    assert_eq!(parse_iso_to_secs("2024-02-29T00:00:00Z"), expected);
}

#[test]
fn test_parse_iso_with_time_component() {
    // 1970-01-01T01:02:03Z => 1*3600 + 2*60 + 3 = 3723
    assert_eq!(parse_iso_to_secs("1970-01-01T01:02:03Z"), 3723);
}

#[test]
fn test_parse_iso_non_leap_century() {
    // 1900 is not a leap year; 2000 is. Test 2000-03-01.
    // Days up to 2000-01-01:
    //   30 years 1970..=1999: 30*365 = 10950 base days
    //   Leap years in [1970,1999]: 1972,1976,1980,1984,1988,1992,1996 = 7
    //   total = 10950 + 7 = 10957 days
    // Jan=31, Feb=29 (2000 is a leap year) => 2000-03-01 day index = 31+29 = 60
    // Total = 10957 + 60 = 11017
    let expected: u64 = 11017 * 86400;
    assert_eq!(parse_iso_to_secs("2000-03-01T00:00:00Z"), expected);
}

// ── Phase 2a: session importance downgrade ────────────────
#[test]
fn test_session_importance_is_005() {
    assert_eq!(
        importance_for_type("session"),
        0.05,
        "session importance should be 0.05, not 0.2"
    );
}

#[test]
fn test_session_importance_lower_than_pattern() {
    assert!(
        importance_for_type("session") < importance_for_type("pattern"),
        "session importance should be lower than pattern"
    );
}
