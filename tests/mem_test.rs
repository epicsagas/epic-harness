/// mem_test.rs — Integration tests for the mem module
/// Uses HARNESS_ROOT env var to redirect ~/.harness to a temp dir.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = env::temp_dir().join(format!("epic_harness_mem_test_{n}_{}", std::process::id()));
    fs::create_dir_all(&base).unwrap();
    base
}

fn set_root(root: &PathBuf) {
    // HARNESS_ROOT overrides HOME in the store module
    // SAFETY: tests run serially (cargo test -- --test-threads=1) and
    // no other thread reads env vars concurrently in this test binary.
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

// ── Tests ─────────────────────────────────────────────

#[test]
fn test_add_and_query() {
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
