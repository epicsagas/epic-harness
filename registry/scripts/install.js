#!/usr/bin/env node
// Epic Harness plugin bootstrap
// Runs on SessionStart via hooks.json.
// Uses only Node.js built-ins — no npm install needed.
// Claude Code is built on Node.js, so `node` is always available on the PATH
// that Claude Code uses to launch hook commands.

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
    const manifestPath = join(
      process.env.CLAUDE_PLUGIN_ROOT || "",
      ".claude-plugin",
      "plugin.json"
    );
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
    // Windows: download installer.ps1 and run via PowerShell
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
    // macOS / Linux: download installer.sh and run via sh
    const tmp = join(os.tmpdir(), "epic-harness-installer.sh");
    log("Downloading installer...");
    await downloadFile(INSTALLER_SH, tmp);
    chmodSync(tmp, 0o755);
    const r = spawnSync("sh", [tmp], { stdio: "inherit" });
    if (r.status !== 0) throw new Error("Shell installer failed");
  }
}

function seed() {
  const r = spawnSync(BINARY, ["install", "claude"], { stdio: "inherit" });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

async function main() {
  const pluginVersion = getPluginVersion();
  const isPlugin = !!process.env.CLAUDE_PLUGIN_ROOT;

  // 1. Binary not found — fresh install
  if (!hasCommand(BINARY)) {
    log(`${BINARY} not found — installing...`);
    try {
      await install();
    } catch (e) {
      log(`Install failed: ${e.message}`);
      log(`Install manually: https://github.com/${REPO}#installation`);
      process.exit(0); // non-fatal — don't break the session
    }
    // Plugin mode: skills/agents auto-discovered from plugin cache, skip manual seeding
    if (hasCommand(BINARY) && !isPlugin) seed();
    return;
  }

  // 2. Binary exists — check version
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
        // non-fatal — old binary still works
      }
    }
  }

  // 3. Seed skills/agents/commands/MCP (standalone installs only)
  if (!isPlugin) seed();
}

main().catch((e) => {
  log(`Unexpected error: ${e.message}`);
  process.exit(0); // non-fatal
});
