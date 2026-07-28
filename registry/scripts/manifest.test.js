import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const CODEX_PATH = join(ROOT, ".codex-plugin", "hooks.json");
const CLAUDE_PATH = join(ROOT, "hooks", "hooks.json");
const CODEX = JSON.parse(readFileSync(CODEX_PATH, "utf8"));
const CLAUDE = JSON.parse(readFileSync(CLAUDE_PATH, "utf8"));
const CODEX_CONTRACT = [
  ["SessionStart", [["*", "resume"]]],
  ["PreToolUse", [["Bash", "guard"], ["apply_patch", "guard"]]],
  ["PostToolUse", [["*", "observe"], ["apply_patch|Edit|Write", "polish"]]],
  ["SubagentStart", [["*", "observe"]]],
  ["SubagentStop", [["*", "observe"]]],
  ["PreCompact", [["*", "snapshot"]]],
  ["SessionEnd", [["*", "reflect", { timeout: 3 }]]],
];

const CLAUDE_CONTRACT = [
  ["SessionStart", [["*", "resume", { async: false }]]],
  [
    "PreToolUse",
    [
      ["Bash", "guard"],
      ["Agent", "observe", {}, "SubagentStart"],
      ["Edit|Write|MultiEdit|NotebookEdit", "guard"],
    ],
  ],
  [
    "PostToolUse",
    [
      ["Edit|Write|MultiEdit|NotebookEdit", "polish"],
      ["*", "observe", { async: true, timeout: 5 }],
    ],
  ],
  ["PreCompact", [["*", "snapshot"]]],
  ["SessionEnd", [["*", "reflect"]]],
];

function handlers(manifest) {
  return Object.entries(manifest.hooks).flatMap(([event, groups]) =>
    groups.flatMap((group) =>
      group.hooks.map((handler) => ({ event, group, handler })),
    ),
  );
}

test("matcher groups contain only supported matcher and hooks fields", () => {
  for (const [name, manifest] of [
    ["Codex", CODEX],
    ["Claude", CLAUDE],
  ]) {
    for (const [event, groups] of Object.entries(manifest.hooks)) {
      for (const group of groups) {
        assert.deepEqual(
          Object.keys(group).sort(),
          ["hooks", "matcher"],
          `${name} ${event} has unsupported matcher-group fields`,
        );
      }
    }
  }
});

test("Codex uses the Node runner and Windows status wrapper for every hook", () => {
  for (const { event, handler } of handlers(CODEX)) {
    assert.equal(handler.type, "command");
    assert.match(
      handler.command,
      /^node "\$\{PLUGIN_ROOT\}\/registry\/scripts\/install\.js" hook /,
      `${event} must quote PLUGIN_ROOT and use the Node runner`,
    );
    assert.match(
      handler.commandWindows,
      /^cmd\.exe \/d \/s \/c call "%PLUGIN_ROOT%\\registry\\scripts\\run-hook\.cmd" /,
      `${event} must define a Windows status-preserving wrapper`,
    );
    assert.doesNotMatch(handler.command, /(?:^|[;&|])\s*epic(?:-harness)?\b/);
    assert.doesNotMatch(
      handler.commandWindows,
      /(?:^|[;&|])\s*epic(?:-harness)?\b/,
    );
  }
});

test("Claude uses the same quoted Node runner and binary contract", () => {
  for (const { event, handler } of handlers(CLAUDE)) {
    assert.match(
      handler.command,
      /^node "\$\{CLAUDE_PLUGIN_ROOT\}\/registry\/scripts\/install\.js" hook /,
      `${event} must quote CLAUDE_PLUGIN_ROOT and use the Node runner`,
    );
    assert.doesNotMatch(handler.command, /(?:^|[;&|])\s*epic(?:-harness)?\b/);
  }
});

function assertManifestContract(name, manifest, contract, rootVariable) {
  assert.deepEqual(
    Object.keys(manifest.hooks),
    contract.map(([event]) => event),
    `${name} event order changed`,
  );

  for (const [event, expectedGroups] of contract) {
    const groups = manifest.hooks[event];
    assert.equal(groups.length, expectedGroups.length, `${name} ${event} group count`);

    for (const [index, [matcher, subcommand, properties = {}, runnerEvent = event]] of expectedGroups.entries()) {
      const group = groups[index];
      assert.equal(group.matcher, matcher, `${name} ${event} matcher ${index}`);
      assert.equal(group.hooks.length, 1, `${name} ${event} handler count ${index}`);

      const handler = group.hooks[0];
      assert.equal(handler.type, "command", `${name} ${event} handler type ${index}`);
      assert.equal(
        handler.command,
        `node "\${${rootVariable}}/registry/scripts/install.js" hook ${runnerEvent} ${subcommand}`,
        `${name} ${event} command ${index}`,
      );
      if (name === "Codex") {
        assert.equal(
          handler.commandWindows,
          `cmd.exe /d /s /c call "%${rootVariable}%\\registry\\scripts\\run-hook.cmd" ${runnerEvent} ${subcommand}`,
          `${name} ${event} Windows command ${index}`,
        );
      }

      for (const [key, value] of Object.entries(properties)) {
        assert.equal(handler[key], value, `${name} ${event} ${key} ${index}`);
      }
      const expectedKeys = [
        "command",
        "type",
        ...(name === "Codex" ? ["commandWindows"] : []),
        ...Object.keys(properties),
      ].sort();
      assert.deepEqual(
        Object.keys(handler).sort(),
        expectedKeys,
        `${name} ${event} handler properties ${index}`,
      );
    }
  }
}

test("Codex manifest has the exact lifecycle matcher, handler, and timeout contract", () => {
  assertManifestContract("Codex", CODEX, CODEX_CONTRACT, "PLUGIN_ROOT");
});

test("Claude manifest has the exact lifecycle matcher, handler, and timeout contract", () => {
  assertManifestContract("Claude", CLAUDE, CLAUDE_CONTRACT, "CLAUDE_PLUGIN_ROOT");
});

test("manifest component paths resolve inside the plugin root", () => {
  for (const path of [
    JSON.parse(
      readFileSync(join(ROOT, ".codex-plugin", "plugin.json"), "utf8"),
    ).hooks,
    "./skills/",
    "./mcp_config.json",
  ]) {
    const resolved = resolve(ROOT, path);
    assert.ok(
      resolved === ROOT ||
        resolved.startsWith(
          `${ROOT}${process.platform === "win32" ? "\\" : "/"}`,
        ),
      `${path} escapes the plugin root`,
    );
    assert.ok(existsSync(resolved), `${path} does not exist`);
  }
});

test("all package and plugin version owners agree", () => {
  const versions = new Map([
    [
      "Cargo.toml",
      readFileSync(join(ROOT, "Cargo.toml"), "utf8").match(
        /^\[package\][\s\S]*?^version = "([^"]+)"/m,
      )?.[1],
    ],
    [
      "Cargo.lock",
      readFileSync(join(ROOT, "Cargo.lock"), "utf8").match(
        /\[\[package\]\]\r?\nname = "epic-harness"\r?\nversion = "([^"]+)"/,
      )?.[1],
    ],
    [
      "package.json",
      JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).version,
    ],
    [
      "plugin.json",
      JSON.parse(readFileSync(join(ROOT, "plugin.json"), "utf8")).version,
    ],
    [
      "app/package.json",
      JSON.parse(readFileSync(join(ROOT, "app", "package.json"), "utf8"))
        .version,
    ],
    [
      ".codex-plugin/plugin.json",
      JSON.parse(
        readFileSync(join(ROOT, ".codex-plugin", "plugin.json"), "utf8"),
      ).version,
    ],
    [
      ".claude-plugin/plugin.json",
      JSON.parse(
        readFileSync(join(ROOT, ".claude-plugin", "plugin.json"), "utf8"),
      ).version,
    ],
  ]);

  assert.equal(new Set(versions.values()).size, 1, JSON.stringify([...versions]));
});

test("canonical runtime revision is a positive integer", () => {
  const revision = readFileSync(join(ROOT, "runtime-revision.txt"), "utf8");
  assert.match(revision, /^[1-9]\d*\r?\n$/);
});

test("runtime changes after a release require a new package version", (t) => {
  const version = JSON.parse(
    readFileSync(join(ROOT, "package.json"), "utf8"),
  ).version;
  const tag = `v${version}`;
  const tagExists = spawnSync("git", ["rev-parse", "--verify", tag], {
    cwd: ROOT,
    encoding: "utf8",
  });
  if (tagExists.status !== 0) {
    t.skip(`${tag} is not a release tag yet`);
    return;
  }

  const runtimePaths = [
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src",
  ];
  const diff = spawnSync("git", ["diff", "--quiet", tag, "--", ...runtimePaths], {
    cwd: ROOT,
    encoding: "utf8",
  });
  assert.equal(
    diff.status,
    0,
    `${tag} exists, but runtime inputs changed without a version bump`,
  );
});

test("CI runs manifest and bootstrap contracts on Linux, macOS, and Windows", () => {
  const workflow = readFileSync(
    join(ROOT, ".github", "workflows", "ci.yml"),
    "utf8",
  );
  assert.match(workflow, /^\s*workflow_dispatch:\s*$/m);
  assert.match(workflow, /plugin-contract:/);
  assert.match(
    workflow,
    /os:\s*\[ubuntu-latest,\s*macos-latest,\s*windows-latest\]/,
  );
  assert.match(workflow, /fetch-depth:\s*0/);
  assert.match(
    workflow,
    /node --test registry\/scripts\/install\.test\.js registry\/scripts\/manifest\.test\.js/,
  );
});

test("CI runs the frozen dashboard install, checks, tests, build, and asset comparison", () => {
  const workflow = readFileSync(
    join(ROOT, ".github", "workflows", "ci.yml"),
    "utf8",
  );

  assert.match(workflow, /working-directory:\s*app\s*\n\s*run:\s*pnpm install --frozen-lockfile/);
  assert.match(workflow, /working-directory:\s*app\s*\n\s*run:\s*pnpm run check/);
  assert.match(workflow, /working-directory:\s*app\s*\n\s*run:\s*pnpm exec vitest run/);
  assert.match(workflow, /working-directory:\s*app\s*\n\s*run:\s*pnpm run build/);
  assert.match(workflow, /cmp -s app\/dist\/index\.html assets\/dashboard\.html/);
});

test("Cargo builds the checked-in dashboard asset without frontend tools or source writes", () => {
  const buildScript = readFileSync(join(ROOT, "build.rs"), "utf8");

  assert.doesNotMatch(buildScript, /Command::new\("pnpm"\)/);
  assert.doesNotMatch(buildScript, /pnpm install|node_modules|dist\/index\.html/);
  assert.doesNotMatch(buildScript, /SKIP_DASHBOARD_BUILD/);
  assert.doesNotMatch(buildScript, /fs::copy/);
  assert.doesNotMatch(buildScript, /cargo:rerun-if-changed=app\//);
});

test("Orbit dismissal uses one exact project-scoped backend", () => {
  const command = readFileSync(
    join(ROOT, "src-tauri", "src", "commands", "harness.rs"),
    "utf8",
  );
  const tauri = readFileSync(
    join(ROOT, "src-tauri", "src", "lib.rs"),
    "utf8",
  );
  const frontend = readFileSync(
    join(ROOT, "app", "src", "lib", "harness.ts"),
    "utf8",
  );

  assert.match(command, /pub async fn dismiss_orbit\s*\(/);
  assert.match(command, /resolve_external_harness_dir\(&project\)/);
  assert.match(command, /dismiss_pipeline_state_pool\s*\(/);
  assert.match(tauri, /commands::harness::dismiss_orbit/);
  assert.match(frontend, /invoke<[^>]+>\('dismiss_orbit',\s*\{\s*project,\s*id\s*\}\)/);
  assert.match(frontend, /\/api\/orbit\/\$\{encodeURIComponent\(id\)\}\?project=\$\{encodeURIComponent\(project\)\}/);
  assert.match(frontend, /project === '__all__'/);
});

test("Orbit completion requires concrete PR and green CI evidence", () => {
  const orbit = readFileSync(join(ROOT, "skills", "orbit", "SKILL.md"), "utf8");

  assert.match(orbit, /"ci_status": null/);
  assert.match(orbit, /`pr_url` is a nonempty concrete GitHub pull-request URL/);
  assert.match(orbit, /`ci_status` is exactly `"success"`/);
  assert.match(orbit, /CI failure[\s\S]*"ci_status": "failed"[\s\S]*STOP/);
  assert.match(orbit, /Only successful CI proceeds to Step 7/);
  assert.doesNotMatch(orbit, /phase_history.*ship.*status: complete/);
  assert.doesNotMatch(orbit, /Always run.*regardless of CI outcome/);
  assert.doesNotMatch(orbit, /evolve must always run, even if CI fails/);
});

test("all README translations reject removed hook and session contracts", () => {
  const readmes = [
    join(ROOT, "README.md"),
    ...readdirSync(join(ROOT, "i18n"), { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => join(ROOT, "i18n", entry.name, "README.md"))
      .filter(existsSync),
  ];
  const stalePatterns = [
    /session_\{date\}_\{pid\}_\{random\}\.jsonl/,
    /plugin_hooks/,
    /hooks\/bin\/epic-harness/,
  ];

  for (const path of readmes) {
    const content = readFileSync(path, "utf8");
    for (const pattern of stalePatterns) {
      assert.doesNotMatch(content, pattern, path);
    }
  }
});

test("runtime owners do not retain the removed bundled hook binary", () => {
  for (const path of ["Makefile", "Cargo.toml", "AGENTS.md", "src/update.rs"]) {
    assert.doesNotMatch(
      readFileSync(join(ROOT, path), "utf8"),
      /hooks\/bin\/epic-harness/,
      path,
    );
  }
});

test("npm package includes the hook runners and canonical runtime revision", () => {
  const result = spawnSync("npm", ["pack", "--dry-run", "--json"], {
    cwd: ROOT,
    encoding: "utf8",
    shell: process.platform === "win32",
  });
  assert.equal(result.status, 0, result.stderr);

  const packed = JSON.parse(result.stdout);
  const files = new Set(packed[0]?.files?.map((file) => file.path));
  for (const path of [
    "registry/scripts/install.js",
    "registry/scripts/run-hook.cmd",
    "runtime-revision.txt",
  ]) {
    assert.ok(files.has(path), `${path} is missing from the npm artifact`);
  }
});
