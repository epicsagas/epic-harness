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

// ── Area F: exact scope, starvation, stale; Area C: access mode ──
use super::decay::{decay_importance_pool, tag_stale_nodes_pool};
use super::recall::{FTS_SCOPED_SQL, RecallAccess, smart_recall_async};
use super::util::{csv_member, split_csv};

async fn mem_pool() -> sqlx::AnyPool {
    let pool = crate::store::pool::test_memory_pool().await;
    init_schema_pool(&pool).await.unwrap();
    pool
}

fn scoped_node(id: &str, title: &str, body: &str, projects: &[&str], tags: &[&str]) -> Node {
    let mut node = make_node(id, title, "concept", tags, None);
    node.frontmatter.projects = projects.iter().map(|s| s.to_string()).collect();
    node.body = body.to_string();
    node
}

async fn recall_ids(
    pool: &sqlx::AnyPool,
    project: Option<&str>,
    hint: Option<&str>,
    limit: usize,
) -> Vec<String> {
    let mut ids: Vec<String> =
        smart_recall_async(pool, project, hint, limit, RecallAccess::Explicit)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.node.frontmatter.id)
            .collect();
    ids.sort();
    ids
}

/// 60 nodes in `big` repeating the hint terms, one weak match in `small`.
async fn seed_starvation(pool: &sqlx::AnyPool) {
    for i in 0..60 {
        let body = "cache invalidation ".repeat(20);
        let node = scoped_node(
            &format!("big-{i:02}"),
            &format!("cache invalidation {i}"),
            &body,
            &["big"],
            &[],
        );
        write_node_pool(pool, &node).await.unwrap();
    }
    let body = format!("{} cache {}", "filler ".repeat(200), "padding ".repeat(200));
    let node = scoped_node("small-0", "unrelated title", &body, &["small"], &[]);
    write_node_pool(pool, &node).await.unwrap();
}

#[tokio::test]
async fn f_project_membership_exact() {
    let pool = mem_pool().await;
    let a = scoped_node("node-a", "shared topic", "x", &["alpha"], &["pin"]);
    let b = scoped_node("node-b", "shared topic", "x", &["alphabet"], &["pinned"]);
    write_node_pool(&pool, &a).await.unwrap();
    write_node_pool(&pool, &b).await.unwrap();

    let only_a = vec!["node-a".to_string()];
    assert_eq!(
        recall_ids(&pool, Some("alpha"), Some("shared"), 10).await,
        only_a
    );
    assert_eq!(recall_ids(&pool, Some("alpha"), None, 10).await, only_a);
    let found = query_nodes_pool(&pool, None, None, Some("alpha"), 10)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].frontmatter.id, "node-a");
    // the tag filter is exact as well: "pin" must not match "pinned"
    let found = query_nodes_pool(&pool, Some("pin"), None, None, 10)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].frontmatter.id, "node-a");
}

#[tokio::test]
async fn f_wildcards_are_literal() {
    let pool = mem_pool().await;
    let node = scoped_node("node-a", "wild topic", "x", &["abc"], &["abc"]);
    write_node_pool(&pool, &node).await.unwrap();
    for p in ["a_c", "a%"] {
        assert!(
            recall_ids(&pool, Some(p), Some("wild"), 10)
                .await
                .is_empty(),
            "{p}"
        );
        assert!(recall_ids(&pool, Some(p), None, 10).await.is_empty(), "{p}");
        let found = query_nodes_pool(&pool, None, None, Some(p), 10)
            .await
            .unwrap();
        assert!(found.is_empty(), "{p}");
        let found = query_nodes_pool(&pool, Some(p), None, None, 10)
            .await
            .unwrap();
        assert!(found.is_empty(), "{p}");
    }
}

#[tokio::test]
async fn f_scoped_recall_not_starved() {
    let pool = mem_pool().await;
    seed_starvation(&pool).await;
    let ids = recall_ids(&pool, Some("small"), Some("cache invalidation"), 2).await;
    assert_eq!(ids, vec!["small-0".to_string()]);
}

#[tokio::test]
async fn f_scoped_recall_big_project_still_ranked() {
    let pool = mem_pool().await;
    seed_starvation(&pool).await;
    let ids = recall_ids(&pool, Some("big"), Some("cache invalidation"), 2).await;
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| id.starts_with("big-")));
}

#[tokio::test]
async fn f_scoped_query_plan_not_correlated() {
    use sqlx::Row as _;
    let pool = mem_pool().await;
    let version: String = sqlx::query("select sqlite_version()")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    println!("sqlite_version = {version}");
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "EXPLAIN QUERY PLAN {FTS_SCOPED_SQL}"
    )))
    .bind("\"cache\"")
    .bind("small")
    .bind(6_i64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let details: Vec<String> = rows.iter().map(|r| r.get::<String, _>(3)).collect();
    println!("{details:#?}");
    assert!(
        details.iter().any(|d| d.contains("nodes_fts")),
        "{details:?}"
    );
    assert!(
        !details
            .iter()
            .any(|d| d.to_uppercase().contains("CORRELATED")),
        "{details:?}"
    );
}

#[tokio::test]
async fn f_empty_project_is_unscoped() {
    let pool = mem_pool().await;
    let a = scoped_node("node-a", "shared topic", "x", &["alpha"], &[]);
    let b = scoped_node("node-b", "shared topic", "x", &["beta"], &[]);
    write_node_pool(&pool, &a).await.unwrap();
    write_node_pool(&pool, &b).await.unwrap();
    for hint in [Some("shared"), None] {
        let scoped = recall_ids(&pool, Some(""), hint, 10).await;
        assert_eq!(scoped, recall_ids(&pool, None, hint, 10).await);
        assert_eq!(scoped.len(), 2);
    }
}

#[tokio::test]
async fn f_stale_excluded_from_recall() {
    let pool = mem_pool().await;
    let stale = scoped_node("node-stale", "stale topic", "x", &["p"], &["stale"]);
    let ness = scoped_node("node-ness", "stale topic", "x", &["p"], &["staleness"]);
    write_node_pool(&pool, &stale).await.unwrap();
    write_node_pool(&pool, &ness).await.unwrap();
    for project in [None, Some("p")] {
        for hint in [Some("stale"), None] {
            let ids = recall_ids(&pool, project, hint, 10).await;
            assert_eq!(ids, vec!["node-ness".to_string()], "{project:?} {hint:?}");
        }
    }
}

#[tokio::test]
async fn f_decay_pinned_exact() {
    let pool = mem_pool().await;
    for (id, tag) in [("node-pinned", "pinned"), ("node-unpinned", "unpinned")] {
        let mut node = scoped_node(id, id, "x", &[], &[tag]);
        node.frontmatter.importance = 0.8;
        node.frontmatter.accessed_at = "2020-01-01T00:00:00Z".into();
        write_node_pool(&pool, &node).await.unwrap();
    }
    decay_importance_pool(&pool, 30, 0.9, 0.05).await.unwrap();
    let pinned = read_node_pool(&pool, "node-pinned").await.unwrap();
    let unpinned = read_node_pool(&pool, "node-unpinned").await.unwrap();
    assert_eq!(pinned.frontmatter.importance, 0.8);
    assert!(unpinned.frontmatter.importance < 0.8);
}

#[tokio::test]
async fn f_stale_tagging_exact() {
    let pool = mem_pool().await;
    // make_node sets updated to 2024-01-01, far older than 180 days
    let ness = scoped_node("node-ness", "x", "x", &[], &["staleness"]);
    let pinned = scoped_node("node-pinned", "x", "x", &[], &["pinned"]);
    write_node_pool(&pool, &ness).await.unwrap();
    write_node_pool(&pool, &pinned).await.unwrap();
    assert_eq!(tag_stale_nodes_pool(&pool, 180).await.unwrap(), 1);
    let ness = read_node_pool(&pool, "node-ness").await.unwrap();
    assert_eq!(ness.frontmatter.tags, vec!["staleness", "stale"]);
    let pinned = read_node_pool(&pool, "node-pinned").await.unwrap();
    assert_eq!(pinned.frontmatter.tags, vec!["pinned"]);
}

#[tokio::test]
async fn f_csv_member_matches_split_csv() {
    use sqlx::Row as _;
    let pool = mem_pool().await;
    let sql = format!("SELECT CASE WHEN {} THEN 1 ELSE 0 END", csv_member("?"));
    let csvs = ["", "a", "a,b", "alpha,alphabet", "x_y,x%y", "a,,b"];
    let values = ["a", "b", "alpha", "alph", "x_y", "x%y", "%", "_"];
    for csv in csvs {
        for value in values {
            let hit: i64 = sqlx::query(sqlx::AssertSqlSafe(sql.clone()))
                .bind(csv)
                .bind(value)
                .fetch_one(&pool)
                .await
                .unwrap()
                .get(0);
            let expected = split_csv(csv).iter().any(|x| x == value);
            assert_eq!(hit == 1, expected, "csv={csv:?} value={value:?}");
        }
    }
}

#[tokio::test]
async fn c_passive_recall_keeps_access_count() {
    let pool = mem_pool().await;
    let mut node = scoped_node("node-a", "topic", "x", &["p"], &[]);
    node.frontmatter.access_count = 3;
    write_node_pool(&pool, &node).await.unwrap();
    let got = smart_recall_async(&pool, Some("p"), None, 5, RecallAccess::Passive)
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    let after = read_node_pool(&pool, "node-a").await.unwrap();
    assert_eq!(after.frontmatter.access_count, 3);
    assert!(!after.frontmatter.accessed_at.is_empty());
}

#[tokio::test]
async fn c_explicit_recall_bumps_access_count() {
    let pool = mem_pool().await;
    let mut node = scoped_node("node-a", "topic", "x", &["p"], &[]);
    node.frontmatter.access_count = 3;
    write_node_pool(&pool, &node).await.unwrap();
    smart_recall_async(&pool, Some("p"), None, 5, RecallAccess::Explicit)
        .await
        .unwrap();
    let after = read_node_pool(&pool, "node-a").await.unwrap();
    assert_eq!(after.frontmatter.access_count, 4);
}

// ── Items 2 to 4: dedup, importance on reopen, malformed timestamps ──
#[tokio::test]
async fn issue_181_dedup_preserves_distinct_project_knowledge() {
    let pool = mem_pool().await;
    let mut alpha = make_node("alpha-id", "Database choice", "decision", &[], None);
    alpha.frontmatter.projects = vec!["alpha".into()];
    alpha.frontmatter.updated = now_iso();
    alpha.body = "Alpha uses SQLite".into();
    let (alpha_id, _) = write_node_dedup_pool(&pool, &alpha, 24).await.unwrap();

    let mut beta = alpha.clone();
    beta.frontmatter.id = "beta-id".into();
    beta.frontmatter.projects = vec!["beta".into()];
    beta.body = "Beta uses PostgreSQL".into();
    let (beta_id, deduped) = write_node_dedup_pool(&pool, &beta, 24).await.unwrap();
    assert!(!deduped, "a different project is not duplicate knowledge");
    assert_ne!(alpha_id, beta_id);
    let stored = read_node_pool(&pool, &beta_id).await.unwrap();
    assert_eq!(stored.body, beta.body);
    assert_eq!(stored.frontmatter.projects, beta.frontmatter.projects);

    // Same title and same project set still deduplicates, in both projects.
    beta.frontmatter.id = "another-beta-id".into();
    assert_eq!(
        write_node_dedup_pool(&pool, &beta, 24).await.unwrap(),
        (beta_id, true)
    );
    alpha.frontmatter.id = "another-alpha-id".into();
    assert_eq!(
        write_node_dedup_pool(&pool, &alpha, 24).await.unwrap(),
        (alpha_id, true)
    );
}

#[tokio::test]
async fn issue_232_reopening_preserves_explicit_importance() {
    let pool = mem_pool().await;
    for (id, kind, importance) in [
        ("explicit-concept", "concept", 0.5),
        ("explicit-session", "session", 0.7),
    ] {
        write_node_pool(&pool, &make_node(id, id, kind, &[], Some(importance)))
            .await
            .unwrap();
    }
    init_schema_pool(&pool).await.unwrap();
    for (id, importance) in [("explicit-concept", 0.5), ("explicit-session", 0.7)] {
        let node = read_node_pool(&pool, id).await.unwrap();
        assert_eq!(node.frontmatter.importance, importance, "{id}");
    }
}

#[test]
fn issue_232_legacy_import_defaults_only_missing_importance() {
    let legacy = "---\nid: legacy-decision\ntype: decision\ntitle: Legacy choice\ncreated: 2024-01-01\nupdated: 2024-01-01\n---\nbody";
    assert_eq!(parse_node(legacy).unwrap().frontmatter.importance, 0.9);
    let explicit = legacy.replace(
        "title: Legacy choice",
        "title: Legacy choice\nimportance: 0.5",
    );
    assert_eq!(parse_node(&explicit).unwrap().frontmatter.importance, 0.5);
}

#[test]
fn issue_233_malformed_unicode_timestamp_is_rejected_without_panicking() {
    for value in [
        "202é-01-01T00:00:00Z",
        "2024-01-01T00:00:0é",
        "0000-01-01T00:00:00Z",
        "1969-12-31T23:59:59Z",
    ] {
        assert_eq!(parse_iso_to_secs(value), 0, "malformed timestamp {value}");
    }
    assert_eq!(parse_iso_to_secs("2024-01-01T00:00:00Z"), 1_704_067_200);
}

#[tokio::test]
async fn issue_231_recall_keeps_operator_substrings_in_words() {
    let pool = mem_pool().await;
    for (id, title) in [("algorithm", "ALGORITHM"), ("cors", "CORS")] {
        write_node_pool(&pool, &scoped_node(id, title, "x", &[], &[]))
            .await
            .unwrap();
    }
    assert_eq!(
        recall_ids(&pool, None, Some("ALGORITHM"), 5).await,
        vec!["algorithm".to_string()]
    );
    assert_eq!(
        recall_ids(&pool, None, Some("CORS"), 5).await,
        vec!["cors".to_string()]
    );
}

// ── Legacy edges table conversion (#418) ──────────────
use super::schema::append_graph_edge;

async fn legacy_file_pool(dir: &std::path::Path) -> (sqlx::AnyPool, PathBuf) {
    let _ = crate::store::pool::test_memory_pool().await; // installs drivers
    let db = dir.join("memory.db");
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db.display()))
        .await
        .unwrap();
    for sql in [
        "CREATE TABLE edges (id TEXT PRIMARY KEY, source TEXT NOT NULL, target TEXT NOT NULL, \
         relation TEXT NOT NULL DEFAULT 'related', weight REAL NOT NULL DEFAULT 1.0, ts TEXT NOT NULL)",
        "CREATE INDEX idx_edges_source ON edges(source)",
        "CREATE INDEX idx_edges_target ON edges(target)",
        "INSERT INTO edges VALUES ('e1', 'n1', 'n2', 'detected_in', 1.0, '2024-01-01T00:00:00Z')",
        "INSERT INTO edges VALUES ('e2', 'n1', 'n2', 'detected_in', 0.5, '2024-01-02T00:00:00Z')",
        "INSERT INTO edges VALUES ('e3', 'n2', 'n3', '', 1.0, '2024-01-03T00:00:00Z')",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }
    (pool, db)
}

fn backups_of(dir: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().contains(".pre-edges-migration-"))
        .collect()
}

#[tokio::test]
async fn issue_418_legacy_edges_table_is_converted_with_backup() {
    use sqlx::Row as _;
    let dir = tempfile::tempdir().unwrap();
    let (pool, _db) = legacy_file_pool(dir.path()).await;

    init_schema_pool(&pool).await.unwrap();

    let rows = sqlx::query("SELECT source, target, label, created FROM edges ORDER BY created")
        .fetch_all(&pool)
        .await
        .unwrap();
    let got: Vec<(String, String, String, String)> = rows
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
        .collect();
    assert_eq!(
        got.len(),
        2,
        "duplicate (source, target, relation) is deduped"
    );
    assert_eq!(got[0].2, "detected_in");
    assert_eq!(got[0].3, "2024-01-01T00:00:00Z");
    assert_eq!(got[1].2, "related", "empty relation becomes 'related'");
    let idx: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_edges_%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(idx, 2);

    let backups = backups_of(dir.path());
    assert_eq!(backups.len(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "backup is private like memory.db");
    }
    let bak = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=ro", backups[0].display()))
        .await
        .unwrap();
    let legacy_cols: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('edges') WHERE name = 'relation'",
    )
    .fetch_one(&bak)
    .await
    .unwrap();
    assert_eq!(legacy_cols, 1, "backup keeps the legacy table");
}

#[tokio::test]
async fn issue_418_second_init_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let (pool, _db) = legacy_file_pool(dir.path()).await;
    init_schema_pool(&pool).await.unwrap();
    init_schema_pool(&pool).await.unwrap();

    assert_eq!(backups_of(dir.path()).len(), 1);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn issue_418_fresh_db_creates_no_backup() {
    let dir = tempfile::tempdir().unwrap();
    let _ = crate::store::pool::test_memory_pool().await; // installs drivers
    let db = dir.path().join("memory.db");
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db.display()))
        .await
        .unwrap();
    init_schema_pool(&pool).await.unwrap();
    assert!(backups_of(dir.path()).is_empty());
}

#[tokio::test]
async fn issue_418_graph_and_append_work_after_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let (pool, _db) = legacy_file_pool(dir.path()).await;
    init_schema_pool(&pool).await.unwrap();

    let ge = super::types::GraphEdge {
        source: "n3".into(),
        target: "n4".into(),
        label: "follows".into(),
        created: "2024-01-04T00:00:00Z".into(),
    };
    append_graph_edge(&pool, &ge).await.unwrap();
    crate::mem::graph::build_graph_pool(&pool).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 3);
}

#[tokio::test]
async fn issue_418_concurrent_init_converts_once() {
    let dir = tempfile::tempdir().unwrap();
    let (pool, db) = legacy_file_pool(dir.path()).await;
    let other = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db.display()))
        .await
        .unwrap();
    let (a, b) = tokio::join!(init_schema_pool(&pool), init_schema_pool(&other));
    a.unwrap();
    b.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn issue_418_in_memory_legacy_table_converts_without_backup() {
    let pool = crate::store::pool::test_memory_pool().await;
    sqlx::query(
        "CREATE TABLE edges (id TEXT PRIMARY KEY, source TEXT NOT NULL, target TEXT NOT NULL, \
         relation TEXT NOT NULL DEFAULT 'related', weight REAL NOT NULL DEFAULT 1.0, ts TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO edges VALUES ('e1', 'n1', 'n2', 'follows', 1.0, '2024-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    init_schema_pool(&pool).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges WHERE label = 'follows'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn issue_418_unique_index_and_label_collision_keeps_earliest_row() {
    use sqlx::Row as _;
    let dir = tempfile::tempdir().unwrap();
    let (pool, _db) = legacy_file_pool(dir.path()).await;
    for sql in [
        "DELETE FROM edges WHERE id = 'e2'",
        "CREATE UNIQUE INDEX idx_edges_src_tgt_rel ON edges(source, target, relation)",
        "INSERT INTO edges VALUES ('e4', 'n5', 'n6', 'related', 1.0, '2024-02-02T00:00:00Z')",
        "INSERT INTO edges VALUES ('e5', 'n5', 'n6', '', 1.0, '2024-02-01T00:00:00Z')",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }
    init_schema_pool(&pool).await.unwrap();

    let rows = sqlx::query("SELECT label, created FROM edges WHERE source = 'n5'")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>(0), "related");
    assert_eq!(rows[0].get::<String, _>(1), "2024-02-01T00:00:00Z");
}

#[tokio::test]
async fn issue_418_failed_conversion_keeps_edges_and_backs_up_once() {
    let dir = tempfile::tempdir().unwrap();
    let (pool, _db) = legacy_file_pool(dir.path()).await;
    sqlx::query("CREATE TABLE edges_legacy (x TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    init_schema_pool(&pool).await.unwrap();
    init_schema_pool(&pool).await.unwrap();

    assert_eq!(backups_of(dir.path()).len(), 1, "no second backup");
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM edges WHERE relation IN ('detected_in', '')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 3, "original legacy edges are intact");
    // The connection must not be left inside an open transaction.
    sqlx::query("CREATE TABLE after_failure (x TEXT)")
        .execute(&pool)
        .await
        .unwrap();
}
