//! Static sanity checks for the shipped plugin assets (issue #131): every
//! Codex and Grok hook handler must route through the shell-free
//! `registry/scripts/hooks/run.mjs` launcher, and the MCP config must launch
//! `epic` directly instead of going through `sh -c`. The files are embedded
//! from their real paths, so a regression fails the build instead of a user
//! session.

use serde_json::Value;

const CODEX_HOOKS: &str = include_str!("../.codex-plugin/hooks.json");
const GROK_HOOKS: &str = include_str!("../.grok-plugin/hooks.json");
const MCP_CONFIG: &str = include_str!("../mcp_config.json");
const RUN_MJS: &str = include_str!("../registry/scripts/hooks/run.mjs");

const LAUNCHER: &str = "node ${PLUGIN_ROOT}/registry/scripts/hooks/run.mjs";
const GROK_LAUNCHER: &str = "node ${GROK_PLUGIN_ROOT}/registry/scripts/hooks/run.mjs";

/// Command handlers, in manifest order: events, matcher groups, handlers.
fn command_handlers(value: &Value) -> Vec<&str> {
    let mut out = Vec::new();
    for groups in value["hooks"].as_object().expect("hooks object").values() {
        for group in groups.as_array().expect("matcher groups") {
            for handler in group["hooks"].as_array().expect("handlers") {
                assert_eq!(handler["type"], "command", "unexpected handler type");
                out.push(handler["command"].as_str().expect("command string"));
            }
        }
    }
    out
}

/// Strings that must never appear in a hook command: they either require a
/// POSIX shell, break under `cmd.exe /C` argv re-escaping (openai/codex#32402),
/// or reintroduce shell sequencing the launcher exists to remove.
const FORBIDDEN: &[&str] = &[
    "\"",
    "$(",
    "test ",
    "command -v",
    ";",
    "&&",
    "||",
    ">",
    "<",
    "|",
];

/// Shared manifest contract: every handler is exactly `<launcher> <sub>` (one
/// trailing word) and carries no shell syntax.
fn assert_launcher_form(handlers: &[&str], launcher: &str) {
    assert!(!handlers.is_empty(), "no command handlers found");
    for (i, cmd) in handlers.iter().enumerate() {
        assert!(
            cmd.starts_with(launcher),
            "handler {i} does not use the launcher: {cmd}"
        );
        assert_eq!(
            cmd.split(launcher).nth(1).map(str::trim),
            Some(cmd.rsplit(' ').next().unwrap()),
            "handler {i}: launcher takes exactly one subcommand word: {cmd}"
        );
        for token in FORBIDDEN {
            assert!(
                !cmd.contains(token),
                "hook command contains {token:?}: {cmd}"
            );
        }
    }
}

#[test]
fn codex_hook_commands_all_use_launcher() {
    let hooks: Value = serde_json::from_str(CODEX_HOOKS).expect("valid JSON");
    let handlers = command_handlers(&hooks);
    assert_eq!(handlers.len(), 10, "expected 10 command handlers");
    assert_launcher_form(&handlers, LAUNCHER);
}

/// The Grok manifest shares the launcher (grok injects `GROK_PLUGIN_ROOT` and
/// expands `${VAR}` in hook commands), so no POSIX inline form survives
/// anywhere and the guard exit code reaches grok unswallowed.
#[test]
fn grok_hook_commands_all_use_launcher() {
    let hooks: Value = serde_json::from_str(GROK_HOOKS).expect("valid JSON");
    let handlers = command_handlers(&hooks);
    assert_eq!(handlers.len(), 7, "expected 7 command handlers");
    assert_launcher_form(&handlers, GROK_LAUNCHER);
}

#[test]
fn mcp_config_launches_epic_without_sh() {
    let config: Value = serde_json::from_str(MCP_CONFIG).expect("valid JSON");
    let server = &config["mcpServers"]["harness-mem"];
    assert_eq!(
        server["command"], "epic",
        "harness-mem must launch via PATH"
    );
    assert_eq!(
        server["args"],
        Value::Array(vec!["mem".into(), "mcp".into()]),
        "harness-mem args must be exactly `mem mcp`"
    );
    assert!(
        !MCP_CONFIG.contains("sh -c") && !MCP_CONFIG.contains("for d in"),
        "mcp_config.json must not shell out"
    );
}

#[test]
fn run_mjs_static_sanity() {
    // The behaviors the hooks depend on, pinned by source text: graceful
    // degradation when `epic` is missing, faithful exit-code propagation, and
    // the SessionStart special case (install.js best-effort, then resume).
    for needle in [
        "session-start",
        "ENOENT",
        "epic not found",
        "stdio: \"inherit\"",
        "process.exit(code ?? 0)",
        "install.js",
        // win32 shell:true degrades through cmd.exe's 9009, not ENOENT.
        "9009",
        "signal",
    ] {
        assert!(RUN_MJS.contains(needle), "run.mjs missing {needle:?}");
    }
    // The launcher must not silently reintroduce a shell on POSIX.
    assert!(
        RUN_MJS.contains("process.platform === \"win32\""),
        "shell opt-in must stay Windows-only"
    );
}
