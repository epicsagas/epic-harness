/**
 * epic-harness OpenCode plugin
 *
 * Bridges OpenCode lifecycle events to the epic-harness binary so the same
 * Ring-0 automation (guard, observe, resume, reflect) works inside OpenCode.
 *
 * Requires: epic-harness binary available in PATH (install via cargo install
 * epic-harness or place the binary in ~/bin/).
 */

import { spawnSync, spawn } from "node:child_process";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Locate the epic-harness binary. Returns null if not found. */
function findBinary() {
  const which = spawnSync("command", ["-v", "epic-harness"], { shell: true });
  if (which.status === 0) return "epic-harness";
  // Fallback: common install locations
  for (const p of [
    `${process.env.HOME}/.cargo/bin/epic-harness`,
    "/usr/local/bin/epic-harness",
  ]) {
    const check = spawnSync("test", ["-x", p], { shell: true });
    if (check.status === 0) return p;
  }
  return null;
}

/**
 * Call epic-harness synchronously (blocks — use for guard/resume).
 * Returns { status, stdout, stderr }.
 */
function callSync(bin, subCmd, hookInput) {
  const result = spawnSync(bin, [subCmd], {
    input: JSON.stringify(hookInput),
    encoding: "utf8",
    timeout: 10_000,
  });
  return {
    status: result.status ?? -1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

/**
 * Call epic-harness in the background (fire-and-forget).
 * Failures are silently ignored so the user's workflow is never blocked.
 */
function callAsync(bin, subCmd, hookInput) {
  try {
    const child = spawn(bin, [subCmd], {
      detached: true,
      stdio: ["pipe", "ignore", "ignore"],
    });
    child.stdin.write(JSON.stringify(hookInput));
    child.stdin.end();
    child.unref();
  } catch {
    // Intentionally silent — never break user workflow
  }
}

/** Build a minimal HookInput object from OpenCode event data. */
function buildHookInput({ toolName, toolInput, toolOutput } = {}) {
  return {
    tool_name: toolName ?? "",
    tool_input: toolInput ?? {},
    tool_output: toolOutput ?? {},
    conversation_summary: null,
    pending_tasks: [],
    context_usage: null,
  };
}

// ── Shell-tool detection ──────────────────────────────────────────────────────

const SHELL_TOOL_NAMES = new Set([
  "bash",
  "shell",
  "run",
  "execute",
  "terminal",
  "cmd",
  "command",
]);

function isShellTool(toolName) {
  return SHELL_TOOL_NAMES.has((toolName ?? "").toLowerCase());
}

// ── Plugin export ─────────────────────────────────────────────────────────────

export default {
  name: "epic-harness",
  version: "1.0.0",

  hooks: {
    /**
     * session.created — restore context from previous session.
     * Blocks briefly; resume is fast (~50 ms typical).
     */
    "session.created": async (event) => {
      try {
        const bin = findBinary();
        if (!bin) return;
        const input = buildHookInput({
          toolName: "session.created",
          toolInput: { session_id: event?.sessionId ?? "" },
        });
        callSync(bin, "resume", input);
      } catch {
        // Silent — never break session start
      }
    },

    /**
     * tool.execute.before — run guard for shell-like tools.
     * Return { abort: true, reason: "..." } to block execution.
     */
    "tool.execute.before": async (event) => {
      try {
        const bin = findBinary();
        if (!bin) return;

        const toolName = event?.tool?.name ?? event?.toolName ?? "";
        if (!isShellTool(toolName)) return;

        const input = buildHookInput({
          toolName,
          toolInput: event?.tool?.input ?? event?.toolInput ?? {},
        });

        const result = callSync(bin, "guard", input);
        if (result.status === 2) {
          // Guard blocked the command
          return {
            abort: true,
            reason: result.stderr.trim() || "Blocked by epic-harness guard",
          };
        }
      } catch {
        // Silent — never block on error
      }
    },

    /**
     * tool.execute.after — record observation (async, non-blocking).
     */
    "tool.execute.after": async (event) => {
      try {
        const bin = findBinary();
        if (!bin) return;

        const toolName = event?.tool?.name ?? event?.toolName ?? "";
        const input = buildHookInput({
          toolName,
          toolInput: event?.tool?.input ?? event?.toolInput ?? {},
          toolOutput: {
            output: event?.result?.output ?? event?.output ?? "",
            stderr: event?.result?.stderr ?? event?.stderr ?? "",
          },
        });

        callAsync(bin, "observe", input);
      } catch {
        // Silent
      }
    },

    /**
     * session.idle — run reflect to evolve skills between interactions.
     * Fire-and-forget; reflect may take a few seconds.
     */
    "session.idle": async (_event) => {
      try {
        const bin = findBinary();
        if (!bin) return;

        const input = buildHookInput({ toolName: "session.idle" });
        callAsync(bin, "reflect", input);
      } catch {
        // Silent
      }
    },
  },
};
