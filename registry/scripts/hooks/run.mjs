#!/usr/bin/env node
// Generic cross-platform hook launcher for the epic Codex plugin.
//
// Every hook in .codex-plugin/hooks.json routes through this script so the
// command strings stay shell-free: `node ${PLUGIN_ROOT}/registry/scripts/hooks/run.mjs <sub>`.
// Codex pipes hook JSON on stdin and expects the hook's stdout + exit code
// back, so we spawn `epic` with inherited stdio and propagate its exit code.
// When `epic` is not installed we degrade to a notice on stdout and exit 0 —
// a missing harness must never block the host session.
//
// `session-start` is the SessionStart special case: run the install.js
// bootstrap best-effort first, then `epic resume` — on every OS, without
// shell sequencing (`;` vs `&`).
//
// Node.js built-ins only, matching registry/scripts/install.js. The .mjs
// extension pins ESM semantics regardless of any package.json "type".

import { spawn, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PLUGIN_ROOT =
  process.env.PLUGIN_ROOT ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

const args = process.argv.slice(2);
if (args.length === 0) {
  // exitCode (not process.exit): stdout to a pipe is an async write, and a
  // piped notice must not be truncated by an early exit.
  console.log("[harness] run.mjs: no subcommand given");
  process.exitCode = 0;
} else {
  // Windows: probe for `epic` BEFORE spawning. spawn(shell:true) on a missing
  // command reports failure inconsistently across Node versions (cmd.exe 9009,
  // exit 1, or an error event) and leaks "'epic' is not recognized" to the
  // hook's stderr — measured on windows-latest, Node 22. `where.exe` ships
  // with every Windows and gives a deterministic answer.
  let epicMissing = false;
  if (process.platform === "win32") {
    const probe = spawnSync("where.exe", ["epic"], { encoding: "utf8" });
    epicMissing = probe.status !== 0;
  }

  if (epicMissing) {
    // No child is pending, so exitCode (not process.exit) lets the piped
    // notice flush before the event loop drains.
    console.log("[harness] epic not found");
    process.exitCode = 0;
  } else {
    if (args[0] === "session-start") {
      // Best-effort bootstrap; its failure must not block resume.
      await new Promise((resolve) => {
        const bootstrap = spawn(
          process.execPath,
          [path.join(PLUGIN_ROOT, "registry", "scripts", "install.js")],
          { stdio: "ignore" },
        );
        bootstrap.on("error", () => resolve());
        bootstrap.on("close", () => resolve());
      });
      args[0] = "resume";
    }

    // On Windows `epic` may be a .cmd shim (npm install); spawning .cmd files
    // without a shell throws EINVAL since the Node 18.20/20.12 security fix.
    // The args are static words (no user input), so a shell is safe there.
    // On POSIX spawn(3) resolves `epic` via PATH directly.
    const child = spawn("epic", args, {
      stdio: "inherit",
      shell: process.platform === "win32",
    });

    let spawnFailed = false;

    child.on("error", (err) => {
      // On spawn failure Node emits both `error` and `close` (close carries a
      // garbage exit code like -2 on some platforms), so this handler owns the
      // outcome and close must not override it. No child is pending here, so
      // setting exitCode (instead of process.exit) lets the pending stdout
      // write flush — a piped notice must not be truncated mid-write.
      spawnFailed = true;
      if (err.code === "ENOENT") {
        console.log("[harness] epic not found");
      } else {
        console.log(`[harness] ${err.message}`);
      }
      process.exitCode = 0;
    });

    child.on("close", (code, signal) => {
      if (spawnFailed) return;
      if (signal) {
        // Guard killed by a signal must not read as a pass.
        process.exit(1);
      }
      // Belt-and-suspenders for the win32 shell:true 9009 path (the where.exe
      // probe above should normally catch it first).
      if (process.platform === "win32" && code === 9009) {
        console.log("[harness] epic not found");
        process.exitCode = 0;
        return;
      }
      process.exit(code ?? 0);
    });
  }
}
