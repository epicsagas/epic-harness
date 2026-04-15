//! mem/mod.rs — Cross-Agent Unified Memory System

pub mod cli;
pub mod graph;
pub mod mcp;
pub mod server;
pub mod store;

pub fn run(args: &[String]) -> i32 {
    // args[0] is "mem", args[1..] are subcommand + flags
    let sub_args = if args.len() > 1 { &args[1..] } else { &[] };
    cli::dispatch(sub_args)
}

/// Process-wide lock for serialising `HARNESS_ROOT` env-var mutations across
/// all test modules in this crate. A single static is required because each
/// module's own `ENV_LOCK` only serialises within that module — concurrent
/// tests from `graph` and `store` would race on the shared env var without this.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
