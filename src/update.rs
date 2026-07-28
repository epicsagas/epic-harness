use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const GITHUB_REPO: &str = "epicsagas/epic-harness";
const CRATE_NAME: &str = "epic-harness";
const SYNC_COOLDOWN_SECS: u64 = 3600;
const DEFAULT_WEBUI_PORT: u16 = 7700;

#[derive(Debug, PartialEq)]
enum InstallMethod {
    Brew,
    Cargo,
    Unknown,
}

impl InstallMethod {
    fn label(&self) -> &'static str {
        match self {
            Self::Brew => "brew",
            Self::Cargo => "cargo",
            Self::Unknown => "unknown",
        }
    }
}

// ── paths ────────────────────────────────────────────────────────────

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn harness_dir() -> PathBuf {
    dirs_home().join(".harness")
}

fn sync_marker() -> PathBuf {
    harness_dir().join(".last-binary-sync")
}

// ── version helpers ─────────────────────────────────────────────────

fn extract_semver(s: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d+\.\d+\.\d+)").ok()?;
    re.captures(s).map(|c| c[1].to_string())
}

fn current_version() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let output = Command::new(&exe).arg("--version").output().ok()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    extract_semver(&combined)
}

fn latest_github_version() -> Option<String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let output = Command::new("curl").args(["-sf", &url]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let re = regex::Regex::new(r#""tag_name"\s*:\s*"(v?[^"]+)""#).ok()?;
    let tag = re.captures(&body)?.get(1)?.as_str();
    Some(tag.trim_start_matches('v').to_string())
}

fn semver_gt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    let (av, bv) = (parse(a), parse(b));
    for i in 0..3 {
        let (ai, bi) = (
            av.get(i).copied().unwrap_or(0),
            bv.get(i).copied().unwrap_or(0),
        );
        if ai > bi {
            return true;
        }
        if ai < bi {
            return false;
        }
    }
    false
}

// ── install method detection ────────────────────────────────────────

fn detect_install_method() -> InstallMethod {
    let exe = match std::env::current_exe().ok() {
        Some(e) => e,
        None => return InstallMethod::Unknown,
    };
    let s = exe.to_string_lossy();

    if (s.contains("/Cellar/") || s.contains("/opt/homebrew/"))
        && Command::new("brew")
            .args(["list", CRATE_NAME])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    {
        return InstallMethod::Brew;
    }
    if s.contains(".cargo")
        && Command::new("cargo")
            .args(["install", "--list"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(CRATE_NAME))
            .unwrap_or(false)
    {
        return InstallMethod::Cargo;
    }
    InstallMethod::Unknown
}

// ── upgrade ─────────────────────────────────────────────────────────

fn run_upgrade(method: &InstallMethod, latest: &str) -> io::Result<i32> {
    match method {
        InstallMethod::Brew => {
            eprintln!("[epic] Upgrading via Homebrew...");
            let st = Command::new("brew")
                .args(["upgrade", CRATE_NAME])
                .status()?;
            if st.success() {
                eprintln!("[epic] Updated to {latest}");
            } else {
                eprintln!("[epic] brew upgrade failed — try: brew upgrade {CRATE_NAME}");
            }
            Ok(st.code().unwrap_or(1))
        }
        InstallMethod::Cargo => {
            if Command::new("cargo-binstall")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                eprintln!("[epic] Upgrading via cargo-binstall...");
                let st = Command::new("cargo")
                    .args([
                        "binstall",
                        "-y",
                        "--no-confirm",
                        &format!("{CRATE_NAME}@{latest}"),
                    ])
                    .status()?;
                if st.success() {
                    eprintln!("[epic] Updated to {latest}");
                } else {
                    eprintln!("[epic] binstall failed — try: cargo binstall {CRATE_NAME}");
                }
                Ok(st.code().unwrap_or(1))
            } else {
                eprintln!("[epic] Upgrading via cargo install...");
                let st = Command::new("cargo")
                    .args(["install", &format!("{CRATE_NAME}@{latest}")])
                    .status()?;
                if st.success() {
                    eprintln!("[epic] Updated to {latest}");
                } else {
                    eprintln!("[epic] cargo install failed — try: cargo install {CRATE_NAME}");
                }
                Ok(st.code().unwrap_or(1))
            }
        }
        InstallMethod::Unknown => {
            eprintln!("[epic] Cannot detect install method. Update manually:");
            eprintln!("  brew upgrade {CRATE_NAME}");
            eprintln!("  cargo install {CRATE_NAME}");
            Ok(1)
        }
    }
}

// ── cooldown ────────────────────────────────────────────────────────

fn should_check() -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = fs::read_to_string(sync_marker())
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    now.saturating_sub(last) >= SYNC_COOLDOWN_SECS
}

fn touch_sync_marker() {
    let marker = sync_marker();
    let _ = fs::create_dir_all(marker.parent().unwrap_or(Path::new(".")));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = fs::write(&marker, now.to_string());
}

// ── MCP registration ────────────────────────────────────────────────

// ── Web UI ──────────────────────────────────────────────────────────

fn start_webui() -> Result<(), String> {
    let port: u16 = std::env::var("HARNESS_WEBUI_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WEBUI_PORT);
    crate::hooks::resume::start_dashboard_on_port(port, true)
}

// ── main entry ──────────────────────────────────────────────────────

pub fn run(args: &[String]) -> i32 {
    let check_only = args.iter().any(|a| a == "--check");
    let force = args.iter().any(|a| a == "--force");

    // The update command is already running inside the intended binary. Reuse
    // that executable for follow-up commands instead of resolving a second
    // plugin-local or PATH-owned runtime.
    let eh = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("[epic] Cannot resolve the current executable: {error}");
            return 1;
        }
    };

    // ── auto-update check (cooled down) ─────────────────────────
    if !check_only
        && (force || should_check())
        && let Some(cur) = current_version()
        && let Some(latest) = latest_github_version()
        && semver_gt(&latest, &cur)
    {
        eprintln!("[epic] Update available: {cur} → {latest}");
        let method = detect_install_method();
        eprintln!("[epic] Detected: {}", method.label());
        if let Err(error) = run_upgrade(&method, &latest) {
            eprintln!("[epic] Upgrade failed: {error}");
        }
        touch_sync_marker();
    }

    // --check: report version status only, skip side effects
    if check_only {
        if let Some(cur) = current_version() {
            eprintln!("Current: {cur}");
            if let Some(latest) = latest_github_version() {
                eprintln!("Latest:  {latest}");
                if semver_gt(&latest, &cur) {
                    eprintln!("[epic] Update available: {cur} → {latest}");
                    return 1;
                }
                eprintln!("[epic] Already up to date.");
            }
        }
        return 0;
    }

    // ── MCP registration (idempotent) ───────────────────────────
    let _ = Command::new(&eh).args(["mem", "mcp-install"]).output();

    // ── Web UI ──────────────────────────────────────────────────
    if let Err(error) = start_webui() {
        eprintln!("[epic] {error}");
        return 1;
    }

    0
}
