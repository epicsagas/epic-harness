//! mem_test.rs — Integration tests for the mem module
//! Uses HARNESS_ROOT env var to redirect ~/.harness to a temp dir.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

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
    epic_harness::hooks::mem::run(&args)
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

    let code = run_mem(&[
        "mcp-install",
        "--path", "/tmp/fake-mem-mcp.cjs",
        "--dry-run",
    ]);
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

    let code = run_mem(&[
        "mcp-install",
        "--path", "/tmp/fake-mem-mcp.cjs",
    ]);
    assert_eq!(code, 0, "mcp-install should exit 0");

    let content = fs::read_to_string(&settings_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    let server = &val["mcpServers"]["harness-mem"];
    assert_eq!(server["command"].as_str().unwrap(), "node");
    assert_eq!(
        server["args"][0].as_str().unwrap(),
        "/tmp/fake-mem-mcp.cjs"
    );
}

#[test]
fn test_mcp_install_already_registered() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let existing = serde_json::json!({
        "mcpServers": {
            "harness-mem": {
                "command": "node",
                "args": ["/old/path/mem-mcp.cjs"]
            }
        }
    });
    let settings_path = root.join("settings.json");
    fs::write(&settings_path, serde_json::to_string(&existing).unwrap()).unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&[
        "mcp-install",
        "--path", "/tmp/fake-mem-mcp.cjs",
    ]);
    assert_eq!(code, 0, "already registered should exit 0");

    // Content should be unchanged (old path preserved)
    let content = fs::read_to_string(&settings_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        val["mcpServers"]["harness-mem"]["args"][0].as_str().unwrap(),
        "/old/path/mem-mcp.cjs",
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
        "--title", "JWT Rotation Pattern",
        "--type", "pattern",
        "--tags", "auth,security",
        "--project", "epic-harness",
        "--agent", "claude-code",
        "--body", "Rotate JWT keys every 24 hours.",
    ]);
    assert_eq!(code, 0, "add should succeed");

    // Verify node file was created
    let nodes_dir = root.join(".harness").join("memory").join("nodes");
    assert!(nodes_dir.exists(), "nodes dir should be created");
    let entries: Vec<_> = fs::read_dir(&nodes_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "exactly one node file should exist");

    // Query by tag
    let idx_path = root.join(".harness").join("memory").join("index.json");
    assert!(idx_path.exists(), "index.json should exist");
    let idx_content = fs::read_to_string(&idx_path).unwrap();
    let idx: serde_json::Value = serde_json::from_str(&idx_content).unwrap();
    let nodes = idx["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["title"].as_str().unwrap(), "JWT Rotation Pattern");
    assert!(nodes[0]["tags"].as_array().unwrap().contains(&serde_json::json!("auth")));
}

#[test]
fn test_link_and_related() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Add two nodes
    run_mem(&[
        "add", "--title", "Node A", "--type", "concept",
        "--tags", "a", "--body", "body a",
    ]);
    run_mem(&[
        "add", "--title", "Node B", "--type", "concept",
        "--tags", "b", "--body", "body b",
    ]);

    // Get their IDs from index
    let idx_content = fs::read_to_string(root.join(".harness/memory/index.json")).unwrap();
    let idx: serde_json::Value = serde_json::from_str(&idx_content).unwrap();
    let nodes = idx["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    let id_a = nodes[0]["id"].as_str().unwrap().to_string();
    let id_b = nodes[1]["id"].as_str().unwrap().to_string();

    // Link A -> B
    let code = run_mem(&["link", &id_a, &id_b, "--relation", "uses"]);
    assert_eq!(code, 0, "link should succeed");

    // Verify edges file
    let edges_path = root.join(".harness/memory/edges.jsonl");
    assert!(edges_path.exists(), "edges.jsonl should exist");
    let edges_content = fs::read_to_string(&edges_path).unwrap();
    assert!(edges_content.contains("uses"), "edge should have 'uses' relation");

    // related from A should return B
    let related = epic_harness::hooks::mem::graph::related_nodes(&id_a, 2);
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

    // No nodes should have been written
    let nodes_dir = root.join(".harness/memory/nodes");
    let count = if nodes_dir.exists() {
        fs::read_dir(&nodes_dir).unwrap().count()
    } else {
        0
    };
    assert_eq!(count, 0, "dry-run should not write any node files");
}

#[test]
fn test_validate() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    // Add a valid node
    run_mem(&[
        "add", "--title", "Valid Node", "--type", "concept",
        "--tags", "test", "--body", "valid body",
    ]);

    // Inject a corrupt node file
    let nodes_dir = root.join(".harness/memory/nodes");
    fs::write(nodes_dir.join("corrupt.md"), "not valid frontmatter at all").unwrap();

    // validate should exit 1 and report the corrupt file
    let code = run_mem(&["validate"]);
    assert_eq!(code, 1, "validate should fail when corrupt file exists");
}

// ── Fix 1: Edge lock tests ─────────────────────────────

#[test]
fn test_delete_edge_by_id_is_consistent() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::hooks::mem::store::{append_edge, delete_edge_by_id, read_edges, Edge};

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
    use epic_harness::hooks::mem::store::{append_edge, remove_edges_for_node, read_edges, Edge};

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

// ── Fix 2: validate_node_id + safe_node_path tests ────

#[test]
fn test_validate_node_id_valid() {
    use epic_harness::hooks::mem::store::validate_node_id;

    // Valid UUID v4
    assert!(validate_node_id("550e8400-e29b-41d4-a716-446655440000"));
    assert!(validate_node_id("00000000-0000-4000-8000-000000000000"));
}

#[test]
fn test_validate_node_id_invalid() {
    use epic_harness::hooks::mem::store::validate_node_id;

    assert!(!validate_node_id("../etc/passwd"));
    assert!(!validate_node_id("../../secret"));
    assert!(!validate_node_id("short"));
    assert!(!validate_node_id("550e8400-e29b-41d4-a716-4466554400000")); // 37 chars
    assert!(!validate_node_id("550e8400/e29b/41d4/a716/446655440000")); // slashes
}

#[test]
fn test_safe_node_path_rejects_traversal() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::hooks::mem::store::safe_node_path;

    let root = temp_root();
    set_root(&root);

    assert!(safe_node_path("../etc/passwd").is_none());
    assert!(safe_node_path("../../secret").is_none());
    assert!(safe_node_path("short").is_none());
}

#[test]
fn test_safe_node_path_accepts_valid_uuid() {
    let _guard = ENV_LOCK.lock().unwrap();
    use epic_harness::hooks::mem::store::safe_node_path;

    let root = temp_root();
    set_root(&root);

    let result = safe_node_path("550e8400-e29b-41d4-a716-446655440000");
    assert!(result.is_some());
    let p = result.unwrap();
    assert!(p.to_string_lossy().ends_with("550e8400-e29b-41d4-a716-446655440000.md"));
}

// ── Fix 4: mcp-install tmp file unique name test ───────

#[test]
fn test_mcp_install_no_leftover_tmp() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root();
    set_root(&root);

    let settings_path = root.join("settings.json");
    fs::write(&settings_path, "{}").unwrap();
    set_claude_settings(&settings_path);

    let code = run_mem(&[
        "mcp-install",
        "--path", "/tmp/fake-mem-mcp.cjs",
    ]);
    assert_eq!(code, 0, "mcp-install should exit 0");

    // No fixed-name .json.tmp should be left behind
    let fixed_tmp = settings_path.with_extension("json.tmp");
    assert!(
        !fixed_tmp.exists(),
        "fixed-name tmp file should not remain after install"
    );
}
