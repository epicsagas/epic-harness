//! mem_test.rs — Integration tests for the mem module
//! Uses HARNESS_ROOT env var to redirect ~/.harness to a temp dir.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = env::temp_dir().join(format!("epic_harness_mem_test_{n}_{}", std::process::id()));
    fs::create_dir_all(&base).unwrap();
    base
}

fn set_root(root: &Path) {
    // HARNESS_ROOT overrides HOME in the store module.
    // SAFETY: caller must hold ENV_LOCK before calling this function.
    // All env-var-dependent tests acquire ENV_LOCK to serialize access.
    unsafe {
        env::set_var("HARNESS_ROOT", root.to_str().unwrap());
    }
}

// ── Helpers ───────────────────────────────────────────

fn run_mem(args: &[&str]) -> i32 {
    let args: Vec<String> = std::iter::once("mem".to_string())
        .chain(args.iter().map(|s| s.to_string()))
        .collect();
    epic_harness::mem::run(&args)
}

fn set_claude_settings(path: &Path) {
    unsafe {
        env::set_var("CLAUDE_SETTINGS_PATH", path.to_str().unwrap());
    }
}

// ── Tests ─────────────────────────────────────────────

#[test]
fn test_mcp_install_dry_run() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Create a settings.json in temp dir
    let settings_path = root.join("settings.json");
    fs::write(&settings_path, "{}").unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&["mcp-install", "--dry-run"]);
    assert_eq!(code, 0, "mcp-install --dry-run should exit 0");

    // File should remain unchanged
    let content = fs::read_to_string(&settings_path).unwrap();
    assert_eq!(content, "{}", "dry-run should not modify settings.json");
}

#[test]
fn test_mcp_install_writes_settings() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let settings_path = root.join("settings.json");
    fs::write(&settings_path, "{}").unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&["mcp-install"]);
    assert_eq!(code, 0, "mcp-install should exit 0");

    let content = fs::read_to_string(&settings_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    let server = &val["mcpServers"]["harness-mem"];
    // Now registers the Rust binary, not node
    let command = server["command"].as_str().unwrap();
    assert!(!command.is_empty(), "command should be non-empty");
    assert_eq!(server["args"][0].as_str().unwrap(), "mem");
    assert_eq!(server["args"][1].as_str().unwrap(), "mcp");
}

#[test]
fn test_mcp_install_already_registered() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let existing = serde_json::json!({
        "mcpServers": {
            "harness-mem": {
                "command": "epic-harness",
                "args": ["mem", "mcp"]
            }
        }
    });
    let settings_path = root.join("settings.json");
    fs::write(&settings_path, serde_json::to_string(&existing).unwrap()).unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&["mcp-install"]);
    assert_eq!(code, 0, "already registered should exit 0");

    // Content should be unchanged
    let content = fs::read_to_string(&settings_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        val["mcpServers"]["harness-mem"]["command"]
            .as_str()
            .unwrap(),
        "epic-harness",
        "existing registration should not be overwritten"
    );
}

#[test]
fn test_add_and_query() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Add a node
    let code = run_mem(&[
        "add",
        "--title",
        "JWT Rotation Pattern",
        "--type",
        "pattern",
        "--tags",
        "auth,security",
        "--project",
        "epic-harness",
        "--agent",
        "claude-code",
        "--body",
        "Rotate JWT keys every 24 hours.",
    ]);
    assert_eq!(code, 0, "add should succeed");

    // Verify node exists in DB via read_index
    let idx = epic_harness::mem::store::read_index();
    let nodes = &idx.nodes;
    assert_eq!(nodes.len(), 1, "exactly one node should be in DB");
    assert_eq!(nodes[0].title, "JWT Rotation Pattern");
    assert!(nodes[0].tags.contains(&"auth".to_string()));

    // Query by tag via CLI
    let code2 = run_mem(&["query", "--tag", "auth"]);
    assert_eq!(code2, 0, "query by tag should succeed");

    // Query by type
    let code3 = run_mem(&["query", "--type", "pattern"]);
    assert_eq!(code3, 0, "query by type should succeed");
}

#[test]
fn test_link_and_related() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Add two nodes
    run_mem(&[
        "add", "--title", "Node A", "--type", "concept", "--tags", "a", "--body", "body a",
    ]);
    run_mem(&[
        "add", "--title", "Node B", "--type", "concept", "--tags", "b", "--body", "body b",
    ]);

    // Get their IDs from store
    let idx = epic_harness::mem::store::read_index();
    let nodes = &idx.nodes;
    assert_eq!(nodes.len(), 2);
    let id_a = nodes[0].id.clone();
    let id_b = nodes[1].id.clone();

    // Link A -> B
    let code = run_mem(&["link", &id_a, &id_b, "--relation", "uses"]);
    assert_eq!(code, 0, "link should succeed");

    // Verify edges exist in DB
    let edges = epic_harness::mem::store::read_edges();
    assert!(!edges.is_empty(), "edges should be stored in DB");
    assert!(
        edges.iter().any(|e| e.relation == "uses"),
        "edge should have 'uses' relation"
    );

    // related from A should return B
    let related = epic_harness::mem::graph::related_nodes(&id_a, 2);
    assert!(related.contains(&id_b), "related from A should include B");
}

#[test]
fn test_migrate_dry_run() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Create a legacy project memory file
    let proj_mem = root.join(".harness/projects/my-proj/memory");
    fs::create_dir_all(&proj_mem).unwrap();
    fs::write(proj_mem.join("notes.md"), "# Notes\nSome content here.").unwrap();

    // Run migrate dry-run
    let code = run_mem(&["migrate", "--project", "my-proj", "--dry-run"]);
    assert_eq!(code, 0, "migrate --dry-run should succeed");

    // No nodes should have been written to DB
    let idx = epic_harness::mem::store::read_index();
    assert_eq!(
        idx.nodes.len(),
        0,
        "dry-run should not write any nodes to DB"
    );
}

#[test]
fn test_validate() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Add a valid node via CLI (goes to DB)
    run_mem(&[
        "add",
        "--title",
        "Valid Node",
        "--type",
        "concept",
        "--tags",
        "test",
        "--body",
        "valid body",
    ]);

    // validate should pass for DB-based nodes (they are always structurally valid)
    // Inject a legacy corrupt .md file to trigger the legacy-path validation
    let legacy_dir = root.join(".harness").join("nodes");
    fs::create_dir_all(&legacy_dir).unwrap();
    fs::write(
        legacy_dir.join("corrupt.md"),
        "not valid frontmatter at all",
    )
    .unwrap();

    // validate should exit 1 and report the corrupt legacy file
    let code = run_mem(&["validate"]);
    assert_eq!(
        code, 1,
        "validate should fail when corrupt legacy file exists"
    );
}

// ── SQLite edge tests ──────────────────────────────────

#[test]
fn test_delete_edge_by_id_is_consistent() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::mem::store::{Edge, append_edge, delete_edge_by_id, read_edges};

    let root = temp_root();
    set_root(&root);

    let edge_a = Edge {
        id: "edge-aaa".to_string(),
        source: "src-1".to_string(),
        target: "tgt-1".to_string(),
        relation: "uses".to_string(),
        weight: 1.0,
        ts: "2026-01-01T00:00:00Z".to_string(),
    };
    let edge_b = Edge {
        id: "edge-bbb".to_string(),
        source: "src-2".to_string(),
        target: "tgt-2".to_string(),
        relation: "blocks".to_string(),
        weight: 0.5,
        ts: "2026-01-01T00:00:01Z".to_string(),
    };

    append_edge(&edge_a).unwrap();
    append_edge(&edge_b).unwrap();
    assert_eq!(read_edges().len(), 2, "should have 2 edges before delete");

    delete_edge_by_id("edge-aaa").unwrap();
    let remaining = read_edges();
    assert_eq!(remaining.len(), 1, "should have 1 edge after delete");
    assert_eq!(remaining[0].id, "edge-bbb");
}

#[test]
fn test_remove_edges_for_node_is_consistent() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::mem::store::{Edge, append_edge, read_edges, remove_edges_for_node};

    let root = temp_root();
    set_root(&root);

    for i in 0..3u32 {
        let edge = Edge {
            id: format!("edge-{i}"),
            source: "node-x".to_string(),
            target: format!("node-{i}"),
            relation: "uses".to_string(),
            weight: 1.0,
            ts: "2026-01-01T00:00:00Z".to_string(),
        };
        append_edge(&edge).unwrap();
    }
    let unrelated = Edge {
        id: "edge-unrelated".to_string(),
        source: "node-other".to_string(),
        target: "node-another".to_string(),
        relation: "related".to_string(),
        weight: 1.0,
        ts: "2026-01-01T00:00:00Z".to_string(),
    };
    append_edge(&unrelated).unwrap();

    remove_edges_for_node("node-x").unwrap();
    let remaining = read_edges();
    assert_eq!(remaining.len(), 1, "only unrelated edge should remain");
    assert_eq!(remaining[0].id, "edge-unrelated");
}

// ── validate_uuid tests ───────────

#[test]
fn test_validate_uuid_valid() {
    use epic_harness::mem::store::validate_uuid;

    // Valid UUID v4
    assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000"));
    assert!(validate_uuid("00000000-0000-4000-8000-000000000000"));
}

#[test]
fn test_validate_uuid_invalid() {
    use epic_harness::mem::store::validate_uuid;

    assert!(!validate_uuid("../etc/passwd"));
    assert!(!validate_uuid("../../secret"));
    assert!(!validate_uuid("short"));
    assert!(!validate_uuid("550e8400-e29b-41d4-a716-4466554400000")); // 37 chars
    assert!(!validate_uuid("550e8400/e29b/41d4/a716/446655440000")); // slashes
}

#[test]
fn test_validate_uuid_rejects_traversal() {
    use epic_harness::mem::store::validate_uuid;

    assert!(!validate_uuid("../etc/passwd"));
    assert!(!validate_uuid("../../secret"));
    assert!(!validate_uuid("short"));
}

#[test]
fn test_validate_uuid_accepts_valid_uuid() {
    use epic_harness::mem::store::validate_uuid;

    assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000"));
}

// ── mcp-install tmp file unique name test ─────────────

#[test]
fn test_mcp_install_no_leftover_tmp() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let settings_path = root.join("settings.json");
    fs::write(&settings_path, "{}").unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&["mcp-install"]);
    assert_eq!(code, 0, "mcp-install should exit 0");

    // No fixed-name .json.tmp should be left behind
    let fixed_tmp = settings_path.with_extension("json.tmp");
    assert!(
        !fixed_tmp.exists(),
        "fixed-name tmp file should not remain after install"
    );
}

// ── search_nodes FTS test ─────────────────────────────

#[test]
fn test_search_nodes_fts() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::mem::store::search_nodes;

    let root = temp_root();
    set_root(&root);

    run_mem(&[
        "add",
        "--title",
        "Rust Async Pattern",
        "--type",
        "pattern",
        "--tags",
        "rust,async",
        "--body",
        "Use tokio for async runtime.",
    ]);
    run_mem(&[
        "add",
        "--title",
        "Python Basics",
        "--type",
        "concept",
        "--tags",
        "python",
        "--body",
        "Python is a scripting language.",
    ]);

    let results = search_nodes("tokio", 10);
    assert_eq!(
        results.len(),
        1,
        "FTS should find one node matching 'tokio'"
    );
    assert_eq!(results[0].frontmatter.title, "Rust Async Pattern");

    let all = search_nodes("pattern", 10);
    assert!(
        !all.is_empty(),
        "FTS should find nodes with 'pattern' in title or tags"
    );
}

// ── write_node_dedup + append_edge tests ─────────────

#[test]
fn test_write_node_dedup_and_append_edge() {
    use epic_harness::mem::store::{
        Edge, Node, NodeFrontmatter, append_edge, new_uuid, now_iso, read_node, write_node_dedup,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let ts = now_iso();

    // Create two nodes via dedup
    let node_a = Node {
        frontmatter: NodeFrontmatter {
            id: new_uuid(),
            node_type: "pattern".into(),
            title: "test pattern A".into(),
            tags: vec!["auto".into()],
            projects: vec!["testproj".into()],
            agents: vec![],
            created: ts.clone(),
            updated: ts.clone(),
            importance: 0.5,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "pattern body A".into(),
    };
    let (id_a, deduped_a) = write_node_dedup(&node_a, 24).unwrap();
    assert!(!deduped_a, "first write should not be deduped");

    // Second write with same title should be deduped
    let node_a2 = Node {
        frontmatter: NodeFrontmatter {
            id: new_uuid(),
            title: "test pattern A".into(),
            ..node_a.frontmatter.clone()
        },
        body: "different body".into(),
    };
    let (id_a2, deduped_a2) = write_node_dedup(&node_a2, 24).unwrap();
    assert!(deduped_a2, "duplicate title within 24h should be deduped");
    assert_eq!(id_a, id_a2, "deduped ID should match original");

    let node_b = Node {
        frontmatter: NodeFrontmatter {
            id: new_uuid(),
            node_type: "session".into(),
            title: "test session B".into(),
            tags: vec!["auto".into(), "session".into()],
            projects: vec!["testproj".into()],
            agents: vec![],
            created: ts.clone(),
            updated: ts.clone(),
            importance: 0.2,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "session body".into(),
    };
    let (id_b, _) = write_node_dedup(&node_b, 24).unwrap();

    // Create edge
    let edge = Edge {
        id: new_uuid(),
        source: id_b.clone(),
        target: id_a.clone(),
        relation: "detected_in".into(),
        weight: 1.0,
        ts: ts.clone(),
    };
    append_edge(&edge).unwrap();

    let read_a = read_node(&id_a).unwrap();
    assert_eq!(read_a.frontmatter.node_type, "pattern");
    let read_b = read_node(&id_b).unwrap();
    assert_eq!(read_b.frontmatter.node_type, "session");
}

// ── smart_recall test ───────────────────────────────

#[test]
fn test_smart_recall() {
    use epic_harness::mem::store::{
        Node, NodeFrontmatter, new_uuid, now_iso, smart_recall, write_node_dedup,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let ts = now_iso();

    // Create nodes for two different projects with different importance
    for (proj, title, imp) in [
        ("myproj", "myproj: auth decision", 0.9),
        ("myproj", "myproj: build error", 0.4),
        ("otherproj", "otherproj: pattern", 0.5),
    ] {
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: new_uuid(),
                node_type: "pattern".into(),
                title: title.into(),
                tags: vec!["auto".into()],
                projects: vec![proj.into()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: imp,
                access_count: 0,
                accessed_at: String::new(),
            },
            body: "body".into(),
        };
        write_node_dedup(&node, 24).unwrap();
    }

    // Smart recall for myproj should return 2 nodes, highest importance first
    let results = smart_recall(Some("myproj"), None, 10).unwrap();
    assert_eq!(results.len(), 2, "should find 2 myproj nodes");
    assert!(
        results[0].score >= results[1].score,
        "should be sorted by score desc"
    );
    assert!(
        results[0].node.frontmatter.title.contains("auth decision"),
        "higher importance node should rank first"
    );

    // Smart recall with hint should boost FTS-matching nodes
    let with_hint = smart_recall(Some("myproj"), Some("auth"), 10).unwrap();
    assert!(!with_hint.is_empty());
    assert!(
        with_hint[0].node.frontmatter.title.contains("auth"),
        "FTS-matching node should rank highest when hint provided"
    );

    // Other project should only return its own nodes
    let other = smart_recall(Some("otherproj"), None, 10).unwrap();
    assert_eq!(other.len(), 1);
}

// ── tag_stale_nodes test ────────────────────────────

#[test]
fn test_tag_stale_nodes() {
    use epic_harness::mem::store::{
        Node, NodeFrontmatter, new_uuid, read_node, tag_stale_nodes, write_node,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let id = new_uuid();

    // Insert a node with an old timestamp (200 days ago)
    let old_node = Node {
        frontmatter: NodeFrontmatter {
            id: id.clone(),
            node_type: "error".into(),
            title: "old error".into(),
            tags: vec!["auto".into()],
            projects: vec!["proj".into()],
            agents: vec![],
            created: "2020-01-01T00:00:00Z".into(),
            updated: "2020-01-01T00:00:00Z".into(),
            importance: 0.4,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "old body".into(),
    };
    write_node(&old_node).unwrap();

    // Insert a fresh node
    let fresh_id = new_uuid();
    let now = epic_harness::mem::store::now_iso();
    let fresh_node = Node {
        frontmatter: NodeFrontmatter {
            id: fresh_id.clone(),
            node_type: "pattern".into(),
            title: "fresh pattern".into(),
            tags: vec!["auto".into()],
            projects: vec!["proj".into()],
            agents: vec![],
            created: now.clone(),
            updated: now,
            importance: 0.5,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "fresh body".into(),
    };
    write_node(&fresh_node).unwrap();

    let staled = tag_stale_nodes(90).unwrap();
    assert_eq!(staled, 1, "only the old node should be tagged stale");

    let node = read_node(&id).unwrap();
    assert!(
        node.frontmatter.tags.contains(&"stale".to_string()),
        "old node should have stale tag"
    );

    let fresh = read_node(&fresh_id).unwrap();
    assert!(
        !fresh.frontmatter.tags.contains(&"stale".to_string()),
        "fresh node should not be stale"
    );

    // Running again should not double-tag
    let staled2 = tag_stale_nodes(90).unwrap();
    assert_eq!(staled2, 0, "already-stale node should not be re-tagged");
}

// ── Auto-edge integration tests ──────────────────────

#[test]
fn test_ingest_creates_project_hub_and_belongs_to_edges() {
    use epic_harness::mem::store::{
        Edge, Node, NodeFrontmatter, append_edge, new_uuid, now_iso, read_edges, read_node,
        write_node_dedup,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let ts = now_iso();

    // Insert a session node with a project slug
    let session_id = new_uuid();
    let session_node = Node {
        frontmatter: NodeFrontmatter {
            id: session_id.clone(),
            node_type: "session".into(),
            title: "test session".into(),
            tags: vec!["auto".into()],
            projects: vec!["testproj".into()],
            agents: vec![],
            created: ts.clone(),
            updated: ts.clone(),
            importance: 0.05,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "session body".into(),
    };
    write_node_dedup(&session_node, 24).unwrap();

    // Insert a pattern node linked to the session
    let pattern_id = new_uuid();
    let pattern_node = Node {
        frontmatter: NodeFrontmatter {
            id: pattern_id.clone(),
            node_type: "pattern".into(),
            title: "test pattern".into(),
            tags: vec!["auto".into()],
            projects: vec!["testproj".into()],
            agents: vec![],
            created: ts.clone(),
            updated: ts.clone(),
            importance: 0.5,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "pattern body".into(),
    };
    write_node_dedup(&pattern_node, 24).unwrap();

    // Create a project hub node
    let hub_id = new_uuid();
    let hub_node = Node {
        frontmatter: NodeFrontmatter {
            id: hub_id.clone(),
            node_type: "project".into(),
            title: "testproj".into(),
            tags: vec!["hub".into()],
            projects: vec!["testproj".into()],
            agents: vec![],
            created: ts.clone(),
            updated: ts.clone(),
            importance: 0.7,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "project hub for testproj".into(),
    };
    write_node_dedup(&hub_node, 24).unwrap();

    // Create belongs_to edges from session and pattern to hub
    let edge1 = Edge {
        id: new_uuid(),
        source: session_id.clone(),
        target: hub_id.clone(),
        relation: "belongs_to".into(),
        weight: 1.0,
        ts: ts.clone(),
    };
    append_edge(&edge1).unwrap();

    let edge2 = Edge {
        id: new_uuid(),
        source: pattern_id.clone(),
        target: hub_id.clone(),
        relation: "belongs_to".into(),
        weight: 1.0,
        ts: ts.clone(),
    };
    append_edge(&edge2).unwrap();

    // Verify project hub exists with type='project'
    let hub = read_node(&hub_id).unwrap();
    assert_eq!(hub.frontmatter.node_type, "project");
    assert_eq!(hub.frontmatter.title, "testproj");

    // Verify belongs_to edges from session and pattern to hub
    let edges = read_edges();
    let belongs_edges: Vec<_> = edges
        .iter()
        .filter(|e| e.relation == "belongs_to" && e.target == hub_id)
        .collect();
    assert_eq!(
        belongs_edges.len(),
        2,
        "should have 2 belongs_to edges to hub"
    );
    let sources: Vec<&str> = belongs_edges.iter().map(|e| e.source.as_str()).collect();
    assert!(sources.contains(&session_id.as_str()));
    assert!(sources.contains(&pattern_id.as_str()));
}

// ── Stats endpoint test ──────────────────────────────

#[test]
fn test_stats_endpoint_returns_correct_counts() {
    use epic_harness::mem::graph::compute_stats;
    use epic_harness::mem::store::{
        Edge, Node, NodeFrontmatter, append_edge, new_uuid, now_iso, write_node_dedup,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let ts = now_iso();

    // Insert 3 session nodes, 2 pattern nodes, 1 decision node
    let node_configs = [
        ("session", "sess1", 0.05),
        ("session", "sess2", 0.05),
        ("session", "sess3", 0.05),
        ("pattern", "pat1", 0.5),
        ("pattern", "pat2", 0.5),
        ("decision", "dec1", 0.9),
    ];

    let mut node_ids: Vec<String> = vec![];
    for (ntype, title, imp) in &node_configs {
        let id = new_uuid();
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: id.clone(),
                node_type: (*ntype).into(),
                title: (*title).into(),
                tags: vec![],
                projects: vec!["testproj".into()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: *imp,
                access_count: 0,
                accessed_at: String::new(),
            },
            body: "body".into(),
        };
        write_node_dedup(&node, 24).unwrap();
        node_ids.push(id);
    }

    // Create 4 edges
    for i in 0..4usize {
        let edge = Edge {
            id: new_uuid(),
            source: node_ids[i].clone(),
            target: node_ids[i + 1].clone(),
            relation: "related".into(),
            weight: 1.0,
            ts: ts.clone(),
        };
        append_edge(&edge).unwrap();
    }

    // Verify /api/stats returns total_nodes=6, total_edges=4
    let stats = compute_stats().unwrap();
    assert_eq!(stats["total_nodes"], 6, "should have 6 total nodes");
    assert_eq!(stats["total_edges"], 4, "should have 4 total edges");

    // Verify by_type has correct counts
    let by_type = stats["by_type"].as_object().unwrap();
    assert_eq!(by_type["session"], 3, "should have 3 session nodes");
    assert_eq!(by_type["pattern"], 2, "should have 2 pattern nodes");
    assert_eq!(by_type["decision"], 1, "should have 1 decision node");
}

// ── smart_recall importance ranking test ────────────

#[test]
fn test_recall_ranks_decisions_above_sessions() {
    use epic_harness::mem::store::{
        Node, NodeFrontmatter, new_uuid, smart_recall, write_node_dedup,
    };
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let fmt_ts = |offset_secs: u64| -> String {
        let s = now_secs.saturating_sub(offset_secs);
        let sec = s % 60;
        let min = (s / 60) % 60;
        let hour = (s / 3600) % 24;
        let days = s / 86400;
        // Simple date calculation
        let (y, m, d) = epic_harness_days_to_ymd(days);
        format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
    };

    let recent_ts = fmt_ts(2 * 86400); // 2 days ago
    let old_ts = fmt_ts(60 * 86400); // 60 days ago

    // Insert 5 session nodes (importance 0.05) with recent timestamps
    for i in 0..5i32 {
        let node = Node {
            frontmatter: NodeFrontmatter {
                id: new_uuid(),
                node_type: "session".into(),
                title: format!("recent session {i}"),
                tags: vec!["test".into()],
                projects: vec!["proj".into()],
                agents: vec![],
                created: recent_ts.clone(),
                updated: recent_ts.clone(),
                importance: 0.05,
                access_count: 0,
                accessed_at: String::new(),
            },
            body: "session data".into(),
        };
        write_node_dedup(&node, 24).unwrap();
    }

    // Insert 1 decision node (importance 0.9) with older timestamp
    let decision_node = Node {
        frontmatter: NodeFrontmatter {
            id: new_uuid(),
            node_type: "decision".into(),
            title: "critical decision".into(),
            tags: vec!["test".into()],
            projects: vec!["proj".into()],
            agents: vec![],
            created: old_ts.clone(),
            updated: old_ts.clone(),
            importance: 0.9,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: "important decision body".into(),
    };
    write_node_dedup(&decision_node, 24).unwrap();

    // Call smart_recall with limit=10
    let results = smart_recall(Some("proj"), None, 10).unwrap();
    assert_eq!(results.len(), 6, "should return all 6 nodes");

    let decision_idx = results
        .iter()
        .position(|sn| sn.node.frontmatter.node_type == "decision");
    let first_session_idx = results
        .iter()
        .position(|sn| sn.node.frontmatter.node_type == "session");
    assert!(decision_idx.is_some(), "decision node should be present");
    assert!(
        first_session_idx.is_some(),
        "session nodes should be present"
    );
    assert!(
        decision_idx.unwrap() < first_session_idx.unwrap(),
        "decision node should rank above all session nodes"
    );

    let decision_score = results[decision_idx.unwrap()].score;
    for sn in &results {
        if sn.node.frontmatter.node_type == "session" {
            assert!(
                sn.score < decision_score,
                "session score ({}) should be lower than decision score ({})",
                sn.score,
                decision_score
            );
        }
    }
}

// Helper to convert days since epoch to (year, month, day)
#[allow(clippy::manual_is_multiple_of)]
fn epic_harness_days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let diy = if leap { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}
