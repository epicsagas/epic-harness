mod hooks;

use std::env;
use std::io::{self, IsTerminal, Read};

fn main() {
    let args: Vec<String> = env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    // install/uninstall read stdin themselves (interactive menu) — skip pre-reading.
    if subcmd == "install" {
        let code = hooks::install::run(&args[2..]);
        std::process::exit(code);
    }
    if subcmd == "uninstall" {
        let code = hooks::install::run_uninstall(&args[2..]);
        std::process::exit(code);
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
        "install" => unreachable!(),
        "mem" => hooks::mem::run(&args[1..]),
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
            eprintln!("epic-harness {} — Self-evolving agent harness for Claude Code\n", env!("CARGO_PKG_VERSION"));
            eprintln!("USAGE:");
            eprintln!("  epic-harness <SUBCOMMAND> [OPTIONS]\n");
            eprintln!("HOOK SUBCOMMANDS (invoked automatically by Claude Code hooks):");
            eprintln!("  resume       Restore session context on conversation start");
            eprintln!("  guard        Block/warn on dangerous shell commands");
            eprintln!("  observe      Record tool call observations for pattern analysis");
            eprintln!("  polish       Auto-format and typecheck after file edits");
            eprintln!("  snapshot     Save session state mid-conversation");
            eprintln!("  reflect      Analyze observations and evolve skills (session end)\n");
            eprintln!("USER SUBCOMMANDS:");
            eprintln!("  mem          Cross-agent unified memory  (harness mem help)");
            eprintln!("  install      Install harness into a supported AI tool");
            eprintln!("  uninstall    Remove harness from a supported AI tool");
            eprintln!("  path         Print the harness data directory");
            eprintln!("  version      Print version\n");
            eprintln!("INSTALL TARGETS:  codex  gemini  cursor  opencode  cline  aider");
            eprintln!("  --local       Install in ./.claude/ instead of ~/.claude/");
            eprintln!("  --dry-run     Preview without writing\n");
            eprintln!("Run 'epic-harness mem help' for memory subcommand details.");
            if is_unknown { 1 } else { 0 }
        }
    };

    // Passthrough stdin to stdout (Claude Code hook contract)
    print!("{stdin_buf}");

    std::process::exit(exit_code);
}
