#!/usr/bin/env node
// Epic Harness Antigravity plugin bootstrap
// Runs on PreInvocation to ensure epic-harness binary is available.
// Uses only Node.js built-ins — no npm install needed.

"use strict";

const { execSync, spawnSync } = require("child_process");
const { createWriteStream, mkdirSync, chmodSync, existsSync, readFileSync } = require("fs");
const { join } = require("path");
const https = require("https");
const os = require("os");

const REPO = "epicsagas/epic-harness";
const BINARY = "epic-harness";
const INSTALLER_SH = `https://github.com/${REPO}/releases/latest/download/epic-harness-installer.sh`;
const INSTALLER_PS1 = `https://github.com/${REPO}/releases/latest/download/epic-harness-installer.ps1`;

function log(msg) {
  process.stderr.write(`[epic-harness plugin] ${msg}\n`);
}

function hasCommand(cmd) {
  const r = spawnSync(cmd, ["version"], { stdio: "pipe", shell: false });
  return r.status === 0;
}

function getBinaryVersion() {
  try {
    const r = spawnSync(BINARY, ["version"], { stdio: "pipe", shell: false });
    if (r.status === 0) {
      const output = r.stdout.toString().trim();
      const match = output.match(/(\d+\.\d+\.\d+)/);
      return match ? match[1] : null;
    }
  } catch (_) {}
  return null;
}

function getPluginVersion() {
  try {
    // In Antigravity context, look for plugin.json next to hooks/
    const manifestPath = join(__dirname, "..", "plugin.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    return manifest.version || null;
  } catch (_) {}
  return null;
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

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    const follow = (u) => {
      https.get(u, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          follow(res.headers.location);
          res.resume();
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} for ${u}`));
          return;
        }
        res.pipe(file);
        file.on("finish", () => file.close(resolve));
      }).on("error", reject);
    };
    follow(url);
  });
}

async function install() {
  const platform = os.platform(); // 'darwin' | 'linux' | 'win32'

  if (platform === "win32") {
    const tmp = join(os.tmpdir(), "epic-harness-installer.ps1");
    log("Downloading Windows installer...");
    await downloadFile(INSTALLER_PS1, tmp);
    const r = spawnSync(
      "powershell",
      ["-ExecutionPolicy", "Bypass", "-File", tmp],
      { stdio: "inherit" }
    );
    if (r.status !== 0) throw new Error("PowerShell installer failed");
  } else {
    const tmp = join(os.tmpdir(), "epic-harness-installer.sh");
    log("Downloading installer...");
    await downloadFile(INSTALLER_SH, tmp);
    chmodSync(tmp, 0o755);
    const r = spawnSync("sh", [tmp], { stdio: "inherit" });
    if (r.status !== 0) throw new Error("Shell installer failed");
  }
}

async function main() {
  // Antigravity hooks receive JSON on stdin — read and discard
  let input = {};
  try {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    input = JSON.parse(Buffer.concat(chunks).toString() || "{}");
  } catch (_) {}

  const pluginVersion = getPluginVersion();

  // 1. Binary not found — fresh install
  if (!hasCommand(BINARY)) {
    log(`${BINARY} not found — installing...`);
    try {
      await install();
    } catch (e) {
      log(`Install failed: ${e.message}`);
      log(`Install manually: https://github.com/${REPO}#installation`);
    }
  } else {
    // 2. Binary exists — check version and update if plugin is newer
    if (pluginVersion) {
      const binaryVersion = getBinaryVersion();
      if (binaryVersion && semverGt(pluginVersion, binaryVersion)) {
        log(`Updating ${BINARY} ${binaryVersion} → ${pluginVersion}...`);
        try {
          await install();
          const newVersion = getBinaryVersion();
          if (newVersion) {
            log(`Updated to ${newVersion}`);
          }
        } catch (e) {
          log(`Update failed: ${e.message}`);
          log(`Continuing with ${binaryVersion}`);
        }
      }
    }
  }

  // Antigravity expects JSON on stdout for PreInvocation
  process.stdout.write(JSON.stringify({ injectSteps: [], terminationBehavior: "" }));
}

main().catch(() => {
  process.stdout.write(JSON.stringify({ injectSteps: [], terminationBehavior: "" }));
});
