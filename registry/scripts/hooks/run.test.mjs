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
