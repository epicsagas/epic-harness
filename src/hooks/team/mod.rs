//! team/mod.rs — Org-level agent team management

pub mod cli;
pub mod store;

pub fn run(args: &[String]) -> i32 {
    // args[0] is "team", args[1..] are subcommand + flags
    let sub_args = if args.len() > 1 { &args[1..] } else { &[] };
    cli::dispatch(sub_args)
}
