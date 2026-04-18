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
