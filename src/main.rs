mod config;
mod episteme_client;
mod eval;
mod evolve;
mod hooks;
mod install;
mod install_wizard;
mod mem;
mod orchestrate;
mod serve;
mod shared;
mod store;
mod team;
mod telemetry;
mod update;

use std::env;
use std::io::{self, IsTerminal, Read};

/// Parse `--flag <value>` or `--flag=<value>` → Option<u32>
fn parse_flag_u32(args: &[String], flag: &str) -> Option<u32> {
    let eq = format!("{flag}=");
    args.iter()
        .find(|a| a.starts_with(&eq))
        .and_then(|a| a[eq.len()..].parse().ok())
        .or_else(|| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
        })
}

/// Parse `--flag <value>` or `--flag=<value>` → Option<String>
fn parse_flag_str(args: &[String], flag: &str) -> Option<String> {
    let eq = format!("{flag}=");
    args.iter()
        .find(|a| a.starts_with(&eq))
        .map(|a| a[eq.len()..].to_string())
        .or_else(|| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        })
}

/// Parse repeated `--flag <value>` → Vec<String>
fn parse_flag_multi(args: &[String], flag: &str) -> Vec<String> {
    let eq = format!("{flag}=");
    let mut results = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with(&eq) {
            results.push(args[i][eq.len()..].to_string());
        } else if args[i] == flag
            && let Some(val) = args.get(i + 1)
            && !val.starts_with('-')
        {
            results.push(val.clone());
            i += 1;
        }
        i += 1;
    }
    results
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    if subcmd == "--version" || subcmd == "-v" {
        eprintln!("epic-harness {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    // install / telemetry: skip consent check (they set it).
    if subcmd == "install" {
        let code = install::run(&args[2..]);
        std::process::exit(code);
    }
    if subcmd == "uninstall" {
        let code = install::run_uninstall(&args[2..]);
        std::process::exit(code);
    }
    if subcmd == "telemetry" {
        let code = telemetry::run_cli(&args[2..]);
        std::process::exit(code);
    }
    if subcmd == "update" {
        let code = update::run(&args[2..]);
        std::process::exit(code);
    }

    // All other subcommands: auto-enable telemetry on first run (opt-out model).
    // Prints a one-time notice if consent was not yet set.
    telemetry::ensure_consent_or_set_default();

    if subcmd == "mem" {
        let code = mem::run(&args[1..]);
        std::process::exit(code);
    }
    if subcmd == "team" {
        let code = team::run(&args[1..]);
        std::process::exit(code);
    }
    if subcmd == "org" {
        let code = team::run_org(&args[1..]);
        std::process::exit(code);
    }
    if subcmd == "eval" {
        let code = eval::run(&args[2..].to_vec());
        std::process::exit(code);
    }
    if subcmd == "serve" {
        let port = parse_flag_u32(&args, "--port").map(|p| p as u16);
        std::process::exit(serve::run_serve(port));
    }
    if subcmd == "dashboard" {
        let port = parse_flag_u32(&args, "--port").map(|p| p as u16);
        std::process::exit(serve::run_dashboard(port));
    }

    // reflect --context: data collection mode (no stdin needed)
    if subcmd == "reflect" {
        let has_context = args
            .iter()
            .any(|a| a == "--context" || a.starts_with("--context="));
        if has_context {
            // --days <N> or --context=<N> or --context <N>
            let days: u32 = parse_flag_u32(&args, "--days")
                .or_else(|| {
                    args.iter()
                        .find(|a| a.starts_with("--context="))
                        .and_then(|a| a.strip_prefix("--context=").filter(|s| !s.is_empty()))
                        .and_then(|s| s.parse().ok())
                })
                .or_else(|| parse_flag_u32(&args, "--context"))
                .unwrap_or(30);

            // --since <YYYYMMDD>
            let since: Option<String> = parse_flag_str(&args, "--since");

            // --project <slug>
            let project: Option<String> = parse_flag_str(&args, "--project");

            // --all-projects
            let all_projects = args.iter().any(|a| a == "--all-projects");

            // --source <name>  (may appear multiple times)
            let sources: Vec<String> = parse_flag_multi(&args, "--source");

            std::process::exit(hooks::reflect::run_context(
                days,
                since,
                project,
                all_projects,
                sources,
            ));
        }
    }

    // Read stdin once, pass to hook subcommands (skip if TTY — no EOF would arrive)
    let mut stdin_buf = String::new();
    if !io::stdin().is_terminal() {
        let _ = io::stdin().read_to_string(&mut stdin_buf);
    }

    let input: hooks::common::HookInput = if stdin_buf.is_empty() {
        hooks::common::HookInput::default()
    } else {
        serde_json::from_str(&stdin_buf).unwrap_or_default()
    };

    let exit_code = match subcmd {
        "resume" => hooks::resume::run(&input),
        "guard" => hooks::guard::run(&input),
        "polish" => hooks::polish::run(&input),
        "observe" => hooks::observe::run(&input),
        "snapshot" => hooks::snapshot::run(&input),
        "reflect" => hooks::reflect::run(&input),
        "migrate" => {
            let dry_run = args.iter().any(|a| a == "--dry-run");
            let reset = args.iter().any(|a| a == "--reset");
            let to_global = args.iter().any(|a| a == "--to-global");
            if to_global {
                store::migrate::run_to_global(dry_run)
            } else {
                store::migrate::run_subcommand(dry_run, reset)
            }
        }
        "install" | "uninstall" | "mem" | "team" | "org" | "eval" | "telemetry" | "serve" | "dashboard"
        | "update" => {
            unreachable!()
        }
        "path" => {
            println!("{}", hooks::common::harness_dir().display());
            0
        }
        "version" => {
            eprintln!("epic-harness {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            let is_unknown = !matches!(subcmd, "help" | "--help" | "-h");
            if is_unknown {
                eprintln!("error: unknown subcommand '{subcmd}'\n");
            }
            eprintln!(
                "epic-harness {} — Self-evolving agent harness\n",
                env!("CARGO_PKG_VERSION")
            );
            eprintln!("USAGE:");
            eprintln!("  epic-harness <SUBCOMMAND> [OPTIONS]\n");
            eprintln!("HOOK SUBCOMMANDS (invoked automatically by agent hooks):");
            eprintln!("  resume       Restore session context on conversation start");
            eprintln!("  guard        Block/warn on dangerous shell commands");
            eprintln!("  observe      Record tool call observations for pattern analysis");
            eprintln!("  polish       Auto-format and typecheck after file edits");
            eprintln!("  snapshot     Save session state mid-conversation");
            eprintln!("  reflect      Analyze observations and evolve skills (session end)");
            eprintln!(
                "  reflect --context [OPTIONS]  Collect harness data as JSON for /reflect skill"
            );
            eprintln!("    --days <N>           Analysis window in days (default: 30)");
            eprintln!("    --since <YYYYMMDD>   Start date (overrides --days)");
            eprintln!("    --project <slug>     Specific project slug");
            eprintln!("    --all-projects       All projects under ~/.harness/projects/");
            eprintln!(
                "    --source <name>      Extra context source: harness|claude-session|alcove|all (repeatable)\n"
            );
            eprintln!("USER SUBCOMMANDS:");
            eprintln!("  eval         Project quality & regression evaluation  (epic eval --init)");
            eprintln!("    --init             Scaffold eval.yaml config");
            eprintln!("    --json             Output as JSON (for CI)");
            eprintln!("    --baseline-update  Save current results as new baseline");
            eprintln!("    --dimension <dim>  Run specific dimension only");
            eprintln!("  migrate      Import legacy JSONL/JSON data into harness.db");
            eprintln!("    --to-global          Merge per-project harness.db files into global DB");
            eprintln!("    --dry-run            Preview without writing");
            eprintln!("    --reset              Retry interrupted migration");
            eprintln!("  org          Browse org team libraries  (epic org help)");
            eprintln!("  team         Manage org-level agent teams  (epic team help)");
            eprintln!("  mem          Cross-agent unified memory  (harness mem help)");
            eprintln!("  dashboard    Open web dashboard in browser (default port: 7700)");
            eprintln!("  serve        Start dashboard web server without opening browser");
            eprintln!("  install      Install harness into a supported AI tool");
            eprintln!("  uninstall    Remove harness from a supported AI tool");
            eprintln!("  update       Self-update to the latest release");
            eprintln!("  telemetry    Manage telemetry consent  (on|off|status)");
            eprintln!("  path         Print the harness data directory");
            eprintln!("  version      Print version");
            eprintln!("  --version, -v  Print version\n");
            eprintln!("INSTALL TARGETS:  codex  antigravity  cursor  opencode  cline  aider");
            eprintln!("  --local           Install in ./.claude/ instead of ~/.claude/");
            eprintln!("  --target <path>   Override install target directory");
            eprintln!("  --dry-run         Preview without writing\n");
            eprintln!("Run 'epic-harness mem help' for memory subcommand details.");
            if is_unknown { 1 } else { 0 }
        }
    };

    // stdout output varies by host protocol:
    // - Claude Code: stdin passthrough (original contract)
    // - Codex/Antigravity: event-specific JSON (detected via hook_event_name)
    if input.hook_event_name.is_some() {
        // Codex / Antigravity protocol
        match subcmd {
            "guard" if exit_code == 2 => {
                // PreToolUse block: explicit deny JSON so Codex never ignores the block
                println!(
                    r#"{{"hookSpecificOutput":{{"hookEventName":"PreToolUse","permissionDecision":"deny"}}}}"#
                );
            }
            "guard" | "observe" | "polish" => {
                // PreToolUse pass or PostToolUse: plain text ignored, no stdout needed
            }
            "resume" => {
                // SessionStart: plain text on stdout = developer context.
                // hint() already writes to stderr; no extra stdout needed.
            }
            "reflect" => {
                // Stop: MUST output JSON — plain text is invalid for this event.
                println!(r#"{{"continue":true}}"#);
            }
            _ => {}
        }
    } else {
        // Claude Code: stdin passthrough contract
        print!("{stdin_buf}");
    }

    let skip_shutdown = matches!(
        subcmd,
        "help" | "--help" | "-h" | "path" | "version" | "--version" | "-v"
    );
    exit_with_cleanup(exit_code, skip_shutdown);
}

/// Gracefully close connection pools before exit to flush WAL.
///
/// # Safety
/// Must only be called from a non-async context (e.g., `main()`).
/// `store::runtime::block_on` panics if called inside a tokio runtime.
fn exit_with_cleanup(code: i32, skip_shutdown: bool) -> ! {
    if !skip_shutdown {
        // SAFETY: Called only from main() which is not inside a tokio runtime.
        store::runtime::block_on(store::pool::shutdown());
    }
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::{parse_flag_multi, parse_flag_str, parse_flag_u32};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ── parse_flag_u32 ────────────────────────────────
    #[test]
    fn parse_flag_u32_eq_form() {
        let args = s(&["reflect", "--context=7"]);
        assert_eq!(parse_flag_u32(&args, "--context"), Some(7));
    }

    #[test]
    fn parse_flag_u32_space_form() {
        let args = s(&["reflect", "--days", "14"]);
        assert_eq!(parse_flag_u32(&args, "--days"), Some(14));
    }

    #[test]
    fn parse_flag_u32_missing() {
        let args = s(&["reflect", "--context"]);
        assert_eq!(parse_flag_u32(&args, "--days"), None);
    }

    #[test]
    fn parse_flag_u32_invalid_value() {
        let args = s(&["reflect", "--days", "abc"]);
        assert_eq!(parse_flag_u32(&args, "--days"), None);
    }

    #[test]
    fn parse_flag_u32_eq_form_prefers_eq() {
        let args = s(&["reflect", "--days=5", "--days", "99"]);
        assert_eq!(parse_flag_u32(&args, "--days"), Some(5));
    }

    // ── parse_flag_str ────────────────────────────────
    #[test]
    fn parse_flag_str_eq_form() {
        let args = s(&["reflect", "--since=20260101"]);
        assert_eq!(parse_flag_str(&args, "--since"), Some("20260101".into()));
    }

    #[test]
    fn parse_flag_str_space_form() {
        let args = s(&["reflect", "--project", "my-project"]);
        assert_eq!(
            parse_flag_str(&args, "--project"),
            Some("my-project".into())
        );
    }

    #[test]
    fn parse_flag_str_missing() {
        let args = s(&["reflect", "--context"]);
        assert_eq!(parse_flag_str(&args, "--since"), None);
    }

    // ── parse_flag_multi ──────────────────────────────
    #[test]
    fn parse_flag_multi_single_space() {
        let args = s(&["reflect", "--source", "harness"]);
        assert_eq!(
            parse_flag_multi(&args, "--source"),
            vec!["harness".to_string()]
        );
    }

    #[test]
    fn parse_flag_multi_repeated_space() {
        let args = s(&["reflect", "--source", "harness", "--source", "alcove"]);
        assert_eq!(
            parse_flag_multi(&args, "--source"),
            vec!["harness".to_string(), "alcove".to_string()]
        );
    }

    #[test]
    fn parse_flag_multi_eq_form() {
        let args = s(&["reflect", "--source=claude-session"]);
        assert_eq!(
            parse_flag_multi(&args, "--source"),
            vec!["claude-session".to_string()]
        );
    }

    #[test]
    fn parse_flag_multi_empty() {
        let args = s(&["reflect", "--context"]);
        assert_eq!(parse_flag_multi(&args, "--source"), Vec::<String>::new());
    }

    #[test]
    fn parse_flag_multi_skips_next_flag_as_value() {
        let args = s(&["reflect", "--source", "--context", "harness"]);
        // "--context" starts with '-', so it should NOT be treated as a value
        assert_eq!(parse_flag_multi(&args, "--source"), Vec::<String>::new());
    }
}
