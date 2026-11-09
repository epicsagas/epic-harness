#[test]
fn native_evolution_exports_have_registered_handlers() {
    let registry = include_str!("../src-tauri/src/lib.rs");
    for command in [
        "get_seesaw_registry",
        "get_variant_pool",
        "get_harness_snapshot",
        "get_adaptation_landscape",
        "get_manifests",
    ] {
        assert!(
            registry.contains(&format!("commands::harness::{command},")),
            "native invoke {command} must have a registered handler"
        );
    }
}
#[path = "../src-tauri/src/commands/native_files.rs"]
mod native_files;

#[test]
fn native_evolution_reads_selected_project_without_mutating_registry() {
    let root = tempfile::tempdir().unwrap();
    for (slug, marker) in [("alpha", 1), ("beta", 2)] {
        let project = root.path().join(slug);
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            project.join("seesaw.json"),
            format!("{{\"solved\":{{\"task\":{marker}}},\"total_solved\":1}}"),
        )
        .unwrap();
    }
    let alpha = root.path().join("alpha");
    let before = std::fs::read(alpha.join("seesaw.json")).unwrap();
    let value = native_files::read_json(&alpha, "seesaw.json", serde_json::json!({})).unwrap();
    assert_eq!(value["solved"]["task"], 1);
    assert_eq!(std::fs::read(alpha.join("seesaw.json")).unwrap(), before);
    assert!(
        native_files::read_json(&alpha, "../beta/seesaw.json", serde_json::Value::Null).is_err()
    );
    assert!(
        include_str!("../src-tauri/src/commands/harness.rs")
            .contains("checked_harness_dir_for_slug")
    );
}

#[test]
fn native_manifest_reader_returns_recent_records_and_preserves_files() {
    let root = tempfile::tempdir().unwrap();
    let text =
        "{\"target\":\"first\"}\nmalformed\n{\"target\":\"second\"}\n{\"target\":\"third\"}\n";
    std::fs::write(root.path().join("manifests.jsonl"), text).unwrap();
    let records = native_files::manifests(root.path(), 2).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["target"], "third");
    assert_eq!(records[1]["target"], "second");
    assert_eq!(
        std::fs::read_to_string(root.path().join("manifests.jsonl")).unwrap(),
        text
    );
}
