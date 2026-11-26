// node --test registry/scripts
// Copies run.mjs into a temp plugin layout next to a fake install.cjs, so the
// real bootstrap (and the network) is never touched.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const win = process.platform === "win32";
const runMjs = path.join(path.dirname(fileURLToPath(import.meta.url)), "run.mjs");
const scratch = mkdtempSync(path.join(os.tmpdir(), "epic-run-test-"));
after(() => rmSync(scratch, { recursive: true, force: true }));
let n = 0;

const shimName = win ? "epic.cmd" : "epic";
const shimBody = win ? "@echo off\r\necho shim-ran %1\r\n" : '#!/bin/sh\necho "shim-ran $1"\n';

function writeShim(dir, body = shimBody) {
  mkdirSync(dir, { recursive: true });
  const p = path.join(dir, shimName);
  writeFileSync(p, body);
  chmodSync(p, 0o755);
}

// fakeBootstrap: JS source of the stand-in install.cjs; it runs with the temp ~/.local/bin as argv[2].
function layout(fakeBootstrap) {
  const base = path.join(scratch, `case-${n++}`);
  const root = path.join(base, "plugin");
  const home = path.join(base, "home");
  mkdirSync(path.join(root, "registry", "scripts", "hooks"), { recursive: true });
  mkdirSync(home, { recursive: true });
  copyFileSync(runMjs, path.join(root, "registry", "scripts", "hooks", "run.mjs"));
  writeFileSync(path.join(root, "registry", "scripts", "install.cjs"), fakeBootstrap);
  return { root, home, bin: path.join(home, ".local", "bin") };
}

function run(l, args, pathDir = "") {
  const env = { HOME: l.home, USERPROFILE: l.home, PATH: pathDir };
  if (win) {
    // What a real Windows hook env always has: System32 on PATH, PATHEXT and
    // ComSpec (spawn shell:true runs ComSpec), SystemRoot (absolute where.exe).
    const { SystemRoot, PATHEXT, ComSpec } = process.env;
    Object.assign(env, { SystemRoot, PATHEXT, ComSpec, PATH: [path.join(SystemRoot, "System32"), pathDir].filter(Boolean).join(";") });
  }
  return spawnSync(process.execPath, [path.join(l.root, "registry", "scripts", "hooks", "run.mjs"), ...args], {
    env,
    encoding: "utf8",
    input: "{}",
  });
}

test("session-start runs the bootstrap, then resume runs the installed binary in the same run", () => {
  const l = layout(`
const fs = require("fs"), path = require("path");
const bin = path.join(process.env.HOME || process.env.USERPROFILE, ".local", "bin");
fs.mkdirSync(bin, { recursive: true });
const p = path.join(bin, ${JSON.stringify(shimName)});
fs.writeFileSync(p, ${JSON.stringify(shimBody)});
fs.chmodSync(p, 0o755);
`);
  const r = run(l, ["session-start"]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /shim-ran resume/);
});

test("session-start with a bootstrap that installs nothing degrades to the notice", () => {
  const l = layout("");
  const r = run(l, ["session-start"]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /\[harness\] epic not found/);
});

test("guard with a binary only in ~/.local/bin still runs it (PATH append)", () => {
  const l = layout("");
  writeShim(l.bin);
  const r = run(l, ["guard"]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /shim-ran guard/);
});

test("~/.local/bin/epic wins over an older epic earlier on PATH", { skip: win }, () => {
  const l = layout("");
  writeShim(l.bin, '#!/bin/sh\necho "local-ran $1"\n');
  const other = path.join(path.dirname(l.root), "other-bin");
  writeShim(other, '#!/bin/sh\necho "path-ran $1"\n');
  const r = run(l, ["guard"], other);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /local-ran guard/);
  assert.doesNotMatch(r.stdout, /path-ran/);
});

test("with no ~/.local/bin/epic the PATH epic is still used", { skip: win }, () => {
  const l = layout("");
  const other = path.join(path.dirname(l.root), "other-bin");
  writeShim(other, '#!/bin/sh\necho "path-ran $1"\n');
  const r = run(l, ["guard"], other);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /path-ran guard/);
});

test("guard with no binary anywhere prints the notice and exits 0", () => {
  const l = layout("");
  const r = run(l, ["guard"]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /\[harness\] epic not found/);
});

test("prompt with no binary anywhere prints nothing and exits 0", () => {
  const l = layout("");
  const r = run(l, ["prompt"]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "");
});

// An older epic does not know `prompt`: it prints an error and exits 1. On this
// event every child failure must be silent and exit 0; other hooks keep theirs.
test("prompt maps a failing epic (non-zero exit, stderr noise) to a silent exit 0", { skip: win }, () => {
  const l = layout("");
  writeShim(l.bin, '#!/bin/sh\necho "error: unknown subcommand $1" >&2\nexit 1\n');
  const r = run(l, ["prompt"]);
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "");
  assert.equal(r.stderr, "");
  assert.equal(run(l, ["guard"]).status, 1, "other subcommands keep the child's exit code");
});

test("prompt maps an epic killed by a signal to a silent exit 0", { skip: win }, () => {
  const l = layout("");
  writeShim(l.bin, "#!/bin/sh\nkill -9 $$\n");
  const r = run(l, ["prompt"]);
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "");
  assert.equal(r.stderr, "");
  assert.equal(run(l, ["guard"]).status, 1, "guard still fails on a signal");
});

test("prompt maps a spawn failure (EACCES) to a silent exit 0", { skip: win }, () => {
  const l = layout("");
  writeShim(l.bin);
  chmodSync(path.join(l.bin, shimName), 0o644);
  const r = run(l, ["prompt"]);
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "");
  assert.equal(r.stderr, "");
  assert.equal(run(l, ["guard"]).status, 1, "guard still surfaces the spawn error");
});

test("prompt passes a working epic's stdout and exit code through", { skip: win }, () => {
  const l = layout("");
  writeShim(l.bin, '#!/bin/sh\necho "ctx"\n');
  const r = run(l, ["prompt"]);
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "ctx\n");
});

// agy mode: stdout is empty unless guard prints a deny object, and the exit
// code is 0 on every path (a non-zero exit or a stray `{}` blocks the tool call).
const agyShim = (body) => {
  const l = layout("");
  writeShim(l.bin, `#!/bin/sh\n${body}\n`);
  return l;
};
const silent = (r) => {
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "");
};

test("agy-guard drops an older binary's {}", { skip: win }, () => {
  silent(run(agyShim("echo '{}'"), ["agy-guard"]));
});

test("agy-guard drops a stdin echo", { skip: win }, () => {
  // cat needs a PATH: without it the shim would fail with 127 and prove nothing.
  const r = run(agyShim("cat"), ["agy-guard"], "/usr/bin:/bin");
  silent(r);
});

test("agy-guard drops non-JSON text and arrays", { skip: win }, () => {
  silent(run(agyShim("echo hi"), ["agy-guard"]));
  silent(run(agyShim(`echo '[{"decision":"deny"}]'`), ["agy-guard"]));
});

test("agy-guard forwards a deny as one line with only decision and reason", { skip: win }, () => {
  const l = agyShim(`printf '%s\\n' '{"decision":"deny","reason":"[guard] BLOCKED: x","extra":1}'`);
  const r = run(l, ["agy-guard"]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, '{"decision":"deny","reason":"[guard] BLOCKED: x"}\n');
});

test("agy-guard fails open on exit 1", { skip: win }, () => {
  silent(run(agyShim("echo boom >&2; exit 1"), ["agy-guard"]));
});

test("agy-guard fails open on a signal", { skip: win }, () => {
  silent(run(agyShim("kill -9 $$"), ["agy-guard"]));
});

test("agy-guard fails open on EACCES", { skip: win }, () => {
  const l = agyShim("echo '{}'");
  chmodSync(path.join(l.bin, shimName), 0o644);
  silent(run(l, ["agy-guard"]));
});

test("agy-guard with no binary exits 0 with the notice on stderr only", () => {
  const r = run(layout(""), ["agy-guard"]);
  silent(r);
  assert.match(r.stderr, /epic not found/);
});

test("agy-observe and agy-reflect drop stdout and an exit code", { skip: win }, () => {
  const l = agyShim("echo '{}'; exit 3");
  for (const sub of ["agy-observe", "agy-reflect"]) silent(run(l, [sub]));
});

test("agy hooks run the plain subcommand with the payload on stdin", { skip: win }, () => {
  const r = run(agyShim('echo "sub=$1" >&2; cat >&2'), ["agy-guard"], "/usr/bin:/bin");
  assert.equal(r.status, 0);
  assert.match(r.stderr, /sub=guard/);
  assert.match(r.stderr, /\{\}/);
  assert.doesNotMatch(r.stderr, /agy-guard/);
});

// run.mjs does its work at import time; with no subcommand that is one notice line.
const { appendToPath, localEpicPath } = await (async () => {
  const [argv, log] = [process.argv, console.log];
  process.argv = [process.execPath, runMjs];
  console.log = () => {};
  try {
    return await import(pathToFileURL(runMjs).href);
  } finally {
    [process.argv, console.log] = [argv, log];
    process.exitCode = undefined;
  }
})();

test("appendToPath reuses the existing Windows key casing and never adds a duplicate", () => {
  const env = { Path: "C:\\Windows\\System32", SystemRoot: "C:\\Windows" };
  appendToPath(env, "C:\\Users\\u\\.local\\bin", "win32");
  assert.deepEqual(Object.keys(env).sort(), ["Path", "SystemRoot"]);
  assert.equal(env.Path, "C:\\Windows\\System32;C:\\Users\\u\\.local\\bin");
});

test("appendToPath creates PATH when absent or empty, appends with ':' on POSIX", () => {
  const w = {};
  appendToPath(w, "D:\\bin", "win32");
  assert.deepEqual(w, { PATH: "D:\\bin" });
  const p = { PATH: "/usr/bin", Path: "ignored" };
  appendToPath(p, "/h/.local/bin", "linux");
  assert.deepEqual(p, { PATH: "/usr/bin:/h/.local/bin", Path: "ignored" });
});

test("localEpicPath returns the absolute ~/.local/bin binary only when it exists", () => {
  const yes = () => true;
  const no = () => false;
  assert.equal(localEpicPath("/home/u", "linux", yes), "/home/u/.local/bin/epic");
  assert.equal(localEpicPath("/Users/u", "darwin", yes), "/Users/u/.local/bin/epic");
  assert.equal(localEpicPath("C:\\Users\\u", "win32", yes), "C:\\Users\\u\\.local\\bin\\epic.exe");
  assert.equal(localEpicPath("/home/u", "linux", no), null);
  assert.equal(localEpicPath("C:\\Users\\u", "win32", no), null);
});
