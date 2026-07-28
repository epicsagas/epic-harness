//! Plugin manifest layout invariants.
//!
//! Both bugs these tests lock down were silent: the plugin loaded cleanly, the
//! host listed the hook events, and nothing fired. They are cheap to re-break
//! by moving a file, so they are asserted rather than documented.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} must be valid JSON: {e}", path.display()))
}

/// Every `"command"` string in a hooks manifest.
fn hook_commands(manifest: &serde_json::Value) -> Vec<String> {
    let mut out = vec![];
    let Some(events) = manifest.get("hooks").and_then(|h| h.as_object()) else {
        return out;
    };
    for entries in events.values() {
        for entry in entries.as_array().into_iter().flatten() {
            for hook in entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .into_iter()
                .flatten()
            {
                if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                    out.push(cmd.to_string());
                }
            }
        }
    }
    out
}

/// Claude Code auto-discovers plugin hooks from `<root>/hooks/hooks.json` and
/// from nowhere else — a manifest under `.claude-plugin/` is never read, so
/// hooks defined there never run.
#[test]
fn claude_hooks_live_at_the_auto_discovered_path() {
    let root = repo_root();
    assert!(
        root.join("hooks/hooks.json").is_file(),
        "hooks/hooks.json must exist — it is the only path Claude Code loads plugin hooks from"
    );
    assert!(
        !root.join(".claude-plugin/hooks.json").exists(),
        ".claude-plugin/hooks.json is never read by Claude Code; a manifest here \
         silently disables every hook it defines"
    );

    let manifest = read_json(&root.join("hooks/hooks.json"));
    for event in [
        "SessionStart",
        "PreToolUse",
        "PostToolUse",
        "PreCompact",
        "SessionEnd",
    ] {
        assert!(
            manifest.get("hooks").and_then(|h| h.get(event)).is_some(),
            "hooks/hooks.json must register {event}"
        );
    }
}

/// `package.json` declares `"type": "module"`, so a `.js` bootstrap must be real
/// ESM. A CommonJS one throws `require is not defined in ES module scope` before
/// it installs anything — and since every hook is dispatched through this script,
/// that single failure takes the whole harness down rather than one hook.
#[test]
fn bootstrap_script_loads_under_the_declared_module_type() {
    let root = repo_root();
    let pkg = read_json(&root.join("package.json"));
    let esm = pkg.get("type").and_then(|t| t.as_str()) == Some("module");

    let js = root.join("registry/scripts/install.js");
    let cjs = root.join("registry/scripts/install.cjs");
    assert!(
        js.is_file() || cjs.is_file(),
        "the SessionStart bootstrap must exist — every hook is dispatched through it"
    );

    if esm && js.is_file() {
        let source = std::fs::read_to_string(&js).unwrap();
        for line in source.lines() {
            let line = line.trim_start();
            assert!(
                !line.starts_with("const ") || !line.contains("= require("),
                "install.js uses require() under \"type\": \"module\" — Node refuses \
                 to load it. Port it to import, or rename the file to .cjs.\n  {line}"
            );
        }
    }
}

/// Whatever the manifests invoke has to exist on disk. A stale path here is
/// invisible at load time and only shows up as a hook that quietly does nothing.
#[test]
fn manifest_commands_reference_files_that_exist() {
    let root = repo_root();
    let manifests = [
        root.join("hooks/hooks.json"),
        root.join(".codex-plugin/hooks.json"),
    ];

    for manifest_path in manifests {
        for command in hook_commands(&read_json(&manifest_path)) {
            for token in command.split_whitespace() {
                let Some((_, tail)) = token.split_once("registry/scripts/") else {
                    continue;
                };
                let script = tail.trim_end_matches(['"', ';', '\'']);
                assert!(
                    root.join("registry/scripts").join(script).is_file(),
                    "{} references registry/scripts/{script}, which does not exist",
                    manifest_path.display()
                );
            }
        }
    }
}
