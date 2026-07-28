import assert from "node:assert/strict";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";

const SCRIPT = fileURLToPath(new URL("./install.js", import.meta.url));

for (const [environmentKey, manifestDir] of [
  ["PLUGIN_ROOT", ".codex-plugin"],
  ["CLAUDE_PLUGIN_ROOT", ".claude-plugin"],
]) {
  test(`${environmentKey} plugin bootstrap is silent and skips obsolete seeding`, () => {
    const root = mkdtempSync(join(tmpdir(), "epic-harness-install-test-"));

    try {
      const bin = join(root, "bin");
      const calls = join(root, "calls.txt");
      const probes = join(root, "probes.txt");
      mkdirSync(join(root, manifestDir), { recursive: true });
      mkdirSync(bin, { recursive: true });
      writeFileSync(
        join(root, manifestDir, "plugin.json"),
        JSON.stringify({ version: "0.8.2" }),
      );

      const stub = join(bin, "epic-harness");
      writeFileSync(
        stub,
        `#!/bin/sh
if [ "$1" = "version" ]; then
  printf '%s\\n' 'version' >> "$EPIC_TEST_PROBES"
  printf '%s\\n' 'epic-harness 0.8.2'
  exit 0
fi
printf '%s\\n' "$*" >> "$EPIC_TEST_CALLS"
`,
      );
      chmodSync(stub, 0o755);

      const result = spawnSync(process.execPath, [SCRIPT], {
        encoding: "utf8",
        env: {
          ...process.env,
          CLAUDE_PLUGIN_ROOT: "",
          PLUGIN_ROOT: "",
          [environmentKey]: root,
          EPIC_TEST_CALLS: calls,
          EPIC_TEST_PROBES: probes,
          PATH: `${bin}:${process.env.PATH ?? ""}`,
        },
      });

      assert.equal(result.status, 0, result.stderr);
      assert.equal(
        result.stdout,
        "",
        "SessionStart bootstrap must not write stdout",
      );
      assert.equal(result.stderr, "", "an up-to-date plugin must be quiet");

      let invocations = "";
      try {
        invocations = readFileSync(calls, "utf8");
      } catch {}
      assert.equal(
        invocations,
        "",
        "plugin bootstrap must not invoke the removed `install claude` command",
      );
      assert.equal(
        readFileSync(probes, "utf8"),
        "version\nversion\n",
        "the plugin manifest must trigger a binary-version comparison",
      );
    } finally {
      rmSync(root, { force: true, recursive: true });
    }
  });
}
