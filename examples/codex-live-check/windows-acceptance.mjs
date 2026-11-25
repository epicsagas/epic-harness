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
//   claude-shell <degrade|live> — the quoted Claude commands run through the
//             shells Claude uses, from a plugin root whose name has a space,
//             `%EPIC_QA%` and `&`: Git Bash and pwsh on Windows, `sh -c`
//             elsewhere. Emulates the documented host behavior (textual
//             ${CLAUDE_PLUGIN_ROOT} substitution; ${env:CLAUDE_PLUGIN_ROOT}
//             for PowerShell); a real Claude install is checked separately.

import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import process from "node:process";

const root = process.env.PLUGIN_ROOT ?? process.cwd();
const mode = process.argv[2] ?? "";

const manifest = JSON.parse(
  readFileSync(path.join(root, ".codex-plugin", "hooks.json"), "utf8"),
);

// Codex substitutes ${PLUGIN_ROOT} inline and hands the command to a shell
// (cmd.exe /C on Windows, openai/codex#32402). The manifest commands are
// pinned shell-free (`node ${PLUGIN_ROOT}/... <sub>`, enforced by
// tests/plugin_assets_test.rs), so the driver resolves them to argv and
// spawns directly — no shell string, so a PLUGIN_ROOT with spaces or
// metacharacters cannot break or inject into the command (CodeQL
// js/shell-command-injection-from-environment). An unrecognized shape is a
// hard error: the driver must never silently skip a manifest command.
const hooks = Object.values(manifest.hooks)
  .flat()
  .flatMap((group) => group.hooks)
  .filter((handler) => handler.type === "command")
  .map((handler) => {
    const m = handler.command.match(
      /^node\s+\$\{PLUGIN_ROOT\}\/(\S+)\s+(\S+)$/,
    );
    if (!m) throw new Error(`unrecognized hook command: ${handler.command}`);
    return { raw: handler.command, script: m[1], sub: m[2] };
  });

let failures = 0;
const check = (name, ok, extra = "") => {
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${extra ? ` — ${extra}` : ""}`);
  if (!ok) failures++;
};

// Same effective command Codex runs, resolved as argv instead of a shell
// string (see above).
const runHook = (hook) =>
  spawnSync("node", [path.join(root, hook.script), hook.sub], {
    input: "{}",
    encoding: "utf8",
    timeout: 60_000,
    env: { ...process.env, PLUGIN_ROOT: root },
  });

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
      protocolVersion: "<PHONE_NUMBER>",
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
    r.status === 0 && r.stdout.includes('"serverInfo"'),
    `status=${r.status} stdout=${r.stdout.trim().slice(0, 160)} stderr=${(r.stderr ?? "").trim().slice(0, 160)}`,
  );
} else if (mode === "claude-shell") {
  const phase = process.argv[3];
  if (phase !== "degrade" && phase !== "live") {
    console.error("usage: windows-acceptance.mjs claude-shell <degrade|live>");
    process.exit(64);
  }
  const claude = JSON.parse(
    readFileSync(path.join(root, ".claude-plugin", "hooks.json"), "utf8"),
  );
  // Exact quoted form only; anything else is a hard error, never a skip.
  const claudeHooks = Object.values(claude.hooks)
    .flat()
    .flatMap((group) => group.hooks)
    .filter((handler) => handler.type === "command")
    .map((handler) => {
      const m = handler.command.match(
        /^node "\$\{CLAUDE_PLUGIN_ROOT\}\/registry\/scripts\/hooks\/run\.mjs" (\S+)$/,
      );
      if (!m) throw new Error(`unrecognized hook command: ${handler.command}`);
      return { raw: handler.command, sub: m[1] };
    });

  const tmp = mkdtempSync(path.join(os.tmpdir(), "epic-claude-shell-"));
  try {
    const pluginDir = path.join(tmp, "epic qa %EPIC_QA% & x");
    const script = path.join("registry", "scripts", "hooks", "run.mjs");
    mkdirSync(path.dirname(path.join(pluginDir, script)), { recursive: true });
    copyFileSync(path.join(root, script), path.join(pluginDir, script));

    // Textual ${CLAUDE_PLUGIN_ROOT} substitution (bash family).
    const textual = (cmd) =>
      cmd.replaceAll("${CLAUDE_PLUGIN_ROOT}", () => pluginDir.replaceAll("\\", "/"));
    // PLUGIN_ROOT must not leak in: run.mjs would resolve install.cjs from it.
    const { PLUGIN_ROOT: _unused, ...baseEnv } = process.env;
    // A cmd.exe-style expansion of %EPIC_QA% would corrupt the path.
    // CLAUDE_PROJECT_DIR is what Claude exports to hooks; the binary uses it to
    // recognize a Claude PreToolUse call.
    const env = { ...baseEnv, EPIC_QA: "expanded", CLAUDE_PROJECT_DIR: tmp };
    const shells =
      process.platform === "win32"
        ? [
            {
              name: "git-bash",
              file: "C:\\Program Files\\Git\\bin\\bash.exe",
              args: (cmd) => ["-c", textual(cmd)],
              env,
            },
            {
              name: "pwsh",
              file: "pwsh",
              args: (cmd) => [
                "-NoProfile",
                "-Command",
                cmd.replaceAll("${CLAUDE_PLUGIN_ROOT}", "${env:CLAUDE_PLUGIN_ROOT}"),
              ],
              env: { ...env, CLAUDE_PLUGIN_ROOT: pluginDir },
            },
          ]
        : [{ name: "sh", file: "sh", args: (cmd) => ["-c", textual(cmd)], env }];

    const runShell = (shell, hook, input) =>
      spawnSync(shell.file, shell.args(hook.raw), {
        input,
        encoding: "utf8",
        timeout: 60_000,
        env: shell.env,
      });
    const detail = (r) =>
      `status=${r.status} stdout=${(r.stdout ?? "").trim().slice(0, 120)} stderr=${(r.stderr ?? "").trim().slice(0, 120)}${r.error ? ` error=${r.error.message}` : ""}`;

    // The temp root holds only run.mjs, so the install.cjs bootstrap that
    // session-start runs first is not available here.
    console.log("NOTE session-start is skipped in claude-shell mode (needs install.cjs)");
    for (const shell of shells) {
      for (const hook of claudeHooks) {
        if (hook.sub === "session-start") continue;
        const r = runShell(shell, hook, "{}");
        const ok =
          r.status === 0 &&
          (phase === "live" || r.stdout.includes("[harness] epic not found"));
        check(`claude-shell ${shell.name} ${phase}: ${hook.raw}`, ok, detail(r));
      }
      if (phase === "live") {
        // On Claude a guard block is exit 0 plus one PreToolUse deny object,
        // not exit 2: `pwsh -Command` maps external exit codes other than 0/1
        // to 1, which Claude reads as a non-blocking error. A mismatch is
        // reported, never masked.
        const guard = claudeHooks.find((h) => h.sub === "guard");
        const r = runShell(
          shell,
          guard,
          JSON.stringify({
            hook_event_name: "PreToolUse",
            tool_name: "Bash",
            tool_input: { command: "rm -rf /" },
          }),
        );
        let decision;
        try {
          decision = JSON.parse(r.stdout).hookSpecificOutput?.permissionDecision;
        } catch {
          decision = undefined;
        }
        const ok = r.status === 0 && decision === "deny";
        check(
          `claude-shell ${shell.name} live: guard block is exit 0 with a deny decision`,
          ok,
          ok
            ? detail(r)
            : `${shell.name} returned exit ${r.status} with permissionDecision=${decision} for a blocked command, expected exit 0 and "deny" (blocking contract broken in this shell); ${detail(r)}`,
        );
      }
    }
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
} else {
  console.error(`usage: windows-acceptance.mjs <degrade|live|mcp|claude-shell>`);
  process.exit(64);
}

process.exitCode = failures ? 1 : 0;
