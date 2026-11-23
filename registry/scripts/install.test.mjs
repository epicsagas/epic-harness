// node --test registry/scripts
// No network: downloads are stubbed, archives are built locally.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { EventEmitter } from "node:events";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));
const install = require("./install.cjs");
const win = process.platform === "win32";

const scratch = mkdtempSync(path.join(os.tmpdir(), "epic-install-test-"));
after(() => rmSync(scratch, { recursive: true, force: true }));
let n = 0;
const fresh = (label) => {
  const d = path.join(scratch, `${label}-${n++}`);
  mkdirSync(d, { recursive: true });
  return d;
};

const sha = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
const posixTarget = "x86_64-unknown-linux-gnu";
const winTarget = "x86_64-pc-windows-msvc";
const hostArch = win ? "x64" : process.arch;
const hostTarget = install.targetFor(process.platform, hostArch) ?? posixTarget;

test("targetFor table", () => {
  const rows = [
    ["darwin", "arm64", "aarch64-apple-darwin"],
    ["darwin", "x64", "x86_64-apple-darwin"],
    ["linux", "arm64", "aarch64-unknown-linux-gnu"],
    ["linux", "x64", posixTarget],
    ["win32", "x64", winTarget],
    ["win32", "arm64", winTarget],
    ["linux", "ia32", null],
    ["freebsd", "x64", null],
  ];
  for (const [p, a, want] of rows) assert.equal(install.targetFor(p, a), want, `${p}/${a}`);
});

test("assetNames builds pinned urls", () => {
  const a = install.assetNames("0.9.1", winTarget);
  const base = "https://github.com/epicsagas/epic-harness/releases/download/v0.9.1/";
  assert.equal(a.archive, "epic-harness-x86_64-pc-windows-msvc.zip");
  assert.equal(a.checksum, "epic-harness-x86_64-pc-windows-msvc.zip.sha256");
  assert.equal(a.archiveUrl, `${base}epic-harness-x86_64-pc-windows-msvc.zip`);
  assert.equal(a.checksumUrl, `${base}epic-harness-x86_64-pc-windows-msvc.zip.sha256`);
  assert.equal(install.assetNames("0.9.1", posixTarget).archive, `epic-harness-${posixTarget}.tar.xz`);
});

test("assetNames rejects non-semver versions", () => {
  for (const v of ["latest", "0.9", "0.9.1/../x", "0.9.1; x"]) {
    assert.throws(() => install.assetNames(v, posixTarget), /invalid version/, v);
  }
});

test("parseSha256", () => {
  const hash = "268eb575463e04b25cdc4982a8ed4200f9f66b3f33598285fd67456ca3236b53";
  const name = "epic-harness-x86_64-pc-windows-msvc.zip";
  assert.equal(install.parseSha256(`${hash} *${name}\n`, name), hash);
  assert.equal(install.parseSha256(`${hash.toUpperCase()}  ${name}`, name), hash);
  assert.equal(install.parseSha256(`${hash.slice(1)} *${name}`, name), null);
  assert.equal(install.parseSha256(`${"g".repeat(64)} *${name}`, name), null);
  assert.equal(install.parseSha256(`${hash} *other.zip`, name), null);
  assert.equal(install.parseSha256("", name), null);
});

test("pluginVersion reads the manifest next to the script, not env", () => {
  const other = fresh("unrelated");
  mkdirSync(path.join(other, ".claude-plugin"));
  writeFileSync(path.join(other, ".claude-plugin", "plugin.json"), '{"version":"7.7.7"}');
  const saved = process.env.CLAUDE_PLUGIN_ROOT;
  process.env.CLAUDE_PLUGIN_ROOT = other;
  try {
    const codex = fresh("codex");
    mkdirSync(path.join(codex, ".codex-plugin"));
    writeFileSync(path.join(codex, ".codex-plugin", "plugin.json"), '{"version":"1.2.3"}');
    assert.equal(install.pluginVersion(codex), "1.2.3");

    const rootOnly = fresh("root");
    writeFileSync(path.join(rootOnly, "plugin.json"), '{"version":"4.5.6"}');
    assert.equal(install.pluginVersion(rootOnly), "4.5.6");

    const bad = fresh("bad");
    writeFileSync(path.join(bad, "plugin.json"), '{"version":"latest"}');
    assert.equal(install.pluginVersion(bad), null);

    // default root is the repo this script lives in, not CLAUDE_PLUGIN_ROOT
    const own = JSON.parse(
      readFileSync(path.join(here, "..", "..", ".claude-plugin", "plugin.json"), "utf8"),
    );
    assert.equal(install.pluginVersion(), own.version);
  } finally {
    if (saved === undefined) delete process.env.CLAUDE_PLUGIN_ROOT;
    else process.env.CLAUDE_PLUGIN_ROOT = saved;
  }
});

test("isSafeRedirect allows only https", () => {
  assert.equal(install.isSafeRedirect("https://example.com/x"), true);
  assert.equal(install.isSafeRedirect("http://example.com/x"), false);
  assert.equal(install.isSafeRedirect("file:///etc/passwd"), false);
  assert.equal(install.isSafeRedirect("not a url"), false);
});

test("shouldAttempt backs off for an hour after a failed same-version attempt", () => {
  const dir = fresh("marker");
  const t0 = 1_000_000_000_000;
  assert.equal(install.shouldAttempt("0.9.2", dir, t0), true);
  install.recordAttempt("0.9.2", false, dir, t0);
  assert.equal(install.shouldAttempt("0.9.2", dir, t0 + 30 * 60 * 1000), false);
  assert.equal(install.shouldAttempt("0.9.2", dir, t0 + 61 * 60 * 1000), true);
  assert.equal(install.shouldAttempt("0.9.3", dir, t0 + 1000), true);
  install.recordAttempt("0.9.2", true, dir, t0);
  assert.equal(install.shouldAttempt("0.9.2", dir, t0 + 1000), true);
});

// Archive layout of the real release: tar.xz has epic-harness-<target>/, zip is flat.
function buildArchive(dir, target) {
  const stage = fresh("stage");
  const ext = win ? ".exe" : "";
  const inner = win ? stage : path.join(stage, `epic-harness-${target}`);
  mkdirSync(inner, { recursive: true });
  for (const name of ["epic", "epic-harness"]) {
    writeFileSync(path.join(inner, name + ext), `binary ${name}`);
  }
  const archive = path.join(dir, win ? `epic-harness-${target}.zip` : `epic-harness-${target}.tar.xz`);
  const r = win
    ? spawnSync(path.join(process.env.SystemRoot, "System32", "tar.exe"), [
        "-a",
        "-cf",
        archive,
        "-C",
        stage,
        "epic.exe",
        "epic-harness.exe",
      ])
    : spawnSync("tar", ["-cJf", archive, "-C", stage, `epic-harness-${target}`]);
  assert.equal(r.status, 0, String(r.stderr));
  return archive;
}

const tmpLeftovers = () => readdirSync(os.tmpdir()).filter((f) => f.startsWith("epic-bootstrap-"));

test("installFromArchive installs both binaries", () => {
  const archive = buildArchive(fresh("dl"), hostTarget);
  const installDir = path.join(fresh("home"), ".local", "bin");
  const before = tmpLeftovers();
  install.installFromArchive({
    archive,
    checksumText: `${sha(archive)} *${path.basename(archive)}\n`,
    target: hostTarget,
    installDir,
  });
  const ext = win ? ".exe" : "";
  for (const name of ["epic", "epic-harness"]) {
    const p = path.join(installDir, name + ext);
    assert.equal(readFileSync(p, "utf8"), `binary ${name}`);
    if (!win) assert.equal(statSync(p).mode & 0o777, 0o755);
  }
  assert.deepEqual(readdirSync(installDir).sort(), [`epic${ext}`, `epic-harness${ext}`].sort());
  assert.deepEqual(tmpLeftovers(), before);
});

test("installFromArchive with a tampered checksum installs nothing", () => {
  const archive = buildArchive(fresh("dl"), hostTarget);
  const installDir = path.join(fresh("home"), ".local", "bin");
  const before = tmpLeftovers();
  assert.throws(
    () =>
      install.installFromArchive({
        archive,
        checksumText: `${"0".repeat(64)} *${path.basename(archive)}\n`,
        target: hostTarget,
        installDir,
      }),
    /checksum mismatch/,
  );
  assert.equal(existsSync(installDir), false);
  assert.deepEqual(tmpLeftovers(), before);
});

// main() with a stubbed download that serves a locally built archive.
// serve: "ok" | "bad-sha" (wrong hash) | "no-sha" (.sha256 missing, as a 404)
function mainFixture(serve = "ok") {
  const root = fresh("plugin");
  mkdirSync(path.join(root, ".claude-plugin"));
  writeFileSync(path.join(root, ".claude-plugin", "plugin.json"), '{"version":"9.9.9"}');
  const home = fresh("home");
  const archive = buildArchive(fresh("remote"), hostTarget);
  const harnessDir = fresh("harness");
  const calls = [];
  const download = async (url, dest) => {
    calls.push(url);
    if (!url.endsWith(".sha256")) return copyFileSync(archive, dest);
    if (serve === "no-sha") throw new Error("HTTP 404");
    const hash = serve === "bad-sha" ? "0".repeat(64) : sha(archive);
    writeFileSync(dest, `${hash} *${path.basename(archive)}\n`);
  };
  return {
    calls,
    installDir: path.join(home, ".local", "bin"),
    deps: { root, home, harnessDir, download, pathEnv: "", platform: process.platform, arch: hostArch },
  };
}

// install.cjs picks its target from platform/arch, so main() needs a supported host.
const mainSkip = install.targetFor(process.platform, hostArch) ? false : "unsupported host";

test("main: fresh install downloads the pinned version and verifies", { skip: mainSkip }, async () => {
  const f = mainFixture();
  await install.main(f.deps);
  assert.deepEqual(f.calls.map((u) => u.split("/download/")[1].split("/")[0]), ["v9.9.9", "v9.9.9"]);
  assert.ok(f.calls[0].endsWith(".sha256"), "checksum is fetched before the archive");
  assert.ok(existsSync(path.join(f.installDir, win ? "epic.exe" : "epic")));
});

test("main: checksum mismatch refuses and leaves no binary", { skip: mainSkip }, async () => {
  const f = mainFixture("bad-sha");
  await install.main(f.deps);
  assert.equal(existsSync(f.installDir), false);
  const marker = JSON.parse(readFileSync(path.join(f.deps.harnessDir, "bootstrap_attempt.json"), "utf8"));
  assert.equal(marker.ok, false);
});

test("main: missing .sha256 refuses without downloading the archive", { skip: mainSkip }, async () => {
  const f = mainFixture("no-sha");
  await install.main(f.deps);
  assert.equal(existsSync(f.installDir), false);
  assert.equal(f.calls.length, 1);
  assert.ok(f.calls[0].endsWith(".sha256"));
});

test("main: a recent failed attempt is not retried", { skip: mainSkip }, async () => {
  const f = mainFixture("no-sha");
  await install.main(f.deps);
  await install.main(f.deps);
  assert.equal(f.calls.length, 1);
});

test("main: unknown plugin version downloads nothing", { skip: mainSkip }, async () => {
  const f = mainFixture();
  rmSync(path.join(f.deps.root, ".claude-plugin"), { recursive: true });
  await install.main(f.deps);
  assert.equal(f.calls.length, 0);
});

// A real .exe is needed on Windows: spawnSync(shell: false) cannot run a .cmd
// (CVE-2024-27980) and findEpic only runs what it finds. Compiled once with the
// csc.exe that ships in every Windows install; version and arg log are sidecar files.
const stubSource = `using System;using System.IO;
class P{static int Main(string[] a){
string d=AppDomain.CurrentDomain.BaseDirectory;
string l=Path.Combine(d,"epic.arglog");
if(File.Exists(l))File.AppendAllText(File.ReadAllText(l).Trim(),string.Join(" ",a)+"\\n");
if(a.Length>0&&a[0]=="--version"){Console.WriteLine("epic-harness "+File.ReadAllText(Path.Combine(d,"epic.version")).Trim());return 0;}
return 1;}}`;
let stubExe;
function winStubExe() {
  if (stubExe) return stubExe;
  const dir = fresh("stub");
  const src = path.join(dir, "stub.cs");
  stubExe = path.join(dir, "stub.exe");
  writeFileSync(src, stubSource);
  const csc = path.join(process.env.SystemRoot, "Microsoft.NET", "Framework64", "v4.0.30319", "csc.exe");
  const r = spawnSync(csc, ["/nologo", `/out:${stubExe}`, src]);
  assert.equal(r.status, 0, String(r.stdout) + String(r.stderr));
  return stubExe;
}

// A stand-in epic that only answers `--version`, like the real probe; `version` fails.
function writeStubEpic(dir, ver, argLog) {
  mkdirSync(dir, { recursive: true });
  if (win) {
    const exe = path.join(dir, "epic.exe");
    copyFileSync(winStubExe(), exe);
    writeFileSync(path.join(dir, "epic.version"), ver);
    if (argLog) writeFileSync(path.join(dir, "epic.arglog"), argLog);
    return exe;
  }
  const shim = path.join(dir, "epic");
  const log = argLog ? `echo "$@" >> '${argLog}'\n` : "";
  writeFileSync(shim, `#!/bin/sh\n${log}case "$1" in --version) echo 'epic-harness ${ver}';; *) exit 1;; esac\n`);
  chmodSync(shim, 0o755);
  return shim;
}

async function captureStderr(fn) {
  const lines = [];
  const orig = process.stderr.write;
  process.stderr.write = (s) => (lines.push(String(s)), true);
  try {
    await fn();
  } finally {
    process.stderr.write = orig;
  }
  return lines;
}

// An epic older than the plugin outside ~/.local/bin (brew, cargo, manual) is never
// touched: the pinned copy goes to ~/.local/bin and the user gets one notice line.
for (const [label, sub, registry, hint] of [
  ["brew", ["Cellar", "epic-harness", "bin"], null, "brew upgrade epic-harness"],
  ["cargo-built", [".cargo", "bin"], '{"installs":{"epic-harness 0.0.1 (registry+x)":{}}}', "cargo install epic-harness --locked --force"],
  ["bootstrap ~/.cargo/bin", [".cargo", "bin"], null, "re-run the installer command"],
  ["manual", ["bin"], null, "epic update"],
]) {
  test(`main: older ${label} epic gets a ~/.local/bin install and one notice`, { skip: mainSkip }, async () => {
    const f = mainFixture();
    const root = fresh("shell");
    const dir = path.join(root, ...sub);
    const shim = writeStubEpic(dir, "0.0.1");
    const before = readFileSync(shim, "utf8");
    if (registry) writeFileSync(path.join(root, ".cargo", ".crates2.json"), registry);
    const lines = await captureStderr(() => install.main({ ...f.deps, pathEnv: dir }));
    assert.equal(f.calls.length, 2);
    assert.ok(existsSync(path.join(f.installDir, win ? "epic.exe" : "epic")));
    assert.equal(readFileSync(shim, "utf8"), before, "PATH binary is never modified");
    const notices = lines.filter((l) => l.includes(hint));
    assert.equal(notices.length, 1, lines.join(""));
    assert.match(notices[0], /shell epic 0\.0\.1 is older than the plugin's 9\.9\.9/);
  });
}

for (const ver of ["9.9.9", "10.0.0"]) {
  test(`main: PATH epic ${ver} (equal or newer than 9.9.9) does nothing`, { skip: mainSkip }, async () => {
    const f = mainFixture();
    const dir = path.join(fresh("shell"), "bin");
    writeStubEpic(dir, ver);
    const lines = await captureStderr(() => install.main({ ...f.deps, pathEnv: dir }));
    assert.equal(f.calls.length, 0);
    assert.equal(existsSync(f.installDir), false);
    assert.deepEqual(lines, []);
  });
}

test("main: older PATH epic with a current ~/.local/bin copy downloads nothing", { skip: mainSkip }, async () => {
  const f = mainFixture();
  const dir = path.join(fresh("shell"), "bin");
  writeStubEpic(dir, "0.0.1");
  writeStubEpic(f.installDir, "9.9.9");
  const lines = await captureStderr(() => install.main({ ...f.deps, pathEnv: dir }));
  assert.equal(f.calls.length, 0);
  assert.deepEqual(lines, []);
});

test("main: an older ~/.local/bin epic is updated in place without a shell notice", { skip: mainSkip }, async () => {
  const f = mainFixture();
  writeStubEpic(f.installDir, "0.0.1");
  const lines = await captureStderr(() => install.main({ ...f.deps, pathEnv: f.installDir }));
  assert.equal(f.calls.length, 2);
  assert.equal(readFileSync(path.join(f.installDir, win ? "epic.exe" : "epic"), "utf8"), "binary epic");
  assert.equal(lines.filter((l) => l.includes("shell epic")).length, 0);
});

test("main: a mixed-case ~/.local/bin on PATH is the same directory on win32", { skip: !win || mainSkip }, async () => {
  const f = mainFixture();
  writeStubEpic(f.installDir, "0.0.1");
  const lines = await captureStderr(() => install.main({ ...f.deps, pathEnv: f.installDir.toUpperCase() }));
  assert.equal(f.calls.length, 2);
  assert.equal(lines.filter((l) => l.includes("shell epic")).length, 0);
});

test("main: the version probe runs `epic --version`, never `epic version`", { skip: mainSkip }, async () => {
  const f = mainFixture();
  const argLog = path.join(fresh("args"), "log");
  const dir = path.join(fresh("shell"), "bin");
  writeStubEpic(dir, "9.9.9", argLog);
  await install.main({ ...f.deps, pathEnv: dir });
  assert.equal(readFileSync(argLog, "utf8"), "--version\n");
});

test("main: the attempt marker exists before the download starts", { skip: mainSkip }, async () => {
  const f = mainFixture();
  const seen = [];
  const download = async () => {
    seen.push(JSON.parse(readFileSync(path.join(f.deps.harnessDir, "bootstrap_attempt.json"), "utf8")));
    throw new Error("session killed");
  };
  await install.main({ ...f.deps, download });
  assert.equal(seen.length, 1);
  assert.equal(seen[0].version, "9.9.9");
  assert.equal(seen[0].ok, false);
});

test("main: a download timeout installs nothing and backs off", { skip: mainSkip }, async () => {
  const f = mainFixture();
  const before = tmpLeftovers();
  let calls = 0;
  const download = async () => {
    calls++;
    throw new Error("timeout");
  };
  const lines = await captureStderr(async () => {
    await install.main({ ...f.deps, download });
    await install.main({ ...f.deps, download });
  });
  assert.equal(calls, 1);
  assert.equal(existsSync(f.installDir), false);
  assert.ok(lines.some((l) => l.includes("Install failed: timeout")));
  assert.deepEqual(tmpLeftovers(), before);
});

test("downloadFile times out a transfer that never answers and removes the partial file", async () => {
  const dest = path.join(fresh("dl"), "x");
  writeFileSync(dest, "partial");
  const hangingGet = () => {
    const req = new EventEmitter();
    req.destroy = (e) => req.emit("error", e);
    return req;
  };
  await assert.rejects(install.downloadFile("https://example.invalid/x", dest, { timeoutMs: 20, get: hangingGet }), /timeout/);
  assert.equal(existsSync(dest), false);
});

test("sweepStaleTmp removes only epic-bootstrap-* dirs older than an hour", () => {
  const dir = fresh("tmp");
  const mk = (name, ageMs) => {
    const d = path.join(dir, name);
    mkdirSync(d);
    writeFileSync(path.join(d, "f"), "x");
    const t = new Date(Date.now() - ageMs);
    utimesSync(d, t, t);
  };
  mk("epic-bootstrap-old", 2 * 60 * 60 * 1000);
  mk("epic-bootstrap-new", 60 * 1000);
  mk("unrelated-old", 2 * 60 * 60 * 1000);
  install.sweepStaleTmp(dir);
  assert.deepEqual(readdirSync(dir).sort(), ["epic-bootstrap-new", "unrelated-old"]);
});

test("install.cjs has no releases/latest, installer script, or elevated shell path", () => {
  const src = readFileSync(path.join(here, "install.cjs"), "utf8");
  for (const needle of ["releases/latest", "installer.sh", "installer.ps1", "ExecutionPolicy", "brew install", "binstall epic-harness --no-confirm"]) {
    assert.ok(!src.includes(needle), `install.cjs still contains ${needle}`);
  }
});
