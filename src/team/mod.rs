//! team/mod.rs — Org-level agent team management

pub mod cli;
pub mod codex;
pub mod store;

/// Process-wide mutex that serializes tests mutating the HOME environment variable.
/// Both `cli::tests` and `store::tests` must hold this lock while changing HOME so
/// that concurrent test threads do not see each other's temporary directories.
#[cfg(test)]
pub(crate) static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn run(args: &[String]) -> i32 {
    // args[0] is "team", args[1..] are subcommand + flags
    let sub_args = if args.len() > 1 { &args[1..] } else { &[] };
    cli::dispatch(sub_args)
}

pub fn run_org(args: &[String]) -> i32 {
    // args[0] is "org", args[1..] are subcommand + flags
    let sub_args = if args.len() > 1 { &args[1..] } else { &[] };
    cli::dispatch_org(sub_args)
}
