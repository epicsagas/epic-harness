#!/usr/bin/env node
// Epic Harness plugin bootstrap and cross-platform hook runner.
// Uses only Node.js built-ins — no npm install needed.

"use strict";

import { spawnSync } from "node:child_process";
import {
  chmodSync,
  createWriteStream,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import https from "node:https";
import os from "node:os";
import { pathToFileURL } from "node:url";

const REPO = "epicsagas/epic-harness";
const BINARY = "epic-harness";
const CARGO_PKG = "epic-harness";
const INSTALLER_MAX_REDIRECTS = 5;
const INSTALLER_REQUEST_TIMEOUT_MS = 15_000;
const INSTALLER_TOTAL_TIMEOUT_MS = 60_000;
const CODEX_GUARD_DENY =
  '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Blocked by Epic Harness guard"}}';
const STRUCTURED_CODEX_EVENTS = new Set([
  "SessionStart",
  "SubagentStop",
  "PreCompact",
  "SessionEnd",
]);
const HOOK_COMMANDS = new Map([
  ["SessionStart", new Set(["resume"])],
  ["PreToolUse", new Set(["guard"])],
  ["PostToolUse", new Set(["observe", "polish"])],
  ["SubagentStart", new Set(["observe"])],
  ["SubagentStop", new Set(["observe"])],
  ["PreCompact", new Set(["snapshot"])],
  ["SessionEnd", new Set(["reflect"])],
]);

function log(message) {
  process.stderr.write(`[epic-harness plugin] ${message}\n`);
}

function hasCommand(command) {
  const result = spawnSync(command, ["version"], {
    shell: false,
    stdio: "pipe",
  });
  return result.status === 0;
}

class HookRunError extends Error {
  constructor(message, exitCode) {
    super(message);
    this.exitCode = exitCode;
  }
}

function getBinaryRuntime() {
  const result = spawnSync(BINARY, ["version"], {
    shell: false,
    stdio: "pipe",
  });
  if (result.status !== 0) return null;

  const output = [result.stderr, result.stdout]
    .filter(Boolean)
    .map((value) => value.toString())
    .join("\n");
  const match = output.match(
    /(?:^|\r?\n)epic-harness\s+v?(\d+\.\d+\.\d+)\s+runtime-revision\s+([1-9]\d*)(?=\s|$)/,
  );
  return match ? { version: match[1], revision: match[2] } : null;
}

function getPluginRuntime() {
  const isClaude = !!process.env.CLAUDE_PLUGIN_ROOT;
  const pluginRoot =
    process.env.CLAUDE_PLUGIN_ROOT || process.env.PLUGIN_ROOT || "";
  if (!pluginRoot) {
    throw new Error("plugin root is unavailable");
  }

  const manifestPath = join(
    pluginRoot,
    isClaude ? ".claude-plugin" : ".codex-plugin",
    "plugin.json",
  );
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    throw new Error(
      `cannot read plugin manifest ${manifestPath}: ${error.message}`,
    );
  }
  const versionMatch =
    /^(\d+\.\d+\.\d+)(?:\+codex\.[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(
      manifest.version ?? "",
    );
  if (!versionMatch) {
    throw new Error(`plugin manifest has an invalid version: ${manifest.version}`);
  }
  const revisionPath = join(pluginRoot, "runtime-revision.txt");
  let revision;
  try {
    revision = readFileSync(revisionPath, "utf8").trim();
  } catch (error) {
    throw new Error(
      `cannot read runtime revision ${revisionPath}: ${error.message}`,
    );
  }
  if (!/^[1-9]\d*$/.test(revision)) {
    throw new Error(`runtime revision must be a positive integer: ${revision}`);
  }
  return { version: versionMatch[1], revision };
}

function installerUrl(version, extension) {
  return `https://github.com/${REPO}/releases/download/v${version}/epic-harness-installer.${extension}`;
}

export function downloadFile(
  url,
  destination,
  {
    requestTimeoutMs = INSTALLER_REQUEST_TIMEOUT_MS,
    totalTimeoutMs = INSTALLER_TOTAL_TIMEOUT_MS,
  } = {},
) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(destination, {
      flags: "wx",
      mode: 0o600,
    });
    let settled = false;
    let activeRequest;
    let totalTimer;
    const fail = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(totalTimer);
      activeRequest?.destroy();
      file.destroy();
      reject(error);
    };
    totalTimer = setTimeout(() => {
      fail(
        new Error(
          `installer download exceeded ${totalTimeoutMs} ms total timeout`,
        ),
      );
    }, totalTimeoutMs);
    file.once("error", fail);

    const follow = (currentUrl, redirects = 0) => {
      let parsedUrl;
      try {
        parsedUrl = new URL(currentUrl);
      } catch (error) {
        fail(new Error(`invalid installer URL ${currentUrl}: ${error.message}`));
        return;
      }
      if (parsedUrl.protocol !== "https:") {
        fail(new Error(`installer URL must use HTTPS: ${currentUrl}`));
        return;
      }
      activeRequest = https.get(parsedUrl, (response) => {
          if ([301, 302, 307, 308].includes(response.statusCode)) {
            if (!response.headers.location) {
              fail(new Error(`redirect without a location for ${currentUrl}`));
              return;
            }
            response.resume();
            if (redirects >= INSTALLER_MAX_REDIRECTS) {
              fail(
                new Error(
                  `installer redirect limit of ${INSTALLER_MAX_REDIRECTS} exceeded`,
                ),
              );
              return;
            }
            follow(
              new URL(response.headers.location, parsedUrl).toString(),
              redirects + 1,
            );
            return;
          }
          if (response.statusCode !== 200) {
            fail(new Error(`HTTP ${response.statusCode} for ${currentUrl}`));
            response.resume();
            return;
          }
          response.pipe(file);
          file.once("finish", () => {
            file.close((error) => {
              if (error) {
                fail(error);
              } else if (!settled) {
                settled = true;
                clearTimeout(totalTimer);
                resolve();
              }
            });
          });
        })
        .on("error", fail);
      activeRequest.setTimeout(requestTimeoutMs, () => {
        fail(
          new Error(
            `installer request timed out after ${requestTimeoutMs} ms for ${currentUrl}`,
          ),
        );
      });
    };
    follow(url);
  });
}

function sameRuntime(left, right) {
  return (
    left?.version === right?.version && left?.revision === right?.revision
  );
}

function runtimeLabel(runtime) {
  return `${runtime.version} (revision ${runtime.revision})`;
}

async function install(requiredRuntime) {
  const requiredVersion = requiredRuntime.version;
  const platform = os.platform();

  if (platform === "darwin") {
    const brewProbe = spawnSync("brew", ["--version"], {
      shell: false,
      stdio: "pipe",
    });
    if (brewProbe.status === 0) {
      log(`Homebrew detected — installing ${requiredVersion}...`);
      const result = spawnSync(
        "brew",
        ["install", "epicsagas/tap/epic-harness"],
        { shell: false, stdio: "inherit" },
      );
      if (result.status === 0 && sameRuntime(getBinaryRuntime(), requiredRuntime)) {
        return;
      }
      log("Homebrew did not provide the required version; trying next method...");
    }
  }

  const binstallProbe = spawnSync("cargo", ["binstall", "--version"], {
    shell: false,
    stdio: "pipe",
  });
  if (binstallProbe.status === 0) {
    log(`cargo-binstall detected — installing ${requiredVersion}...`);
    const result = spawnSync(
      "cargo",
      [
        "binstall",
        `${CARGO_PKG}@${requiredVersion}`,
        "--no-confirm",
        "--force",
      ],
      { shell: false, stdio: "inherit" },
    );
    if (result.status === 0) return;
    log("cargo-binstall failed; falling back to the release installer...");
  }

  if (platform === "win32") {
    const privateDirectory = mkdtempSync(
      join(os.tmpdir(), "epic-harness-installer-"),
    );
    try {
      const destination = join(privateDirectory, "installer.ps1");
      log(`Downloading Windows installer for ${requiredVersion}...`);
      await downloadFile(installerUrl(requiredVersion, "ps1"), destination);
      const result = spawnSync(
        "powershell",
        ["-ExecutionPolicy", "Bypass", "-File", destination],
        { shell: false, stdio: "inherit" },
      );
      if (result.status !== 0) throw new Error("PowerShell installer failed");
      return;
    } finally {
      rmSync(privateDirectory, { recursive: true, force: true });
    }
  }

  const privateDirectory = mkdtempSync(
    join(os.tmpdir(), "epic-harness-installer-"),
  );
  try {
    const destination = join(privateDirectory, "installer.sh");
    log(`Downloading installer for ${requiredVersion}...`);
    await downloadFile(installerUrl(requiredVersion, "sh"), destination);
    chmodSync(destination, 0o700);
    const result = spawnSync("sh", [destination], {
      shell: false,
      stdio: "inherit",
    });
    if (result.status !== 0) throw new Error("shell installer failed");
  } finally {
    rmSync(privateDirectory, { recursive: true, force: true });
  }
}

async function ensureCompatibleRuntime() {
  const requiredRuntime = getPluginRuntime();
  const present = hasCommand(BINARY);
  const currentRuntime = present ? getBinaryRuntime() : null;

  if (sameRuntime(currentRuntime, requiredRuntime)) return;

  if (!present) {
    log(`${BINARY} not found — installing ${runtimeLabel(requiredRuntime)}...`);
  } else if (currentRuntime) {
    log(
      `Updating ${BINARY} ${currentRuntime.version} → ${requiredRuntime.version} ` +
        `(runtime revision ${currentRuntime.revision} → ${requiredRuntime.revision})...`,
    );
  } else {
    log(
      `${BINARY} has an unreadable version or runtime revision — installing ${runtimeLabel(requiredRuntime)}...`,
    );
  }

  await install(requiredRuntime);

  const installedRuntime = getBinaryRuntime();
  if (!sameRuntime(installedRuntime, requiredRuntime)) {
    const actual = installedRuntime
      ? runtimeLabel(installedRuntime)
      : "no readable version";
    throw new Error(
      `required ${BINARY} ${requiredRuntime.version} is unavailable after installation ` +
        `(runtime revision ${requiredRuntime.revision}; found ${actual})`,
    );
  }

  log(
    present
      ? `Updated to ${runtimeLabel(installedRuntime)}`
      : `Installed ${BINARY} ${runtimeLabel(installedRuntime)}`,
  );
}

function runHook(event, subcommand) {
  if (!HOOK_COMMANDS.get(event)?.has(subcommand)) {
    throw new Error(`unsupported hook command: ${event} ${subcommand}`);
  }

  const captureStdout =
    event === "PreToolUse" || STRUCTURED_CODEX_EVENTS.has(event);
  const result = spawnSync(BINARY, [subcommand], {
    encoding: captureStdout ? "utf8" : undefined,
    shell: false,
    stdio: captureStdout
      ? ["inherit", "pipe", "inherit"]
      : ["inherit", "inherit", "inherit"],
  });

  if (result.error?.code === "ENOENT") {
    throw new HookRunError(
      `${BINARY} not found while running ${event}`,
      event === "PreToolUse" ? 2 : 1,
    );
  }
  if (result.error) {
    throw new HookRunError(
      `${BINARY} ${subcommand} failed: ${result.error.message}`,
      event === "PreToolUse" ? 2 : 1,
    );
  }
  if (result.status !== 0) {
    throw new HookRunError(
      `${BINARY} ${subcommand} failed with exit code ${result.status}`,
      event === "PreToolUse" ? 2 : (result.status ?? 1),
    );
  }

  if (STRUCTURED_CODEX_EVENTS.has(event)) {
    let output = result.stdout.trim();
    if (!output && event === "SubagentStop") {
      output = "{}";
    }
    if (!output) {
      return;
    }

    let parsed;
    try {
      parsed = JSON.parse(output);
    } catch {
      throw new Error(
        `${BINARY} ${subcommand} emitted invalid JSON for ${event}`,
      );
    }
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error(
        `${BINARY} ${subcommand} emitted a non-object JSON value for ${event}`,
      );
    }
    process.stdout.write(`${output}\n`);
  }
}

function failureOutputForInvocation() {
  const [mode, event] = process.argv.slice(2);
  if (mode !== "hook") {
    return null;
  }
  if (event === "PreToolUse") {
    return CODEX_GUARD_DENY;
  }
  if (STRUCTURED_CODEX_EVENTS.has(event)) {
    return "{}";
  }
  return null;
}

async function main() {
  const [mode, event, subcommand, ...extra] = process.argv.slice(2);

  if (mode === undefined) {
    await ensureCompatibleRuntime();
    return;
  }
  if (mode !== "hook" || !event || !subcommand || extra.length > 0) {
    throw new Error("usage: install.js [hook <event> <subcommand>]");
  }

  if (event === "SessionStart") {
    await ensureCompatibleRuntime();
  }
  runHook(event, subcommand);
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    const output = failureOutputForInvocation();
    if (output) {
      process.stdout.write(`${output}\n`);
    }
    log(error.message);
    process.exitCode = error.exitCode ?? 1;
  });
}
