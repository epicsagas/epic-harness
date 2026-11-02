//! `epic-harness --version` (and aliases) must print to stdout so shell
//! substitution like `$(epic-harness --version)` works; it used to go to
//! stderr and came back empty.

use std::process::{Command, Stdio};

fn run(args: &[&str]) -> (String, String, i32) {
    let harness_dir =
        std::env::temp_dir().join(format!("epic-version-test-{}", std::process::id()));
    std::fs::create_dir_all(&harness_dir).expect("create temp HARNESS_DIR");
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(args)
        .env("HARNESS_DIR", &harness_dir)
        .stdin(Stdio::null())
        .output()
        .expect("spawn epic-harness");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn version_flag_prints_to_stdout_only() {
    let expected = format!("epic-harness {}\n", env!("CARGO_PKG_VERSION"));
    for args in [&["--version"][..], &["-v"], &["version"]] {
        let (stdout, stderr, code) = run(args);
        assert_eq!(code, 0, "{args:?} exit code");
        assert_eq!(stdout, expected, "{args:?} stdout");
        assert!(stderr.is_empty(), "{args:?} wrote to stderr: {stderr}");
    }
}
