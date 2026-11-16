//! install.ps1 static contract. PowerShell is not available on every host, so
//! the behavior (a locked `epic.exe` failing the copy) is checked on Windows
//! CI only; these assertions pin the script structure that guarantees it.

const INSTALL_PS1: &str = include_str!("../install.ps1");

fn pos(needle: &str) -> usize {
    INSTALL_PS1
        .find(needle)
        .unwrap_or_else(|| panic!("install.ps1 missing {needle:?}"))
}

#[test]
fn failed_copy_cannot_reach_the_success_message() {
    // Non-terminating errors (a locked binary makes Copy-Item fail) must
    // become terminating before any step runs.
    assert!(pos("$ErrorActionPreference = \"Stop\"") < pos("Invoke-WebRequest"));
    // The installed file is compared with the extracted one after the copy.
    let copy = pos("Copy-Item -Path $src -Destination $dst");
    let check = pos("Get-FileHash -Path $dst");
    let success = pos("Installed: ");
    assert!(copy < check && check < success);
}

#[test]
fn arm64_falls_back_to_the_published_x64_archive() {
    // Only x86_64-pc-windows-msvc is published; an Arm64 target would 404.
    assert!(!INSTALL_PS1.contains("aarch64-pc-windows-msvc\"\n"));
    let arm = pos("$arch -eq \"Arm64\"");
    let notice = pos("runs under emulation");
    let target = INSTALL_PS1[arm..].find("$target = ").unwrap() + arm;
    assert!(arm < notice && notice < target);
    assert!(INSTALL_PS1[target..].starts_with("$target = \"x86_64-pc-windows-msvc\""));
}

#[test]
fn downloads_the_published_archive_name() {
    assert!(INSTALL_PS1.contains("$archive = \"epic-harness-$target.zip\""));
}
