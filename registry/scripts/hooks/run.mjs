#!/usr/bin/env node
// Generic cross-platform hook launcher for the epic Claude, Codex, Grok, and
// Antigravity (agy) plugins.
//
// Every hook in .claude-plugin/hooks.json, .codex-plugin/hooks.json,
// .grok-plugin/hooks.json, and the root hooks.json (agy) routes through this
// script so the command strings stay shell-free:
// `node ${<host root>}/registry/scripts/hooks/run.mjs <sub>`
// (PLUGIN_ROOT / CLAUDE_PLUGIN_ROOT / GROK_PLUGIN_ROOT per host; agy uses a
// relative path and no variable). The host
// pipes hook JSON on stdin and expects the hook's stdout + exit code back, so
// we spawn `epic` with inherited stdio and propagate its exit code. When
// `epic` is not installed we degrade to a notice on stdout and exit 0 — a
// missing harness must never block the host session.
//
// `session-start` is the SessionStart special case: run the install.cjs
// bootstrap best-effort first, then `epic resume` — on every OS, without
// shell sequencing (`;` vs `&`).
//
// `agy-guard`, `agy-observe` and `agy-reflect` run `epic guard|observe|reflect`
// in agy mode, which holds two invariants on every path. A: stdout is empty,
// except that guard forwards one `{"decision":"deny","reason":...}` line when
// the child printed exactly such an object. B: the exit code is always 0.
// Why: agy reads any PreToolUse stdout object, even `{}`, as a deny, and any
// non-zero exit as a block of the tool call. Without this filter an older or
// crashing `epic` (which echoes stdin or prints `{}`) would block every shell
// command. Guard therefore fails open: a missing or broken `epic` allows.
//
// Node.js built-ins only, matching registry/scripts/install.cjs. The .mjs
// extension pins ESM semantics regardless of any package.json "type"; the
// bootstrap is .cjs for the same reason in mirror image — CommonJS regardless
// of "type" (require() inside an ESM-treated .js file throws ReferenceError).

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PLUGIN_ROOT =
  process.env.PLUGIN_ROOT ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

// Append (never prepend) `dir` to PATH so a binary installed there runs without
// shadowing other tools. Windows names the variable "Path" (or any casing) and
// process.env is case-insensitive there, but a plain env object is not: reuse
// the existing key so no second PATH/Path entry is ever created.
export function appendToPath(env, dir, platform = process.platform) {
  const win = platform === "win32";
  const key = (win && Object.keys(env).find((k) => k.toLowerCase() === "path")) || "PATH";
  env[key] = [env[key], dir].filter(Boolean).join(win ? path.win32.delimiter : path.posix.delimiter);
}

// The bootstrap keeps ~/.local/bin/epic at the plugin version, so hooks run it
// by absolute path instead of whatever older epic PATH resolves first.
export function localEpicPath(home, platform = process.platform, exists = existsSync) {
  const p = (platform === "win32" ? path.win32 : path.posix).join(
    home,
    ".local",
    "bin",
    platform === "win32" ? "epic.exe" : "epic",
  );
  return exists(p) ? p : null;
}

const args = process.argv.slice(2);

// `prompt` (UserPromptSubmit) stdout becomes model context on every prompt, so
// a missing harness stays silent there instead of injecting the notice. For the
// same reason every child failure (an older epic that does not know `prompt`,
// EACCES, a signal) ends in a silent exit 0 with nothing on stdout or stderr:
// the hook must never surface an error on the user's prompt.
const quiet = args[0] === "prompt";
// An explicit map, not an `agy-` prefix strip: a future `agy-session-start`
// must not silently drop resume's injectSteps.
const agySub = new Map([
  ["agy-guard", "guard"],
  ["agy-observe", "observe"],
  ["agy-reflect", "reflect"],
]).get(args[0]);
// agy reads stdout as a hook decision, so its notices go to stderr.
const epicNotFound = () => {
  if (agySub) console.error("[harness] epic not found");
  else if (!quiet) console.log("[harness] epic not found");
};
// The deny line agy may receive: only a JSON object with decision "deny".
const agyDenyLine = (text) => {
  try {
    const obj = JSON.parse(text.trim());
    if (obj === null || typeof obj !== "object" || Array.isArray(obj) || obj.decision !== "deny") return "";
    return JSON.stringify({ decision: "deny", reason: typeof obj.reason === "string" ? obj.reason : "" }) + "\n";
  } catch {
    return "";
  }
};
if (args.length === 0) {
  // exitCode (not process.exit): stdout to a pipe is an async write, and a
  // piped notice must not be truncated by an early exit.
  console.log("[harness] run.mjs: no subcommand given");
  process.exitCode = 0;
} else {
  // session-start: bootstrap first, on every OS, so a freshly installed
  // binary is found by the probe and spawn below.
  if (args[0] === "session-start") {
    // Best-effort bootstrap; its failure must not block resume. Stdout is
    // ignored (SessionStart stdout reaches the model context — bootstrap
    // noise must not be injected), but stderr is inherited so a failing
    // install.cjs is visible instead of silently skipped.
    await new Promise((resolve) => {
      const bootstrap = spawn(
        process.execPath,
        [path.join(PLUGIN_ROOT, "registry", "scripts", "install.cjs")],
        { stdio: ["ignore", "ignore", "inherit"] },
      );
      bootstrap.on("error", () => resolve());
      bootstrap.on("close", () => resolve());
    });
    args[0] = "resume";
  }
  if (agySub) args[0] = agySub;

  // Append (never prepend) the bootstrap install dir: a binary installed
  // there runs in this same session without shadowing other tools on PATH.
  const localBin = path.join(os.homedir(), ".local", "bin");
  if (existsSync(localBin)) appendToPath(process.env, localBin);

  // Windows: probe for `epic` BEFORE spawning. spawn(shell:true) on a missing
  // command reports failure inconsistently across Node versions (cmd.exe 9009,
  // exit 1, or an error event) and leaks "'epic' is not recognized" to the
  // hook's stderr — measured on windows-latest, Node 22. `where.exe` ships
  // with every Windows and gives a deterministic answer.
  const localEpic = localEpicPath(os.homedir());
  let epicMissing = false;
  if (!localEpic && process.platform === "win32") {
    // Absolute path: libuv does not search System32 itself, so a PATH without
    // it (a hook env that was trimmed down) would make the probe itself fail
    // and read as "epic not found".
    const where = process.env.SystemRoot
      ? path.join(process.env.SystemRoot, "System32", "where.exe")
      : "where.exe";
    const probe = spawnSync(where, ["epic"], { encoding: "utf8" });
    epicMissing = probe.status !== 0;
  }

  if (epicMissing) {
    // No child is pending, so exitCode (not process.exit) lets the piped
    // notice flush before the event loop drains.
    epicNotFound();
    process.exitCode = 0;
  } else {
    // On Windows `epic` may be a .cmd shim (npm install); spawning .cmd files
    // without a shell throws EINVAL since the Node 18.20/20.12 security fix.
    // The args are static words (no user input), so a shell is safe there.
    // On POSIX spawn(3) resolves `epic` via PATH directly.
    // An absolute .exe/binary needs no shell, even on Windows.
    const options = {
      stdio: "inherit",
      shell: !localEpic && process.platform === "win32",
    };
    // `prompt`: stdout carries the hook JSON, stderr (an older epic's
    // unknown-subcommand error) must never reach the host.
    if (quiet) options.stdio = ["inherit", "inherit", "ignore"];
    // agy: stdout is piped so the close handler can decide what (if anything) agy sees.
    if (agySub) options.stdio = ["inherit", "pipe", "inherit"];
    const child = spawn(localEpic || "epic", args, options);

    const chunks = [];
    if (agySub) child.stdout?.on("data", (c) => chunks.push(c));
    let spawnFailed = false;

    child.on("error", (err) => {
      // On spawn failure Node emits both `error` and `close` (close carries a
      // garbage exit code like -2 on some platforms), so this handler owns the
      // outcome and close must not override it. No child is pending here, so
      // setting exitCode (instead of process.exit) lets the pending stdout
      // write flush — a piped notice must not be truncated mid-write.
      spawnFailed = true;
      if (agySub) {
        // Fail open: nothing on stdout, exit 0 (a non-zero exit blocks the tool call).
        if (err.code === "ENOENT") epicNotFound();
        else console.error(`[harness] ${err.message}`);
        process.exitCode = 0;
      } else if (err.code === "ENOENT") {
        // Missing harness: degrade on stdout, exit 0.
        epicNotFound();
        process.exitCode = 0;
      } else if (quiet) {
        process.exitCode = 0;
      } else {
        // Present-but-broken install (EACCES, ...): surface on stderr and
        // fail the hook (exit 1 is a hook error, not a block) instead of
        // masquerading as "not installed".
        console.error(`[harness] ${err.message}`);
        process.exitCode = 1;
      }
    });

    child.on("close", (code, signal) => {
      if (spawnFailed) return;
      if (agySub) {
        // exitCode (not process.exit): the deny line is a piped write.
        process.exitCode = 0;
        if (process.platform === "win32" && code === 9009) epicNotFound();
        else if (agySub === "guard") {
          const line = agyDenyLine(Buffer.concat(chunks).toString("utf8"));
          if (line) process.stdout.write(line);
        }
        return;
      }
      if (quiet) {
        // Silent fail-open; a current epic already exits 0 for `prompt`.
        process.exit(0);
      }
      if (signal) {
        // Guard killed by a signal must not read as a pass.
        process.exit(1);
      }
      // Belt-and-suspenders for the win32 shell:true 9009 path (the where.exe
      // probe above should normally catch it first).
      if (process.platform === "win32" && code === 9009) {
        epicNotFound();
        process.exitCode = 0;
        return;
      }
      process.exit(code ?? 0);
    });
  }
}
