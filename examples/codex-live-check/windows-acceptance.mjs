#!/usr/bin/env node
// Behavioral acceptance for the Codex hook + MCP surfaces (#131/#132) on a
// real host shell. Static shell-free checks live in Rust
// (tests/plugin_assets_test.rs); this script proves the shipped commands
// actually EXECUTE.
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

import { readFileSync, mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import process from "node:process";

const root = process.env.PLUGIN_ROOT ?? process.cwd();
const mode = process.argv[2] ?? "";

const manifest = JSON.parse(
  readFileSync(path.join(root, ".codex-plugin", "hooks.json"), "utf8"),
);

// Exercise the real host shell; argv bypass would hide command-expansion bugs.
const hooks = Object.values(manifest.hooks)
  .flat()
  .flatMap((group) => group.hooks)
  .filter((handler) => handler.type === "command")
  .map((handler) => {
    const m = handler.command.match(
      /^node\s+(?:"\$\{PLUGIN_ROOT\}\/([^"\r\n]+)"|\$\{PLUGIN_ROOT\}\/(\S+))\s+(\S+)$/,
    );
    if (!m) throw new Error(`unrecognized hook command: ${handler.command}`);
    return { raw: handler.command, script: m[1] ?? m[2], sub: m[3] };
  });

let failures = 0;
const check = (name, ok, extra = "") => {
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${extra ? ` — ${extra}` : ""}`);
  if (!ok) failures++;
};

// Each acceptance invocation owns an isolated data/home root.
const fixture = mkdtempSync(path.join(tmpdir(), "epic-release-acceptance-"));
const home = path.join(fixture, "home"), data = path.join(fixture, "data");
mkdirSync(path.join(home, ".config/epic-harness"), { recursive: true });
mkdirSync(data);
writeFileSync(path.join(home, ".config/epic-harness/telemetry-consent"), "off");
writeFileSync(path.join(data, "config.toml"), "[dashboard]\nport=0\nauto_open=false\n");
const childEnv = { ...process.env, HOME: home, USERPROFILE: home, HARNESS_DIR: data, HARNESS_ROOT: fixture, PLUGIN_ROOT: root };
Object.assign(childEnv, { XDG_CONFIG_HOME: path.join(home, '.config'), APPDATA: path.join(home, 'AppData/Roaming'), LOCALAPPDATA: path.join(home, 'AppData/Local') });
for (const variable of ["HARNESS_DB_URL", "HARNESS_MEM_URL", "DATABASE_URL"]) delete childEnv[variable];
process.on("exit", () => {
  if (!path.resolve(fixture).startsWith(path.resolve(tmpdir()) + path.sep)) throw new Error("invalid acceptance cleanup root");
  rmSync(fixture, { recursive: true, force: true });
});
const runHook = (hook) => {
  const command = hook.raw.replaceAll("\u0024{PLUGIN_ROOT}", root);
  const options = { input: "{}", encoding: "utf8", timeout: 60000, env: childEnv, windowsHide: true };
  return process.platform === "win32"
    ? spawnSync("cmd.exe", ["/D", "/S", "/C", command], { ...options, windowsVerbatimArguments: true })
    : spawnSync("/bin/sh", ["-c", command], options);
};

if (mode === "degrade") {
  for (const hook of hooks) {
    if (hook.sub === "session-start") continue; // the installer; runs in `live`
    const r = runHook(hook);
    check(
      `degrade: ${hook.raw}`,
      r.status === 0 && r.stdout.includes("[harness] epic not found"),
      `status=${r.status} stdout=${r.stdout.trim().slice(0, 120)} stderr=${(r.stderr ?? "").trim().slice(0, 120)}`,
    );
  }
} else if (mode === "live") {
  for (const hook of hooks) {
    const r = runHook(hook);
    check(
      `live: ${hook.raw}`,
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
    env: childEnv,
    windowsHide: true,
  });
  check(
    "harness-mem mcp initialize",
    r.status === 0 && r.stdout.includes('"serverInfo"'),
    `status=${r.status} stdout=${r.stdout.trim().slice(0, 160)} stderr=${(r.stderr ?? "").trim().slice(0, 160)}`,
  );
} else {
  console.error(`usage: windows-acceptance.mjs <degrade|live|mcp>`);
  process.exit(64);
}

process.exitCode = failures ? 1 : 0;
