//! Headless stdin regression: subcommands that read stdin must exit when
//! stdin is an open pipe that never closes. Bash-tool / unix socket callers
//! hold stdin open with no EOF, and plain `read_to_string` blocked there —
//! `epic-harness reflect` sat at 0% CPU for 15+ minutes inside an orbit run
//! before this was pinned. Covers both read sites: the hook dispatch in
//! main.rs and `evolve accept-synth` in evolve/cli.rs.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Generous: `reflect` with no payload still runs full local analysis.
const WAIT: Duration = Duration::from_secs(30);

/// Run a subcommand with stdin piped but never written or closed, and
/// assert it exits on its own within `WAIT`. Returns the exit status.
/// `HARNESS_DIR` is isolated so the child never touches the developer's
/// live harness state (DB writes + throttle-marker stamps).
fn run_with_stdin_held_open(tag: &str, subcmd: &str, rest: &[&str]) -> std::process::ExitStatus {
    let harness_dir: PathBuf =
        std::env::temp_dir().join(format!("epic-stdin-test-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&harness_dir).expect("create temp HARNESS_DIR");

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg(subcmd)
        .args(rest)
        .env("HARNESS_DIR", &harness_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn epic-harness");

    // Hold stdin open — never write, never close. The old code hung right
    // here; the child must time out its read and proceed instead.
    let _stdin = child.stdin.take().expect("stdin piped");

    let start = Instant::now();
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break status,
            None if start.elapsed() > WAIT => {
                let _ = child.kill();
                let _ = child.wait(); // reap — no zombie left for the CI runner
                let _ = std::fs::remove_dir_all(&harness_dir);
                panic!("{subcmd} still running after {WAIT:?} with stdin held open");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let _ = std::fs::remove_dir_all(&harness_dir);
    status
}

#[test]
fn reflect_exits_when_stdin_never_closes() {
    let status = run_with_stdin_held_open("reflect", "reflect", &[]);
    // Clean exit expected: with no payload, reflect runs local analysis on
    // an empty HARNESS_DIR. A crash (nonzero) must fail the test too.
    assert!(status.success(), "reflect exited with {status}");
}

#[test]
fn accept_synth_exits_when_stdin_never_closes() {
    // Same bug class in evolve accept-synth: `--stdin` piped but never
    // written must time out (EXIT_IO = 6), not hang forever.
    let status = run_with_stdin_held_open(
        "accept-synth",
        "evolve",
        &["accept-synth", "--skill", "evo-stdin-regression", "--stdin"],
    );
    assert_eq!(status.code(), Some(6), "accept-synth exited with {status}");
}
