use std::fs;

fn main() {
    const REVISION_FILE: &str = "runtime-revision.txt";

    println!("cargo:rerun-if-changed={REVISION_FILE}");
    let raw = fs::read_to_string(REVISION_FILE)
        .unwrap_or_else(|error| panic!("cannot read {REVISION_FILE}: {error}"));
    let revision = raw.trim();
    assert!(
        !revision.is_empty() && revision.bytes().all(|byte| byte.is_ascii_digit()),
        "{REVISION_FILE} must contain one positive integer"
    );
    let parsed = revision
        .parse::<u64>()
        .unwrap_or_else(|error| panic!("invalid {REVISION_FILE}: {error}"));
    assert!(parsed > 0, "{REVISION_FILE} must be greater than zero");

    println!("cargo:rustc-env=EPIC_HARNESS_RUNTIME_REVISION={parsed}");
}
