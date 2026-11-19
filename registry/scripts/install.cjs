#!/usr/bin/env node
// Epic Harness plugin bootstrap
// Runs on SessionStart via run.mjs.
// Uses only Node.js built-ins — no npm install needed.
// Claude Code is built on Node.js, so `node` is always available on the PATH
// that Claude Code uses to launch hook commands.
//
// Installs ONLY the release archive pinned to this plugin's own version
// (v<manifest version>) into ~/.local/bin and verifies its sha256 from the
// same release before anything is extracted. Integrity, not authenticity: the
// pin comes from the plugin manifest delivered through the marketplace. No
// downloaded script is ever executed. An older epic elsewhere on PATH (brew,
// cargo, manual) is never touched: the pinned copy goes to ~/.local/bin, which
// run.mjs prefers for hooks, and the user gets a one-line update notice.

"use strict";

const { spawnSync } = require("child_process");
const crypto = require("crypto");
const fs = require("fs");
const https = require("https");
const os = require("os");
const path = require("path");
const { pipeline } = require("stream");

const REPO = "epicsagas/epic-harness";
const README_URL = `https://github.com/${REPO}#installation`;
const SEMVER = /^\d+\.\d+\.\d+$/;
const RETRY_AFTER_MS = 60 * 60 * 1000;
const DOWNLOAD_TIMEOUT_MS = 120 * 1000;
const TMP_PREFIX = "epic-bootstrap-";

function log(msg) {
  process.stderr.write(`[epic-harness plugin] ${msg}\n`);
}

function pluginVersion(root = path.resolve(__dirname, "..", "..")) {
  for (const rel of [
    [".claude-plugin", "plugin.json"],
    [".codex-plugin", "plugin.json"],
    [".grok-plugin", "plugin.json"],
    ["plugin.json"],
  ]) {
    try {
      const { version } = JSON.parse(
        fs.readFileSync(path.join(root, ...rel), "utf8"),
      );
      if (SEMVER.test(version)) return version;
    } catch (_) {}
  }
  return null;
}

function targetFor(platform, arch) {
  return (
    {
      "darwin/arm64": "aarch64-apple-darwin",
      "darwin/x64": "x86_64-apple-darwin",
      "linux/arm64": "aarch64-unknown-linux-gnu",
      "linux/x64": "x86_64-unknown-linux-gnu",
      // arm64 Windows runs the x64 build under emulation (install.ps1)
      "win32/x64": "x86_64-pc-windows-msvc",
      "win32/arm64": "x86_64-pc-windows-msvc",
    }[`${platform}/${arch}`] || null
  );
}

function assetNames(version, target) {
  if (!SEMVER.test(version)) throw new Error(`invalid version: ${version}`);
  const archive = `epic-harness-${target}.${target.includes("windows") ? "zip" : "tar.xz"}`;
  const base = `https://github.com/${REPO}/releases/download/v${version}/`;
  return {
    archive,
    checksum: `${archive}.sha256`,
    archiveUrl: base + archive,
    checksumUrl: `${base}${archive}.sha256`,
  };
}

function parseSha256(text, archiveName) {
  const [hash, name] = String(text).split(/\r?\n/)[0].trim().split(/\s+/);
  if (!/^[0-9a-f]{64}$/i.test(hash || "")) return null;
  if ((name || "").replace(/^\*/, "") !== archiveName) return null;
  return hash.toLowerCase();
}

function isSafeRedirect(url) {
  try {
    return new URL(url).protocol === "https:";
  } catch (_) {
    return false;
  }
}

function semverGt(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if (pa[i] > pb[i]) return true;
    if (pa[i] < pb[i]) return false;
  }
  return false;
}

function markerPath(dir) {
  return path.join(dir, "bootstrap_attempt.json");
}

// A failed attempt for the same version is not retried for an hour, so a
// missing `xz` or not-yet-uploaded assets cannot cause a download per session.
function shouldAttempt(version, dir, now = Date.now()) {
  try {
    const m = JSON.parse(fs.readFileSync(markerPath(dir), "utf8"));
    if (m.version === version && m.ok === false && now - m.at < RETRY_AFTER_MS)
      return false;
  } catch (_) {}
  return true;
}

function recordAttempt(version, ok, dir, now = Date.now()) {
  try {
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(markerPath(dir), JSON.stringify({ version, at: now, ok }));
  } catch (_) {}
}

// Leftovers of a killed session: nothing else removes them.
function sweepStaleTmp(dir = os.tmpdir(), now = Date.now()) {
  try {
    for (const f of fs.readdirSync(dir)) {
      if (!f.startsWith(TMP_PREFIX)) continue;
      const p = path.join(dir, f);
      if (now - fs.statSync(p).mtimeMs > RETRY_AFTER_MS)
        fs.rmSync(p, { recursive: true, force: true });
    }
  } catch (_) {}
}

// timeoutMs bounds the whole transfer (redirects and body), not socket idleness.
function downloadFile(url, dest, { hops = 5, deadline, timeoutMs = DOWNLOAD_TIMEOUT_MS, get = https.get } = {}) {
  deadline = deadline ?? Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    let timer;
    const fail = (e) => {
      clearTimeout(timer);
      fs.rmSync(dest, { force: true });
      reject(e);
    };
    const req = get(url, (res) => {
      if ([301, 302, 303, 307, 308].includes(res.statusCode)) {
        res.resume();
        let next = null;
        try {
          next = new URL(res.headers.location, url).href;
        } catch (_) {}
        if (!next || !isSafeRedirect(next)) {
          return fail(new Error(`unsafe redirect from ${url}`));
        }
        if (hops <= 0) return fail(new Error("too many redirects"));
        clearTimeout(timer);
        return downloadFile(next, dest, { hops: hops - 1, deadline, get }).then(resolve, fail);
      }
      if (res.statusCode !== 200) {
        res.resume();
        return fail(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      pipeline(res, fs.createWriteStream(dest), (e) => {
        if (e) return fail(e);
        clearTimeout(timer);
        resolve();
      });
    });
    timer = setTimeout(() => req.destroy(new Error("timeout")), Math.max(0, deadline - Date.now()));
    req.on("error", fail);
  });
}

// Verify the archive against its .sha256 text BEFORE extracting; on mismatch
// nothing is extracted or installed.
function installFromArchive({
  archive,
  checksumText,
  target,
  installDir,
  platform = process.platform,
}) {
  const expected = parseSha256(checksumText, path.basename(archive));
  if (!expected) throw new Error("invalid checksum file");
  const actual = crypto
    .createHash("sha256")
    .update(fs.readFileSync(archive))
    .digest("hex");
  if (actual !== expected) throw new Error("checksum mismatch");

  const win = platform === "win32";
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), TMP_PREFIX));
  try {
    // Absolute tar.exe on Windows: a Git-for-Windows GNU tar earlier on PATH
    // cannot read zip.
    const tar = win
      ? path.join(process.env.SystemRoot, "System32", "tar.exe")
      : "tar";
    const r = spawnSync(tar, [win ? "-xf" : "-xJf", archive, "-C", tmp], {
      shell: false,
    });
    if (r.status !== 0) throw new Error("archive extraction failed");
    const srcDir = win ? tmp : path.join(tmp, `epic-harness-${target}`);
    const names = ["epic", "epic-harness"].map((n) => (win ? `${n}.exe` : n));
    for (const n of names) {
      if (!fs.existsSync(path.join(srcDir, n)))
        throw new Error(`${n} missing from archive`);
    }
    fs.mkdirSync(installDir, { recursive: true });
    for (const n of names) {
      // copy + rename: atomic replace, concurrent sessions never see a torn file
      const staged = path.join(installDir, `${n}.tmp-${process.pid}`);
      try {
        fs.copyFileSync(path.join(srcDir, n), staged);
        if (!win) fs.chmodSync(staged, 0o755);
        fs.renameSync(staged, path.join(installDir, n));
      } catch (e) {
        fs.rmSync(staged, { force: true });
        throw e; // a locked Windows destination keeps the old binary
      }
    }
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

async function fetchAndInstall({ version, target, installDir, platform, download }) {
  const a = assetNames(version, target);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), TMP_PREFIX));
  try {
    const archive = path.join(tmp, a.archive);
    const sum = path.join(tmp, a.checksum);
    // checksum first: a missing .sha256 refuses before the big download
    await download(a.checksumUrl, sum);
    await download(a.archiveUrl, archive);
    installFromArchive({
      archive,
      checksumText: fs.readFileSync(sum, "utf8"),
      target,
      installDir,
      platform,
    });
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

function findEpic(pathEnv, installDir, platform) {
  const exts = platform === "win32" ? [".exe", ".cmd"] : [""];
  for (const dir of [...pathEnv.split(path.delimiter).filter(Boolean), installDir]) {
    for (const ext of exts) {
      const p = path.join(dir, `epic${ext}`);
      if (fs.existsSync(p)) return p;
    }
  }
  return null;
}

function binaryVersion(bin) {
  // --version exits before the telemetry consent step that `version` runs
  const r = spawnSync(bin, ["--version"], { stdio: "pipe", shell: false, timeout: 10000 });
  const m = r.status === 0 && r.stdout.toString().match(/(\d+\.\d+\.\d+)/);
  return m ? m[1] : null;
}

function sameDir(a, b, platform) {
  const [x, y] = [a, b].map((p) => path.resolve(p));
  return platform === "win32" ? x.toLowerCase() === y.toLowerCase() : x === y;
}

function updateCommand(bin) {
  if (/cellar|homebrew|linuxbrew/i.test(bin)) return "brew upgrade epic-harness";
  if (/[\\/]\.cargo[\\/]/.test(bin)) {
    // cargo records its own installs; anything else in ~/.cargo/bin came from
    // the installer script the old bootstrap used.
    let cargoOwned = false;
    try {
      const reg = path.join(path.dirname(path.dirname(bin)), ".crates2.json");
      cargoOwned = fs.readFileSync(reg, "utf8").includes('"epic-harness ');
    } catch (_) {}
    return cargoOwned
      ? "cargo install epic-harness --locked --force"
      : `re-run the installer command from ${README_URL}`;
  }
  return "epic update";
}

async function main(deps = {}) {
  const {
    download = downloadFile,
    root,
    home = os.homedir(),
    pathEnv = process.env.PATH || "",
    platform = process.platform,
    arch = process.arch,
  } = deps;
  const harnessDir =
    deps.harnessDir || process.env.HARNESS_DIR || path.join(home, ".harness");
  const installDir = path.join(home, ".local", "bin");

  const version = pluginVersion(root);
  if (!version) {
    log(`cannot determine plugin version; install manually: ${README_URL}`);
    return;
  }

  const found = findEpic(pathEnv, installDir, platform);
  let notice = null;
  if (found) {
    const current = binaryVersion(found);
    if (!current || !semverGt(version, current)) return; // up to date, never downgrade
    if (!sameDir(path.dirname(found), installDir, platform)) {
      // Hooks run the ~/.local/bin copy, so a current one there needs no download.
      const local = findEpic("", installDir, platform);
      const localVersion = local && binaryVersion(local);
      if (localVersion && !semverGt(version, localVersion)) return;
      notice = `your shell epic ${current} is older than the plugin's ${version} (hooks use ${installDir}); update it with: ${updateCommand(found)}`;
    }
  }

  const target = targetFor(platform, arch);
  if (!target) {
    log(`unsupported platform ${platform}/${arch}; install manually: ${README_URL}`);
    return;
  }
  if (!shouldAttempt(version, harnessDir)) return;

  log(found ? `Updating epic to ${version}...` : `epic not found, installing ${version}...`);
  // Marker first: a killed session must not cause a download per session.
  recordAttempt(version, false, harnessDir);
  sweepStaleTmp();
  try {
    await fetchAndInstall({ version, target, installDir, platform, download });
    recordAttempt(version, true, harnessDir);
    log(`Installed epic ${version} to ${installDir}`);
    if (notice) log(notice);
  } catch (e) {
    log(`Install failed: ${e.message}`);
    log(`Install manually: ${README_URL}`);
  }
}

module.exports = {
  pluginVersion,
  targetFor,
  assetNames,
  parseSha256,
  isSafeRedirect,
  shouldAttempt,
  recordAttempt,
  sweepStaleTmp,
  downloadFile,
  updateCommand,
  installFromArchive,
  main,
};

if (require.main === module) {
  main().catch((e) => {
    log(`Unexpected error: ${e.message}`);
    process.exit(0); // non-fatal
  });
}
