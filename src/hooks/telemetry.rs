// Public API surface — most methods are called by future command integrations.
#![allow(dead_code)]

/// epic-harness telemetry — PostHog (product analytics) + Sentry (error monitoring)
///
/// Architecture decision: epiccounty/reports/telemetry_sentry_posthog_architecture_2026-04-29.md
///   - PostHog: single project, `product` property for per-product breakdown
///   - Sentry:  per-product project (SENTRY_DSN_EPIC_HARNESS)
///   - Consent: opt-out (on by default), on/off only
///   - PII: strictly forbidden — enum values only, no free strings
///
/// Consent flow:
///   - New users:     install wizard sets consent explicitly
///   - Existing users / hook-less agents: first binary invocation auto-enables
///     telemetry and prints a one-time opt-out notice
///
/// Implementation uses std-only HTTP (no extra crates) to keep binary lean.
use std::fs;
use std::io::Read;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

// ── Enum-gated values (no free strings allowed) ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Work,
    Ship,
    Review,
    Plan,
    Test,
    Audit,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Ship => "ship",
            Self::Review => "review",
            Self::Plan => "plan",
            Self::Test => "test",
            Self::Audit => "audit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Success,
    Error,
    Abort,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Abort => "abort",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FailureClass {
    UserAbort,
    ClaudeApiError,
    ToolPermissionDenied,
    HookFailed,
    GitConflict,
    TimeoutExceeded,
    Unknown,
}

impl FailureClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::UserAbort => "user_abort",
            Self::ClaudeApiError => "claude_api_error",
            Self::ToolPermissionDenied => "tool_permission_denied",
            Self::HookFailed => "hook_failed",
            Self::GitConflict => "git_conflict",
            Self::TimeoutExceeded => "timeout_exceeded",
            Self::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for FailureClass {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "user_abort" => Self::UserAbort,
            "claude_api_error" => Self::ClaudeApiError,
            "tool_permission_denied" => Self::ToolPermissionDenied,
            "hook_failed" => Self::HookFailed,
            "git_conflict" => Self::GitConflict,
            "timeout_exceeded" => Self::TimeoutExceeded,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RuleKind {
    Builtin,
    ConventionalCommit,
    Custom,
}

impl RuleKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::ConventionalCommit => "conventional_commit",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FormatterKind {
    Tsc,
    Biome,
    Prettier,
    Ruff,
    Gofmt,
}

impl FormatterKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tsc => "tsc",
            Self::Biome => "biome",
            Self::Prettier => "prettier",
            Self::Ruff => "ruff",
            Self::Gofmt => "gofmt",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ToolCategory {
    Bash,
    Edit,
    Write,
    Read,
    Glob,
    Grep,
    Other,
}

impl ToolCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Edit => "edit",
            Self::Write => "write",
            Self::Read => "read",
            Self::Glob => "glob",
            Self::Grep => "grep",
            Self::Other => "other",
        }
    }
}

impl std::str::FromStr for ToolCategory {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bash" => Self::Bash,
            "edit" => Self::Edit,
            "write" => Self::Write,
            "read" => Self::Read,
            "glob" => Self::Glob,
            "grep" => Self::Grep,
            _ => Self::Other,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SessionTrend {
    Improving,
    Stable,
    Declining,
}

impl SessionTrend {
    fn as_str(self) -> &'static str {
        match self {
            Self::Improving => "improving",
            Self::Stable => "stable",
            Self::Declining => "declining",
        }
    }
}

impl std::str::FromStr for SessionTrend {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "improving" => Self::Improving,
            "declining" => Self::Declining,
            _ => Self::Stable,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokensBucket {
    Lt1k,
    Lt10k,
    Lt100k,
    Lt500k,
    Gte500k,
}

impl TokensBucket {
    pub fn from_count(n: u64) -> Self {
        match n {
            0..=999 => Self::Lt1k,
            1_000..=9_999 => Self::Lt10k,
            10_000..=99_999 => Self::Lt100k,
            100_000..=499_999 => Self::Lt500k,
            _ => Self::Gte500k,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lt1k => "<1k",
            Self::Lt10k => "<10k",
            Self::Lt100k => "<100k",
            Self::Lt500k => "<500k",
            Self::Gte500k => ">=500k",
        }
    }
}

// ── Consent ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConsentLevel {
    On,
    Off,
}

fn consent_file() -> PathBuf {
    dirs_config().join("epic-harness").join("telemetry-consent")
}

fn install_id_file() -> PathBuf {
    dirs_config().join("epic-harness").join("install-id")
}

fn dirs_config() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h).join(".config");
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        return PathBuf::from(up).join(".config");
    }
    PathBuf::from(".config")
}

/// Returns None when no consent file exists yet (unset state).
pub fn read_consent_raw() -> Option<ConsentLevel> {
    let s = fs::read_to_string(consent_file()).ok()?;
    match s.trim() {
        "off" => Some(ConsentLevel::Off),
        // legacy values from the 3-level system → treat as On
        "on" | "community" | "anonymous" => Some(ConsentLevel::On),
        _ => None,
    }
}

pub fn read_consent() -> ConsentLevel {
    read_consent_raw().unwrap_or(ConsentLevel::Off)
}

pub fn write_consent(level: ConsentLevel) {
    let path = consent_file();
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let val = match level {
        ConsentLevel::On => "on",
        ConsentLevel::Off => "off",
    };
    let _ = fs::write(&path, val);
}

/// Called at every binary entry point (except `install` / `telemetry`).
///
/// If consent has never been set, automatically enables telemetry and prints a
/// one-time opt-out notice to stderr. This covers existing users who won't run
/// `install` again, and hook-less agents (Codex, Gemini CLI, etc.).
pub fn ensure_consent_or_set_default() {
    if read_consent_raw().is_none() {
        write_consent(ConsentLevel::On);
        eprintln!("[harness] Anonymous telemetry is enabled by default.");
        eprintln!("[harness] No personally identifiable information is collected.");
        eprintln!("[harness] To opt out: epic-harness telemetry off");
        eprintln!("[harness] Details: https://github.com/epicsagas/epic-harness#telemetry");
    }
}

fn is_valid_uuid(s: &str) -> bool {
    // xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    let mut parts = s.splitn(6, '-');
    let p0 = parts.next().unwrap_or("");
    let p1 = parts.next().unwrap_or("");
    let p2 = parts.next().unwrap_or("");
    let p3 = parts.next().unwrap_or("");
    let p4 = parts.next().unwrap_or("");
    parts.next().is_none()
        && p0.len() == 8
        && p1.len() == 4
        && p2.len() == 4
        && p3.len() == 4
        && p4.len() == 12
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && p2.starts_with('4')
}

fn load_or_create_install_id() -> String {
    let path = install_id_file();
    if let Ok(s) = fs::read_to_string(&path) {
        let trimmed = s.trim().to_string();
        if is_valid_uuid(&trimmed) {
            return trimmed;
        }
    }
    let id = new_uuid_v4();
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(&path, &id);
    id
}

fn new_uuid_v4() -> String {
    // Read 16 bytes from /dev/urandom for UUID v4
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut bytes);
    } else {
        // Fallback: mix pid + timestamp bytes
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid_bytes = pid.to_le_bytes();
        let ts_bytes = ts.to_le_bytes();
        for (i, b) in pid_bytes.iter().enumerate() {
            bytes[i] ^= b;
        }
        for (i, b) in ts_bytes.iter().enumerate() {
            bytes[8 + i % 8] ^= b;
        }
    }
    // Set version 4 and variant bits
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

// ── Install wizard prompt (TTY) ───────────────────────────────────────────────

/// Prints the telemetry explanation and asks for on/off.
/// Returns the chosen level. Called from the install wizard (TTY context).
pub fn prompt_consent_interactive() -> ConsentLevel {
    use std::io::Write;
    eprintln!();
    eprintln!("  ┌─ Telemetry ──────────────────────────────────────────────────────────┐");
    eprintln!("  │ epic-harness collects anonymous usage data to improve the product.   │");
    eprintln!("  │                                                                      │");
    eprintln!("  │  What we send:    command name, duration, outcome, version, OS       │");
    eprintln!("  │  What we never:   code, file paths, repo names, prompts, PII         │");
    eprintln!("  │  Identifier:      random install ID (not linked to you or machine)   │");
    eprintln!("  │  Opt out anytime: epic-harness telemetry off                         │");
    eprintln!("  └──────────────────────────────────────────────────────────────────────┘");
    eprintln!();
    eprint!("  Enable telemetry? [Y/n]: ");
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    match line.trim().to_lowercase().as_str() {
        "n" | "no" => ConsentLevel::Off,
        _ => ConsentLevel::On,
    }
}

// ── Telemetry client ─────────────────────────────────────────────────────────

pub struct Telemetry {
    consent: ConsentLevel,
    distinct_id: String,
    base_props: String,
}

impl Telemetry {
    pub fn init() -> Self {
        ensure_consent_or_set_default();
        let consent = read_consent();
        let distinct_id = match consent {
            ConsentLevel::On => load_or_create_install_id(),
            ConsentLevel::Off => String::new(),
        };
        let base_props = format!(
            r#""product":"epic-harness","product_version":"{}","os":"{}","telemetry_schema":"v1""#,
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
        );
        Self {
            consent,
            distinct_id,
            base_props,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.consent == ConsentLevel::On
    }

    /// Track a PostHog event. `extra` must be pre-serialised JSON key-value pairs
    /// using only enum-gated values — no free strings, no user input.
    pub(crate) fn track(&self, event: &str, extra: &str) {
        if !self.is_enabled() {
            return;
        }
        let event_escaped = json_escape(event);
        let distinct_id_escaped = json_escape(&self.distinct_id);
        let payload = if extra.is_empty() {
            format!(
                r#"{{"event":"{}","distinct_id":"{}","properties":{{{}}}}}"#,
                event_escaped, distinct_id_escaped, self.base_props,
            )
        } else {
            format!(
                r#"{{"event":"{}","distinct_id":"{}","properties":{{{},{}}}}}"#,
                event_escaped, distinct_id_escaped, self.base_props, extra,
            )
        };
        posthog_send(&payload);
    }

    /// Capture an error to Sentry. `message` must be a pre-classified enum string (no free text).
    pub fn capture_error(
        &self,
        message: &str,
        failure_class: FailureClass,
        command: Option<Command>,
    ) {
        if !self.is_enabled() {
            return;
        }
        let cmd_tag = command.map(|c| c.as_str()).unwrap_or("none");
        sentry_send(
            message,
            failure_class.as_str(),
            cmd_tag,
            env!("CARGO_PKG_VERSION"),
        );
    }
}

// ── Typed event helpers ───────────────────────────────────────────────────────

impl Telemetry {
    pub fn track_session_started(&self) {
        self.track("epic_session_started", "");
    }

    pub fn track_command_invoked(&self, command: Command) {
        self.track(
            "command_invoked",
            &format!(r#""command":"{}""#, command.as_str()),
        );
    }

    pub fn track_command_completed(
        &self,
        command: Command,
        duration_ms: u64,
        outcome: Outcome,
        tokens_bucket: Option<TokensBucket>,
    ) {
        let tokens_str = tokens_bucket
            .map(|b| format!(r#","tokens_used_bucket":"{}""#, b.as_str()))
            .unwrap_or_default();
        self.track(
            "command_completed",
            &format!(
                r#""command":"{}","duration_ms":{},"outcome":"{}"{}"#,
                command.as_str(),
                duration_ms,
                outcome.as_str(),
                tokens_str,
            ),
        );
    }

    pub fn track_command_failed(
        &self,
        command: Command,
        duration_ms: u64,
        failure_class: FailureClass,
    ) {
        self.track(
            "command_failed",
            &format!(
                r#""command":"{}","duration_ms":{},"failure_class":"{}""#,
                command.as_str(),
                duration_ms,
                failure_class.as_str(),
            ),
        );
        // Mirror to Sentry only for server/tool errors (not user-driven)
        match failure_class {
            FailureClass::UserAbort | FailureClass::GitConflict => {}
            _ => self.capture_error("command_failed", failure_class, Some(command)),
        }
    }

    pub fn track_hook_failed(&self, command: Option<Command>) {
        let cmd_str = command
            .map(|c| format!(r#","command":"{}""#, c.as_str()))
            .unwrap_or_default();
        self.track(
            "command_failed",
            &format!(r#""failure_class":"hook_failed"{}"#, cmd_str),
        );
        self.capture_error("hook_failed", FailureClass::HookFailed, command);
    }

    /// guard hook: a Bash command was blocked.
    pub fn track_hook_blocked(&self, rule: RuleKind) {
        self.track("hook_blocked", &format!(r#""rule":"{}""#, rule.as_str()));
    }

    /// guard hook: a Bash command triggered a warning.
    pub fn track_hook_warned(&self, rule: RuleKind) {
        self.track("hook_warned", &format!(r#""rule":"{}""#, rule.as_str()));
    }

    /// polish hook: formatter or type-checker failed.
    pub fn track_polish_failed(&self, formatter: FormatterKind) {
        self.track(
            "polish_failed",
            &format!(r#""formatter":"{}""#, formatter.as_str()),
        );
        self.capture_error("polish_failed", FailureClass::HookFailed, None);
    }

    /// observe hook: a single tool call ended in error (sampled — only when failure_class is set).
    pub fn track_tool_error(&self, tool_category: ToolCategory, failure_class: FailureClass) {
        self.track(
            "tool_error",
            &format!(
                r#""tool_category":"{}","failure_class":"{}""#,
                tool_category.as_str(),
                failure_class.as_str(),
            ),
        );
    }

    /// reflect hook (SessionEnd): aggregate session metrics.
    pub fn track_session_ended(
        &self,
        success_rate: f64,
        avg_score: f64,
        total_observations: u64,
        trend: SessionTrend,
        skills_seeded: u64,
    ) {
        self.track(
            "session_ended",
            &format!(
                r#""success_rate_pct":{},"avg_score":{},"observations":{},"trend":"{}","skills_seeded":{}"#,
                (success_rate * 100.0).round() as u32,
                avg_score,
                total_observations,
                trend.as_str(),
                skills_seeded,
            ),
        );
    }
}

// ── PostHog transport (std TcpStream, no external crates) ────────────────────

const POSTHOG_HOST: &str = "us.i.posthog.com";
const POSTHOG_PORT: u16 = 443;

fn posthog_key() -> Option<&'static str> {
    option_env!("POSTHOG_KEY").filter(|k| !k.is_empty())
}

fn posthog_send(payload: &str) {
    let Some(key) = posthog_key() else { return };

    let body = format!(
        r#"{{"api_key":"{}","batch":[{}]}}"#,
        json_escape(key),
        payload
    );
    http_post_tls(POSTHOG_HOST, POSTHOG_PORT, "/batch/", &body);
}

// ── Sentry transport ─────────────────────────────────────────────────────────

fn sentry_dsn() -> Option<&'static str> {
    option_env!("SENTRY_DSN_EPIC_HARNESS").filter(|k| !k.is_empty())
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn sentry_send(message: &str, failure_class: &str, command: &str, version: &str) {
    let Some(dsn) = sentry_dsn() else { return };

    // Parse DSN: https://<key>@<host>/<project_id>
    let (host, path) = parse_sentry_dsn(dsn).unwrap_or_default();
    if host.is_empty() {
        return;
    }

    let event_id = new_uuid_v4().replace('-', "");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let message = json_escape(message);
    let failure_class = json_escape(failure_class);
    let command = json_escape(command);
    let version_escaped = json_escape(version);
    let envelope = format!(
        "{{}}\n{{\"type\":\"event\"}}\n{{\
            \"event_id\":\"{event_id}\",\
            \"timestamp\":{ts},\
            \"level\":\"error\",\
            \"message\":\"{message}\",\
            \"release\":\"{version_escaped}\",\
            \"tags\":{{\
                \"failure_class\":\"{failure_class}\",\
                \"command\":\"{command}\"\
            }}\
        }}\n"
    );

    // Extract key for Auth header
    let key = dsn
        .split('@')
        .next()
        .and_then(|s| s.split("//").nth(1))
        .unwrap_or("");

    let auth =
        format!("Sentry sentry_version=7,sentry_key={key},sentry_client=epic-harness/{version}");

    http_post_envelope(&host, &format!("{path}/envelope/"), &envelope, &auth);
}

fn parse_sentry_dsn(dsn: &str) -> Option<(String, String)> {
    // https://<key>@<host>/<project_id>
    let without_scheme = dsn.strip_prefix("https://")?;
    let at = without_scheme.find('@')?;
    let rest = &without_scheme[at + 1..];
    let slash = rest.find('/')?;
    let host = rest[..slash].to_string();
    let project = &rest[slash..];
    let api_path = format!("/api{project}");
    Some((host, api_path))
}

// ── Minimal TLS-capable HTTP via openssl s_client fallback → raw TCP ─────────

fn http_post_tls(host: &str, _port: u16, path: &str, body: &str) {
    // Use `curl` as the TLS transport — avoids adding openssl/rustls crates.
    // Fire-and-forget: spawn detached, ignore errors.
    let url = format!("https://{host}{path}");
    let _ = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "5",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            body,
            &url,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn http_post_envelope(host: &str, path: &str, body: &str, auth: &str) {
    let url = format!("https://{host}{path}");
    let _ = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "5",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/x-sentry-envelope",
            "-H",
            &format!("X-Sentry-Auth: {auth}"),
            "-d",
            body,
            &url,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

// ── Telemetry subcommand handler (epic-harness telemetry [...]) ───────────────

pub fn run_cli(args: &[String]) -> i32 {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "on" => {
            write_consent(ConsentLevel::On);
            let id = load_or_create_install_id();
            eprintln!("[telemetry] Enabled (install ID: {id}).");
            0
        }
        "off" => {
            write_consent(ConsentLevel::Off);
            eprintln!("[telemetry] Disabled. No data will be sent.");
            0
        }
        // legacy aliases → map to on
        "community" | "anonymous" => {
            write_consent(ConsentLevel::On);
            let id = load_or_create_install_id();
            eprintln!("[telemetry] Enabled (install ID: {id}).");
            0
        }
        _ => {
            let level = read_consent();
            match level {
                ConsentLevel::On => {
                    let id = load_or_create_install_id();
                    eprintln!("[telemetry] Status: on  (install ID: {id})");
                }
                ConsentLevel::Off => {
                    eprintln!("[telemetry] Status: off");
                }
            }
            eprintln!("[telemetry] Toggle: epic-harness telemetry on|off");
            0
        }
    }
}

// Keep unused import quiet — TcpStream reserved for future plaintext fallback
#[allow(dead_code)]
fn _tcp_unused(_: TcpStream, _: Duration) {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ── New enum round-trip tests (TDD: written before implementation) ──

    #[test]
    fn rule_kind_as_str() {
        assert_eq!(RuleKind::Builtin.as_str(), "builtin");
        assert_eq!(RuleKind::ConventionalCommit.as_str(), "conventional_commit");
        assert_eq!(RuleKind::Custom.as_str(), "custom");
    }

    #[test]
    fn formatter_kind_as_str() {
        assert_eq!(FormatterKind::Tsc.as_str(), "tsc");
        assert_eq!(FormatterKind::Biome.as_str(), "biome");
        assert_eq!(FormatterKind::Prettier.as_str(), "prettier");
        assert_eq!(FormatterKind::Ruff.as_str(), "ruff");
        assert_eq!(FormatterKind::Gofmt.as_str(), "gofmt");
    }

    #[test]
    fn tool_category_from_str_known() {
        assert!(matches!(
            ToolCategory::from_str("bash").unwrap(),
            ToolCategory::Bash
        ));
        assert!(matches!(
            ToolCategory::from_str("edit").unwrap(),
            ToolCategory::Edit
        ));
        assert!(matches!(
            ToolCategory::from_str("write").unwrap(),
            ToolCategory::Write
        ));
        assert!(matches!(
            ToolCategory::from_str("read").unwrap(),
            ToolCategory::Read
        ));
        assert!(matches!(
            ToolCategory::from_str("glob").unwrap(),
            ToolCategory::Glob
        ));
        assert!(matches!(
            ToolCategory::from_str("grep").unwrap(),
            ToolCategory::Grep
        ));
    }

    #[test]
    fn tool_category_from_str_unknown() {
        assert!(matches!(
            ToolCategory::from_str("unknown_tool").unwrap(),
            ToolCategory::Other
        ));
        assert!(matches!(
            ToolCategory::from_str("").unwrap(),
            ToolCategory::Other
        ));
    }

    #[test]
    fn tool_category_as_str_roundtrip() {
        assert_eq!(ToolCategory::from_str("bash").unwrap().as_str(), "bash");
        assert_eq!(ToolCategory::from_str("edit").unwrap().as_str(), "edit");
        assert_eq!(ToolCategory::Other.as_str(), "other");
    }

    #[test]
    fn session_trend_from_str_known() {
        assert!(matches!(
            SessionTrend::from_str("improving").unwrap(),
            SessionTrend::Improving
        ));
        assert!(matches!(
            SessionTrend::from_str("declining").unwrap(),
            SessionTrend::Declining
        ));
        assert!(matches!(
            SessionTrend::from_str("stable").unwrap(),
            SessionTrend::Stable
        ));
    }

    #[test]
    fn session_trend_from_str_unknown_defaults_stable() {
        assert!(matches!(
            SessionTrend::from_str("whatever").unwrap(),
            SessionTrend::Stable
        ));
        assert!(matches!(
            SessionTrend::from_str("").unwrap(),
            SessionTrend::Stable
        ));
    }

    #[test]
    fn session_trend_as_str_roundtrip() {
        assert_eq!(
            SessionTrend::from_str("improving").unwrap().as_str(),
            "improving"
        );
        assert_eq!(
            SessionTrend::from_str("declining").unwrap().as_str(),
            "declining"
        );
        assert_eq!(SessionTrend::from_str("stable").unwrap().as_str(), "stable");
    }

    #[test]
    fn failure_class_from_str_known() {
        assert!(matches!(
            FailureClass::from_str("user_abort").unwrap(),
            FailureClass::UserAbort
        ));
        assert!(matches!(
            FailureClass::from_str("claude_api_error").unwrap(),
            FailureClass::ClaudeApiError
        ));
        assert!(matches!(
            FailureClass::from_str("tool_permission_denied").unwrap(),
            FailureClass::ToolPermissionDenied
        ));
        assert!(matches!(
            FailureClass::from_str("hook_failed").unwrap(),
            FailureClass::HookFailed
        ));
        assert!(matches!(
            FailureClass::from_str("git_conflict").unwrap(),
            FailureClass::GitConflict
        ));
        assert!(matches!(
            FailureClass::from_str("timeout_exceeded").unwrap(),
            FailureClass::TimeoutExceeded
        ));
    }

    #[test]
    fn failure_class_from_str_unknown_defaults_unknown() {
        assert!(matches!(
            FailureClass::from_str("not_a_class").unwrap(),
            FailureClass::Unknown
        ));
        assert!(matches!(
            FailureClass::from_str("").unwrap(),
            FailureClass::Unknown
        ));
    }

    #[test]
    fn uuid_v4_format() {
        let id = new_uuid_v4();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        // Version 4
        assert!(parts[2].starts_with('4'));
        // Variant bits
        let variant = u8::from_str_radix(&parts[3][..2], 16).unwrap();
        assert!(variant & 0xc0 == 0x80);
    }

    #[test]
    fn uuid_uniqueness() {
        assert_ne!(new_uuid_v4(), new_uuid_v4());
    }

    #[test]
    fn tokens_bucket_boundaries() {
        assert_eq!(TokensBucket::from_count(0).as_str(), "<1k");
        assert_eq!(TokensBucket::from_count(999).as_str(), "<1k");
        assert_eq!(TokensBucket::from_count(1000).as_str(), "<10k");
        assert_eq!(TokensBucket::from_count(9999).as_str(), "<10k");
        assert_eq!(TokensBucket::from_count(10_000).as_str(), "<100k");
        assert_eq!(TokensBucket::from_count(500_000).as_str(), ">=500k");
    }

    #[test]
    fn command_enum_str_values() {
        assert_eq!(Command::Work.as_str(), "work");
        assert_eq!(Command::Ship.as_str(), "ship");
        assert_eq!(Command::Review.as_str(), "review");
        assert_eq!(Command::Plan.as_str(), "plan");
        assert_eq!(Command::Test.as_str(), "test");
        assert_eq!(Command::Audit.as_str(), "audit");
    }

    #[test]
    fn outcome_str_values() {
        assert_eq!(Outcome::Success.as_str(), "success");
        assert_eq!(Outcome::Error.as_str(), "error");
        assert_eq!(Outcome::Abort.as_str(), "abort");
    }

    #[test]
    fn failure_class_str_values() {
        assert_eq!(FailureClass::UserAbort.as_str(), "user_abort");
        assert_eq!(FailureClass::HookFailed.as_str(), "hook_failed");
        assert_eq!(FailureClass::Unknown.as_str(), "unknown");
    }

    #[test]
    fn parse_sentry_dsn_valid() {
        let dsn = "https://abc123@o123456.ingest.sentry.io/789";
        let (host, path) = parse_sentry_dsn(dsn).unwrap();
        assert_eq!(host, "o123456.ingest.sentry.io");
        assert_eq!(path, "/api/789");
    }

    #[test]
    fn parse_sentry_dsn_invalid() {
        assert!(parse_sentry_dsn("not-a-dsn").is_none());
    }

    #[test]
    fn is_valid_uuid_accepts_valid_v4() {
        assert!(is_valid_uuid("550e8400-e29b-4d4a-a716-446655440000"));
        // generated by new_uuid_v4 — must always pass its own validator
        let id = new_uuid_v4();
        assert!(
            is_valid_uuid(&id),
            "new_uuid_v4() produced invalid UUID: {id}"
        );
    }

    #[test]
    fn is_valid_uuid_rejects_empty_string() {
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn is_valid_uuid_rejects_injection_string() {
        assert!(!is_valid_uuid(r#"","injected":"value""#));
    }

    #[test]
    fn is_valid_uuid_rejects_uuid_v1() {
        // UUID v1: third group starts with '1', not '4'
        assert!(!is_valid_uuid("6ba7b810-9dad-11d1-80b4-00c04fd430c8"));
    }

    #[test]
    fn is_valid_uuid_rejects_short_string() {
        assert!(!is_valid_uuid("1234-5678"));
    }

    fn make_telemetry(consent: ConsentLevel) -> Telemetry {
        let base_props = format!(
            r#""product":"epic-harness","product_version":"{}","os":"{}","telemetry_schema":"v1""#,
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
        );
        Telemetry {
            consent,
            distinct_id: if consent == ConsentLevel::On {
                "test-id".into()
            } else {
                String::new()
            },
            base_props,
        }
    }

    #[test]
    fn consent_off_disables_telemetry() {
        assert!(!make_telemetry(ConsentLevel::Off).is_enabled());
    }

    #[test]
    fn consent_on_enables_telemetry() {
        assert!(make_telemetry(ConsentLevel::On).is_enabled());
    }

    #[test]
    fn read_consent_defaults_to_off_when_unset() {
        // read_consent() must be conservative — no file means no consent.
        // ensure_consent_or_set_default() is responsible for setting On.
        assert_eq!(
            read_consent_raw().unwrap_or(ConsentLevel::Off),
            ConsentLevel::Off
        );
    }

    #[test]
    fn json_escape_handles_quotes_and_backslash() {
        assert_eq!(json_escape(r#"say "hello""#), r#"say \"hello\""#);
        assert_eq!(json_escape("back\\slash"), r#"back\\slash"#);
        assert_eq!(json_escape("new\nline"), r#"new\nline"#);
        assert_eq!(json_escape("carriage\rreturn"), r#"carriage\rreturn"#);
        assert_eq!(json_escape("plain text"), "plain text");
    }

    #[test]
    fn json_escape_handles_control_chars() {
        assert_eq!(json_escape("\t"), "\\t");
        assert_eq!(json_escape("\0"), "\\u0000");
        assert_eq!(json_escape("\x01"), "\\u0001");
        assert_eq!(json_escape("\x1f"), "\\u001f");
    }

    #[test]
    fn base_props_contains_product() {
        let t = make_telemetry(ConsentLevel::On);
        assert!(t.base_props.contains(r#""product":"epic-harness""#));
        assert!(t.base_props.contains(r#""telemetry_schema":"v1""#));
    }
}
