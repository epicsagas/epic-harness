# install.ps1 — one-line installer for epic (Windows)
# Usage: irm https://github.com/epicsagas/epic-harness/releases/latest/download/install.ps1 | iex
param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin"
)

# A failed step (locked binary, bad download) must stop the script, not fall
# through to the success message.
$ErrorActionPreference = "Stop"

$Repo   = "epicsagas/epic-harness"
$Binary = "epic"

# ── Detect architecture ───────────────────────────────────────────────────────
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -eq "X64") {
    $target = "x86_64-pc-windows-msvc"
} elseif ($arch -eq "Arm64") {
    # No aarch64-pc-windows-msvc archive is published; the x64 build runs under emulation.
    Write-Host "Windows on Arm: installing the x86_64 build (runs under emulation)."
    $target = "x86_64-pc-windows-msvc"
} else {
    Write-Error "Error: unsupported architecture $arch"; exit 1
}

# ── Resolve latest version ────────────────────────────────────────────────────
$tag = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest").tag_name
if (-not $tag) { Write-Error "Error: could not determine latest version"; exit 1 }
$version = $tag.TrimStart("v")

$baseUrl = "https://github.com/$Repo/releases/download/$tag"
$archive = "epic-harness-$target.zip"
$url     = "$baseUrl/$archive"
$shaUrl  = "$baseUrl/$archive.sha256"

# ── Download, verify, and install ────────────────────────────────────────────
Write-Host "Installing $Binary v$version for $target..."

$tmpdir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "epic-install-$pid") -Force
$zip    = Join-Path $tmpdir $archive
$shaFile = Join-Path $tmpdir "$archive.sha256"

try {
    Invoke-WebRequest -Uri $url    -OutFile $zip     -UseBasicParsing
    Invoke-WebRequest -Uri $shaUrl -OutFile $shaFile -UseBasicParsing

    # SHA-256 verification
    $expected = (Get-Content $shaFile -Raw).Split(" ")[0].Trim()
    $actual   = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        Write-Error "Error: SHA-256 verification failed (expected $expected, got $actual)"; exit 1
    }

    Expand-Archive -Path $zip -DestinationPath $tmpdir -Force

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $src = Join-Path $tmpdir "$Binary.exe"
    $dst = Join-Path $InstallDir "$Binary.exe"
    Copy-Item -Path $src -Destination $dst -Force
    # Confirm the installed file is the new binary, not a stale one.
    if ((Get-FileHash -Path $dst -Algorithm SHA256).Hash -ne (Get-FileHash -Path $src -Algorithm SHA256).Hash) {
        Write-Error "Error: $dst was not replaced (is $Binary running?)"; exit 1
    }
} finally {
    Remove-Item -Path $tmpdir -Recurse -Force -ErrorAction SilentlyContinue
}

# ── Verify ────────────────────────────────────────────────────────────────────
if (Get-Command $Binary -ErrorAction SilentlyContinue) {
    Write-Host "Installed: $Binary v$version"
} else {
    Write-Host ""
    Write-Host "Add $InstallDir to your PATH:"
    Write-Host "  `$env:PATH = `"$InstallDir;`$env:PATH`""
}
