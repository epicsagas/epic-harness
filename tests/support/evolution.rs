#![allow(dead_code)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct Fixture {
    _temp: tempfile::TempDir,
    pub home: PathBuf,
    pub data: PathBuf,
    pub work: PathBuf,
    pub project: PathBuf,
}
impl Fixture {
    pub fn new(extra: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let data = temp.path().join("data");
        let work = temp.path().join("issue-project");
        let project = data.join("projects/issue-project");
        for dir in [
            &home,
            &data,
            &work,
            &project,
            &home.join(".config/epic-harness"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(home.join(".config/epic-harness/telemetry-consent"), "off").unwrap();
        std::fs::write(
            data.join("config.toml"),
            format!("[dashboard]\nport=0\nauto_open=false\n{extra}"),
        )
        .unwrap();
        Self {
            _temp: temp,
            home,
            data,
            work,
            project,
        }
    }
    pub fn call(&self, args: &[&str], input: serde_json::Value) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_epic-harness"));
        command
            .args(args)
            .current_dir(&self.work)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("HARNESS_DIR", self.data.canonicalize().unwrap())
            .env(
                "HARNESS_DB_URL",
                format!("sqlite:{}", self.data.join("harness.db").display()),
            )
            .env("HARNESS_ROOT", self._temp.path())
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("APPDATA", self.home.join("AppData/Roaming"))
            .env("LOCALAPPDATA", self.home.join("AppData/Local"))
            .env("EPIC_HOOK_PROFILE", "standard")
            .env("EPIC_ORCHESTRATION", "disabled")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in [
            "HARNESS_MEM_URL",
            "DATABASE_URL",
            "EPIC_AGENT_ID",
            "CLAUDE_MODEL",
            "ANTHROPIC_MODEL",
        ] {
            command.env_remove(key);
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
    pub fn initialize(&self) {
        let output = self.call(&["reflect", "--context"], serde_json::json!({}));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    pub fn sql(&self, statement: &str) {
        use sqlx::Connection;
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let options =
                sqlx::sqlite::SqliteConnectOptions::new().filename(self.data.join("harness.db"));
            let mut connection = sqlx::SqliteConnection::connect_with(&options)
                .await
                .unwrap();
            // All statements are authored by these isolated fixture tests.
            sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                .execute(&mut connection)
                .await
                .unwrap();
        });
    }
    pub fn metric(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.project.join("metrics.json")).unwrap())
            .unwrap()
    }
    pub fn seed_pending(&self, name: &str) -> PathBuf {
        let pending = serde_json::json!({"schema_version":1,"skill_name":name,"origin":"high_freq_error","confidence":0.8,"evidence":{},"template_content":"template","prompt_guidance":"Synthetic evidence only","created":"2026-11-09T00:00:00Z","status":"pending","consumed":null});
        std::fs::write(
            self.project.join("pending_synth.jsonl"),
            format!("{pending}\n"),
        )
        .unwrap();
        let body = self.work.join("body.md");
        std::fs::write(&body, "## Process\nRead the exact synthetic failure and add a focused regression before editing. Rerun that regression and the surrounding tests.\n## Red Flags\nNever claim success without checking the command exit status.\n").unwrap();
        body
    }
}
pub fn context(output: &Output) -> String {
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}
pub fn write(path: impl AsRef<Path>, text: &str) {
    std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
