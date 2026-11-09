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
$archive = "epic-harness-$target.zip"
$url     = "$baseUrl/$archive"
$shaUrl  = "$baseUrl/$archive.sha256"

# ── Download, verify, and install ────────────────────────────────────────────
Write-Host "Installing $Binary v$version for $target..."

$tmpdir = New-Item -ItemType Directory -Path (Join-Path ([IO.Path]::GetTempPath()) ("epic-install-" + [Guid]::NewGuid().ToString("N"))) -ErrorAction Stop
$written = @()
$backups = @{}
$keepRecovery = $false
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
    $sources = @{}
    foreach ($name in @('epic', 'epic-harness')) {
        $candidates = @(Get-ChildItem -LiteralPath $tmpdir -File -Recurse -Filter "$name.exe" -ErrorAction Stop)
        if ($candidates.Count -gt 1) { throw "Ambiguous executable payload: $name.exe" }
        if ($candidates.Count -eq 1) { $sources[$name] = $candidates[0].FullName }
    }
    $fallback = $sources['epic-harness']
    if (-not $fallback) { $fallback = $sources['epic'] }
    if (-not $fallback) { throw 'Archive contains neither epic.exe nor epic-harness.exe' }
    # Read all old bytes before changing either alias. A locked secondary alias
    # must not leave a successful first copy mixed with the previous version.
    foreach ($name in @('epic', 'epic-harness')) {
        $destination = Join-Path $InstallDir "$name.exe"
        if (Test-Path -LiteralPath $destination) {
            $backup = Join-Path $tmpdir "$name.previous"
            Copy-Item -LiteralPath $destination -Destination $backup -ErrorAction Stop
            $backups[$name] = $backup
        }
    }
    foreach ($name in @('epic', 'epic-harness')) {
        $source = $sources[$name]
        if (-not $source) { $source = $fallback }
        $destination = Join-Path $InstallDir "$name.exe"
        $written += $name
        Copy-Item -LiteralPath $source -Destination $destination -Force -ErrorAction Stop
        $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256 -ErrorAction Stop).Hash
        $installedHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256 -ErrorAction Stop).Hash
        if ($installedHash -ne $sourceHash) { throw "Installed $name does not match the verified payload" }
    }
    Write-Host "Installed: $Binary v$version at $InstallDir (epic and epic-harness)"
    if (-not (Get-Command $Binary -ErrorAction SilentlyContinue)) {
        Write-Host "Add $InstallDir to your PATH."
    }
} catch {
    $failure = $_.Exception.Message
    foreach ($name in $written) {
        $destination = Join-Path $InstallDir "$name.exe"
        try {
            if ($backups.ContainsKey($name)) { Copy-Item -LiteralPath $backups[$name] -Destination $destination -Force -ErrorAction Stop }
            else { Remove-Item -LiteralPath $destination -Force -ErrorAction Stop }
        } catch {
            $keepRecovery = $true
            Write-Warning "Could not restore $destination. Recovery files retained at $($tmpdir.FullName)."
        }
    }
    Write-Error ("Installation failed: " + $failure) -ErrorAction Continue
    exit 1
} finally {
    # Delete only the unique temporary directory created by this invocation.
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $resolvedTemporary = [IO.Path]::GetFullPath($tmpdir.FullName)
    if (-not $keepRecovery -and $resolvedTemporary.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolvedTemporary).StartsWith("epic-install-")) {
        Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force -ErrorAction SilentlyContinue
    }
}
