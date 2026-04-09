mod hooks;

use std::env;
use std::io::{self, Read};

fn main() {
    let args: Vec<String> = env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    // Read stdin once, pass to subcommand
    let mut stdin_buf = String::new();
    let _ = io::stdin().read_to_string(&mut stdin_buf);

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
        "version" => {
            eprintln!("epic-harness {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("Usage: epic-harness <resume|guard|polish|observe|snapshot|reflect>");
            1
        }
    };

    // Passthrough stdin to stdout (Claude Code hook contract)
    print!("{stdin_buf}");

    std::process::exit(exit_code);
}
