#!/usr/bin/env node
/**
 * Ring 0: guard — PreToolUse(Bash)
 *
 * Block dangerous commands. Exit 2 = blocked.
 * Supports custom rules from .harness/guard-rules.yaml (#8)
 */
import { existsSync, readFileSync } from "node:fs";
import {
  runGuardHook, hint, harnessExists,
  GUARD_RULES_FILE, parseSimpleYaml,
  type HookInput, type Rule,
} from "./common.js";

export const BLOCKED: Rule[] = [
  { pattern: /git\s+push\s+.*--force\s+(origin\s+)?(main|master)\b/, msg: "Force push to main/master blocked" },
  { pattern: /rm\s+-rf\s+\/(?!\w)/, msg: "rm -rf / blocked" },
  { pattern: /DROP\s+(DATABASE|TABLE)\s+.*prod/i, msg: "DROP on production DB blocked" },
];

export const WARNED: Rule[] = [
  { pattern: /git\s+push\s+.*--force/, msg: "Force push — ensure this is intentional" },
  { pattern: /git\s+reset\s+--hard/, msg: "Hard reset will discard local changes" },
  { pattern: /rm\s+-rf\s+/, msg: "Recursive delete — double-check the path" },
];

/** Load custom rules from .harness/guard-rules.yaml */
function loadCustomRules(): { blocked: Rule[]; warned: Rule[] } {
  if (!harnessExists() || !existsSync(GUARD_RULES_FILE)) {
    return { blocked: [], warned: [] };
  }
  try {
    const content = readFileSync(GUARD_RULES_FILE, "utf8");
    return parseSimpleYaml(content);
  } catch {
    return { blocked: [], warned: [] };
  }
}

const isMain = import.meta.url === `file://${process.argv[1]}`;
if (isMain) runGuardHook((input: HookInput) => {
  const cmd = (input.tool_input?.command as string) || "";
  if (!cmd) return;

  // Merge built-in + custom rules
  const custom = loadCustomRules();
  const allBlocked = [...BLOCKED, ...custom.blocked];
  const allWarned = [...WARNED, ...custom.warned];

  for (const rule of allBlocked) {
    if (rule.pattern.test(cmd)) {
      hint("guard", `BLOCKED: ${rule.msg}`);
      process.exit(2);
    }
  }

  for (const rule of allWarned) {
    if (rule.pattern.test(cmd)) {
      hint("guard", `WARNING: ${rule.msg}`);
    }
  }
});
