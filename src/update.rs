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

// ── resolve binary ──────────────────────────────────────────────────

fn resolve_binary() -> Option<PathBuf> {
    // Plugin-bundled binary first
    if let Ok(root) = std::env::var("CLAUDE_PLUGIN_ROOT") {
        let bundled = Path::new(&root).join("hooks/bin/epic-harness");
        if bundled.is_file() {
            return Some(bundled);
        }
    }
    // PATH lookup
    let output = Command::new("which").arg(CRATE_NAME).output().ok()?;
    if output.status.success() {
        let p = String::from_utf8_lossy(&output.stdout);
        let p = p.trim();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    None
}

// ── install if missing ──────────────────────────────────────────────

fn install_binary() -> Option<PathBuf> {
    eprintln!("[epic] {CRATE_NAME} binary not found — installing...");
    let status = if Command::new("brew").arg("--version").output().is_ok() {
        Command::new("brew")
            .args(["install", CRATE_NAME])
            .status()
            .ok()
    } else {
        None
    };

    let status = status.filter(|s| s.success()).or_else(|| {
        if Command::new("cargo-binstall").arg("--version").output().is_ok() {
            Command::new("cargo-binstall")
                .args(["-y", "--no-confirm", CRATE_NAME])
                .status()
                .ok()
        } else {
            None
        }
    });

    let status = status.filter(|s| s.success()).or_else(|| {
        Command::new("cargo")
            .args(["install", CRATE_NAME])
            .status()
            .ok()
    });

    if status.map(|s| s.success()).unwrap_or(false) {
        resolve_binary()
    } else {
        eprintln!("[epic] Neither brew nor cargo found. Install one and restart.");
        None
    }
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
        let (ai, bi) = (av.get(i).copied().unwrap_or(0), bv.get(i).copied().unwrap_or(0));
        if ai > bi { return true; }
        if ai < bi { return false; }
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
        && Command::new("brew").args(["list", CRATE_NAME]).output().map(|o| o.status.success()).unwrap_or(false) {
        return InstallMethod::Brew;
    }
    if s.contains(".cargo")
        && Command::new("cargo").args(["install", "--list"]).output().map(|o| String::from_utf8_lossy(&o.stdout).contains(CRATE_NAME)).unwrap_or(false) {
        return InstallMethod::Cargo;
    }
    InstallMethod::Unknown
}

// ── upgrade ─────────────────────────────────────────────────────────

fn run_upgrade(method: &InstallMethod, latest: &str) -> io::Result<i32> {
    match method {
        InstallMethod::Brew => {
            eprintln!("[epic] Upgrading via Homebrew...");
            let st = Command::new("brew").args(["upgrade", CRATE_NAME]).status()?;
            if st.success() { eprintln!("[epic] Updated to {latest}"); } else { eprintln!("[epic] brew upgrade failed — try: brew upgrade {CRATE_NAME}"); }
            Ok(st.code().unwrap_or(1))
        }
        InstallMethod::Cargo => {
            if Command::new("cargo-binstall").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
                eprintln!("[epic] Upgrading via cargo-binstall...");
                let st = Command::new("cargo").args(["binstall", "-y", "--no-confirm", &format!("{CRATE_NAME}@{latest}")]).status()?;
                if st.success() { eprintln!("[epic] Updated to {latest}"); } else { eprintln!("[epic] binstall failed — try: cargo binstall {CRATE_NAME}"); }
                Ok(st.code().unwrap_or(1))
            } else {
                eprintln!("[epic] Upgrading via cargo install...");
                let st = Command::new("cargo").args(["install", &format!("{CRATE_NAME}@{latest}")]).status()?;
                if st.success() { eprintln!("[epic] Updated to {latest}"); } else { eprintln!("[epic] cargo install failed — try: cargo install {CRATE_NAME}"); }
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
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let last = fs::read_to_string(sync_marker()).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
    now.saturating_sub(last) >= SYNC_COOLDOWN_SECS
}

fn touch_sync_marker() {
    let marker = sync_marker();
    let _ = fs::create_dir_all(marker.parent().unwrap_or(Path::new(".")));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let _ = fs::write(&marker, now.to_string());
}

// ── MCP registration ────────────────────────────────────────────────

// ── Web UI ──────────────────────────────────────────────────────────

fn start_webui() {
    let port: u16 = std::env::var("HARNESS_WEBUI_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WEBUI_PORT);

    let url = format!("http://127.0.0.1:{port}/");
    if Command::new("curl").args(["-sf", &url]).output().is_ok() {
        return; // already running
    }

    let exe = match std::env::current_exe().ok() {
        Some(e) => e,
        None => return,
    };

    // Detach from current process
    let _ = Command::new(&exe)
        .args(["mem", "serve", "--port", &port.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    // Wait for bind, then open browser
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if Command::new("curl").args(["-sf", &url]).output().is_ok() {
            break;
        }
    }
    if Command::new("curl").args(["-sf", &url]).output().is_ok() {
        let browse_url = format!("http://localhost:{port}");
        #[cfg(target_os = "macos")]
        { let _ = Command::new("open").arg(&browse_url).spawn(); }
        #[cfg(target_os = "linux")]
        { let _ = Command::new("xdg-open").arg(&browse_url).spawn(); }
        #[cfg(target_os = "windows")]
        { let _ = Command::new("cmd").args(["/c", "start", &browse_url]).spawn(); }
    }
}

// ── main entry ──────────────────────────────────────────────────────

pub fn run(args: &[String]) -> i32 {
    let check_only = args.iter().any(|a| a == "--check");
    let force = args.iter().any(|a| a == "--force");

    // ── resolve or install binary ────────────────────────────────
    let eh = resolve_binary().or_else(install_binary);
    let eh = match eh {
        Some(p) => p,
        None => return 1,
    };

    // ── auto-update check (cooled down) ─────────────────────────
    let mut did_update = false;
    if !check_only && (force || should_check()) {
        if let Some(cur) = current_version() {
            if let Some(latest) = latest_github_version() {
                if semver_gt(&latest, &cur) {
                    eprintln!("[epic] Update available: {cur} → {latest}");
                    let method = detect_install_method();
                    eprintln!("[epic] Detected: {}", method.label());
                    match run_upgrade(&method, &latest) {
                        Ok(c) => { did_update = c == 0; }
                        Err(e) => eprintln!("[epic] Upgrade failed: {e}"),
                    }
                }
            }
        }
        touch_sync_marker();
    }

    // Re-resolve after potential update
    let eh = if did_update { resolve_binary().unwrap_or(eh) } else { eh };

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
    start_webui();

    0
}
