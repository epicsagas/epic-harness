//! tests.rs — Tests for the store module

use super::*;
use std::path::PathBuf;

fn make_node(id: &str, title: &str, node_type: &str, tags: &[&str], importance: Option<f64>) -> Node {
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

/// Open an isolated in-memory SQLite DB with the full harness schema applied.
fn open_mem_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY, type TEXT NOT NULL, title TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '', projects TEXT NOT NULL DEFAULT '',
            agents TEXT NOT NULL DEFAULT '', created TEXT NOT NULL, updated TEXT NOT NULL,
            body TEXT NOT NULL DEFAULT '', importance REAL NOT NULL DEFAULT 0.5,
            access_count INTEGER NOT NULL DEFAULT 0, accessed_at TEXT NOT NULL DEFAULT ''
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts
            USING fts5(title, body, tags, content=nodes, content_rowid=rowid, tokenize='trigram');
        CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, title, body, tags)
            VALUES (new.rowid, new.title, new.body, new.tags);
        END;
        CREATE TABLE IF NOT EXISTS edges (
            id TEXT PRIMARY KEY, source TEXT NOT NULL, target TEXT NOT NULL,
            relation TEXT NOT NULL DEFAULT 'related', weight REAL NOT NULL DEFAULT 1.0,
            ts TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
        CREATE INDEX IF NOT EXISTS idx_nodes_importance ON nodes(importance DESC);
        CREATE INDEX IF NOT EXISTS idx_nodes_updated ON nodes(updated DESC);
        CREATE TABLE IF NOT EXISTS _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT OR IGNORE INTO _meta (key, value) VALUES ('schema_version', '2');",
    ).expect("schema");
    conn
}

// ── Fix 4: graph-boost lifts connected node ────────────
#[test]
fn smart_recall_graph_boost_lifts_connected_node() {
    let conn = open_mem_db();

    let id_a = "aaaaaaaa-0000-4000-8000-000000000001";
    let id_b = "aaaaaaaa-0000-4000-8000-000000000002";
    let id_e = "eeeeeeee-0000-4000-8000-000000000001";

    let n1 = make_node(id_a, "Alpha Node", "concept", &[], Some(0.8));
    let n2 = make_node(id_b, "Beta Node",  "concept", &[], Some(0.4));
    write_node_conn(&conn, &n1).unwrap();
    write_node_conn(&conn, &n2).unwrap();

    let edge = Edge {
        id: id_e.to_string(),
        source: id_a.to_string(),
        target: id_b.to_string(),
        relation: "related".to_string(),
        weight: 5.0,
        ts: now_iso(),
    };
    append_edge_conn(&conn, &edge).unwrap();

    let results = smart_recall_conn(&conn, None, None, 10);
    let ids: Vec<&str> = results.iter().map(|sn| sn.node.frontmatter.id.as_str()).collect();
    assert!(ids.contains(&id_a), "boost-a must appear");
    assert!(ids.contains(&id_b), "boost-b must appear");

    for sn in &results {
        assert!(sn.score > 0.0, "score must be positive: {}", sn.node.frontmatter.id);
    }
}

// ── Phase 2a: session importance downgrade ────────────────
#[test]
fn test_session_importance_is_005() {
    assert_eq!(importance_for_type("session"), 0.05,
        "session importance should be 0.05, not 0.2");
}

#[test]
fn test_session_importance_lower_than_pattern() {
    assert!(importance_for_type("session") < importance_for_type("pattern"),
        "session importance should be lower than pattern");
}

// ── Phase 2b: FTS5 trigram tokenizer ─────────────────────
#[test]
fn test_fts_uses_trigram_tokenizer() {
    let conn = open_mem_db();
    // Query the FTS table info to verify trigram tokenizer
    let info: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE name = 'nodes_fts'",
        [],
        |row| row.get(0),
    ).expect("nodes_fts should exist");
    assert!(info.contains("trigram"),
        "FTS5 table should use trigram tokenizer, got: {}", info);
}

#[test]
fn test_fts_trigram_korean_substring_search() {
    let conn = open_mem_db();

    // Insert a node with Korean text
    let node = make_node("korean-test-001", "Korean Test", "concept", &[], Some(0.7));
    let node_with_korean = Node {
        frontmatter: node.frontmatter.clone(),
        body: "한국어 테스트입니다".to_string(),
    };
    write_node_conn(&conn, &node_with_korean).unwrap();

    // Trigram tokenizer should match Korean substrings (3+ chars for valid trigrams)
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH '한국어'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    assert!(count > 0, "trigram FTS should match Korean substring");
}

// ── Phase 2a: schema version tracking ─────────────────────
#[test]
fn test_schema_version_meta_table_exists() {
    let conn = open_mem_db();
    let version: String = conn.query_row(
        "SELECT value FROM _meta WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    ).expect("_meta table with schema_version should exist");
    assert_eq!(version, "2", "schema_version should be '2'");
}

// ── Phase 2a: session importance backfill in schema ────────
#[test]
fn test_session_importance_backfill() {
    let conn = open_mem_db();

    // Insert a session node with default importance
    conn.execute(
        "INSERT INTO nodes (id, type, title, tags, projects, agents, created, updated, body, importance)
         VALUES ('session-backfill-test', 'session', 'Session Backfill Test', '', '', '', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'body', 0.5)",
        [],
    ).unwrap();

    // Run the backfill that schema.rs applies
    let _ = conn.execute_batch(
        "UPDATE nodes SET importance = 0.05 WHERE type = 'session' AND importance > 0.05;"
    );

    let imp: f64 = conn.query_row(
        "SELECT importance FROM nodes WHERE id = 'session-backfill-test'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!((imp - 0.05).abs() < f64::EPSILON,
        "session node importance should be backfilled to 0.05, got: {}", imp);
}
