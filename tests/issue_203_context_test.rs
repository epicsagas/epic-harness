#[path = "common/observation.rs"]
mod observation;

use std::process::Command;

fn context(f: &observation::Fixture) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args([
            "reflect",
            "--context",
            "--days",
            "1",
            "--project",
            "fixture-project",
        ])
        .current_dir(&f.cwd)
        .env("HOME", &f.home)
        .env("USERPROFILE", &f.home)
        .env("HARNESS_DIR", f.data.canonicalize().unwrap())
        .env("HARNESS_ROOT", &f.root)
        .env("XDG_CONFIG_HOME", f.home.join(".config"))
        .env(
            "HARNESS_DB_URL",
            format!("sqlite:{}", f.data.join("harness.db").display()),
        )
        .env_remove("HARNESS_MEM_URL")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn fixture() -> observation::Fixture {
    let f = observation::Fixture::new();
    f.observe(serde_json::json!({"tool_name":"Bash","session_id":"context-fixture","tool_input":{"command":"node synthetic.js"},"tool_response":{"output":"ok","exit_code":0}}));
    f
}

fn fallback(f: &observation::Fixture, filename: &str, timestamps: &[&str]) {
    let dir = f.data.join("projects/fixture-project/obs");
    std::fs::create_dir_all(&dir).unwrap();
    let lines = timestamps.iter().map(|timestamp| {
        serde_json::json!({"timestamp":timestamp,"tool":"Read","tool_category":"read","action":"fixture.txt","result":"success","score":1.0}).to_string()
    }).collect::<Vec<_>>().join("\n");
    std::fs::write(dir.join(filename), lines).unwrap();
}

#[test]
fn issue_203_context_reads_current_records_from_uuid_fallback_without_backfill() {
    let f = fixture();
    let filename = "session_12345678-abcd-4000-8000-123456789abc.jsonl";
    let now = epic_harness::shared::helpers::now_iso();
    fallback(&f, filename, &[&now]);
    assert_eq!(context(&f)["obs_stats"]["total"], 2);
    assert!(
        f.data
            .join("projects/fixture-project/obs")
            .join(filename)
            .exists(),
        "context must not require running mutating backfill"
    );
}

#[test]
fn issue_203_context_excludes_old_future_and_malformed_record_dates() {
    let f = fixture();
    fallback(
        &f,
        "session_20990101_fixture.jsonl",
        &["2000-01-01T10:00:00Z", "2999-01-01T10:00:00Z", "😃😃😃"],
    );
    assert_eq!(
        context(&f)["obs_stats"]["total"],
        1,
        "record timestamps determine the analysis window regardless of filename"
    );
}
