//! Headless stdin regression: hook subcommands must exit when stdin is an
//! open pipe that never closes. Bash-tool / unix socket callers hold stdin
//! open with no EOF, and plain `read_to_string` blocked there — `epic-harness
//! reflect` sat at 0% CPU for 15+ minutes inside an orbit run before this
//! was pinned.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Generous: `reflect` with no payload still runs full local analysis.
const WAIT: Duration = Duration::from_secs(30);

#[test]
fn reflect_exits_when_stdin_never_closes() {
    // Isolated HARNESS_DIR: reflect would otherwise run real analysis against
    // the developer's live ~/.harness (DB writes + throttle-marker stamps).
    let harness_dir = std::env::temp_dir().join(format!("epic-stdin-test-{}", std::process::id()));
    std::fs::create_dir_all(&harness_dir).expect("create temp HARNESS_DIR");

    let mut child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .arg("reflect")
        .env("HARNESS_DIR", &harness_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn epic-harness reflect");
    // Hold stdin open — never write, never close. The old code hung right
    // here; the child must time out its read and proceed instead.
    let _stdin = child.stdin.take().expect("stdin piped");

    let start = Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(_) => break,
            None if start.elapsed() > WAIT => {
                let _ = child.kill();
                panic!("reflect still running after {WAIT:?} with stdin held open");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let _ = std::fs::remove_dir_all(&harness_dir);
}
