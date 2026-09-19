#!/usr/bin/env node
// Behavioral acceptance for the Codex hook + MCP surfaces (#131/#132) on a
// real host shell. Static shell-free checks live in Rust
// (tests/plugin_assets_test.rs); this script proves the shipped commands
// actually EXECUTE — on Windows via `cmd.exe /C`, exactly how Codex runs
// hook commands there (openai/codex#32402).
//
// Modes (run by the ci.yml windows-hooks job):
//   degrade — epic must NOT be on PATH: every hook exits 0 with the
//             `[harness] epic not found` notice (session-start excluded: it
//             IS the installer, covered by `live`).
//   live    — epic on PATH (installed from the latest release artifact by
//             the workflow): every hook exits 0.
//   mcp     — `epic mem mcp` JSON-RPC initialize handshake proves the
//             harness-mem server starts (this is what mcp_config.json
//             launches).

import { readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import process from "node:process";

const root = process.env.PLUGIN_ROOT ?? process.cwd();
const mode = process.argv[2] ?? "";
const isWin = process.platform === "win32";

const manifest = JSON.parse(
  readFileSync(path.join(root, ".codex-plugin", "hooks.json"), "utf8"),
);
const commands = [
  ...new Set(
    Object.values(manifest.hooks)
      .flat()
      .flatMap((group) => group.hooks)
      .filter((handler) => handler.type === "command")
      .map((handler) => handler.command),
  ),
];

let failures = 0;
const check = (name, ok, extra = "") => {
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${extra ? ` — ${extra}` : ""}`);
  if (!ok) failures++;
};

// The exact way Codex invokes hook commands: cmd.exe /C on Windows.
const runHook = (cmd) =>
  spawnSync(isWin ? "cmd.exe" : "/bin/sh", isWin ? ["/C", cmd] : ["-c", cmd], {
    input: "{}",
    encoding: "utf8",
    timeout: 60_000,
    env: { ...process.env, PLUGIN_ROOT: root },
  });

if (mode === "degrade") {
  for (const cmd of commands) {
    if (cmd.includes("session-start")) continue; // the installer; runs in `live`
    const r = runHook(cmd);
    check(
      `degrade: ${cmd}`,
      r.status === 0 && r.stdout.includes("[harness] epic not found"),
      `status=${r.status} stdout=${r.stdout.trim().slice(0, 120)} stderr=${(r.stderr ?? "").trim().slice(0, 120)}`,
    );
  }
} else if (mode === "live") {
  for (const cmd of commands) {
    const r = runHook(cmd);
    check(
      `live: ${cmd}`,
      r.status === 0,
      `status=${r.status} stderr=${(r.stderr ?? "").trim().slice(0, 200)}`,
    );
  }
} else if (mode === "mcp") {
  const initialize = JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "windows-acceptance", version: "0.0.0" },
    },
  });
  const r = spawnSync("epic", ["mem", "mcp"], {
    input: initialize + "\n",
    encoding: "utf8",
    timeout: 30_000,
  });
  check(
    "harness-mem mcp initialize",
    r.stdout.includes('"serverInfo"'),
    `status=${r.status} stdout=${r.stdout.trim().slice(0, 160)} stderr=${(r.stderr ?? "").trim().slice(0, 160)}`,
  );
} else {
  console.error(`usage: windows-acceptance.mjs <degrade|live|mcp>`);
  process.exit(64);
}

process.exitCode = failures ? 1 : 0;
