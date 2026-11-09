# install.ps1 — one-line installer for epic (Windows)
# Usage: irm https://github.com/epicsagas/epic-harness/releases/latest/download/install.ps1 | iex
param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin"
)

$Repo   = "epicsagas/epic-harness"
$Binary = "epic"

# ── Detect architecture ───────────────────────────────────────────────────────
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -eq "X64") {
    $target = "x86_64-pc-windows-msvc"
} elseif ($arch -eq "Arm64") {
    $target = "aarch64-pc-windows-msvc"
} else {
    Write-Error "Error: unsupported architecture $arch"; exit 1
}

# ── Resolve latest version ────────────────────────────────────────────────────
$tag = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest").tag_name
if (-not $tag) { Write-Error "Error: could not determine latest version"; exit 1 }
$version = $tag.TrimStart("v")

$baseUrl = "https://github.com/$Repo/releases/download/$tag"
$archive = "$Binary-$target.zip"
$url     = "$baseUrl/$archive"
$shaUrl  = "$baseUrl/$archive.sha256"

# ── Download, verify, and install ────────────────────────────────────────────
Write-Host "Installing $Binary v$version for $target..."

$tmpdir = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ("epic-install-" + [Guid]::NewGuid().ToString("N"))) -ErrorAction Stop
try {
    $zip = Join-Path $tmpdir $archive
    $shaFile = Join-Path $tmpdir "$archive.sha256"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing -ErrorAction Stop
    Invoke-WebRequest -Uri $shaUrl -OutFile $shaFile -UseBasicParsing -ErrorAction Stop
    $expected = (Get-Content -LiteralPath $shaFile -Raw -ErrorAction Stop).Trim().Split()[0].ToLower()
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256 -ErrorAction Stop).Hash.ToLower()
    if ($actual -ne $expected) { throw "SHA-256 verification failed" }
    Expand-Archive -LiteralPath $zip -DestinationPath $tmpdir -Force -ErrorAction Stop
    New-Item -ItemType Directory -Path $InstallDir -Force -ErrorAction Stop | Out-Null
    $source = Join-Path $tmpdir "$Binary.exe"
    $destination = Join-Path $InstallDir "$Binary.exe"
    Copy-Item -LiteralPath $source -Destination $destination -Force -ErrorAction Stop
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256 -ErrorAction Stop).Hash
    $installedHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256 -ErrorAction Stop).Hash
    if ($installedHash -ne $sourceHash) { throw "Installed executable does not match the verified payload" }
    Write-Host "Installed: $Binary v$version at $destination"
    if (-not (Get-Command $Binary -ErrorAction SilentlyContinue)) {
        Write-Host "Add $InstallDir to your PATH."
    }
} catch {
    Write-Error ("Installation failed: " + $_.Exception.Message) -ErrorAction Continue
    exit 1
} finally {
    # Delete only the unique temporary directory created by this invocation.
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $resolvedTemporary = [IO.Path]::GetFullPath($tmpdir.FullName)
    if ($resolvedTemporary.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolvedTemporary).StartsWith("epic-install-")) {
        Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force -ErrorAction SilentlyContinue
    }
}
