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
        "path" => {
            println!("{}", hooks::common::harness_dir().display());
            0
        }
        "version" => {
            eprintln!("epic-harness {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("Usage: epic-harness <resume|guard|polish|observe|snapshot|reflect|install|uninstall|path>");
            eprintln!(
                "       epic-harness install [codex|gemini|cursor|opencode|cline|aider] [--local] [--dry-run]"
            );
            eprintln!(
                "       (omit tool name for interactive menu; root-only GEMINI.md only if missing)"
            );
            1
        }
    };

    // Passthrough stdin to stdout (Claude Code hook contract)
    print!("{stdin_buf}");

    std::process::exit(exit_code);
}
