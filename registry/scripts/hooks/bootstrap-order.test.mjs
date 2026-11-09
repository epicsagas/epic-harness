import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, copyFileSync, existsSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('SessionStart bootstraps a missing executable before its final availability check', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'epic-bootstrap-order-'));
  const home = path.join(root, 'home'), bin = path.join(root, 'bin');
  try {
    mkdirSync(path.join(root, 'registry/scripts/hooks'), { recursive: true });
    mkdirSync(path.join(home, '.config/epic-harness'), { recursive: true });
    mkdirSync(path.join(root, 'data'), { recursive: true });
    mkdirSync(bin);
    writeFileSync(path.join(home, '.config/epic-harness/telemetry-consent'), 'off');
    writeFileSync(path.join(root, 'data/config.toml'), '[dashboard]\nport=0\nauto_open=false\n');
    copyFileSync(fileURLToPath(new URL('./run.mjs', import.meta.url)), path.join(root, 'registry/scripts/hooks/run.mjs'));
    const marker = path.join(root, 'bootstrapped');
    const script = path.join(bin, 'epic.mjs');
    writeFileSync(script, "console.log(JSON.stringify({args:process.argv.slice(2)}));");
    const executable = path.join(bin, process.platform === 'win32' ? 'epic.cmd' : 'epic');
    const shim = process.platform === 'win32'
      ? `@echo off\r\n"${process.execPath}" "${script}" %*\r\n`
      : `#!/bin/sh\nexec '${process.execPath}' '${script}' "$@"\n`;
    writeFileSync(path.join(root, 'registry/scripts/install.cjs'), `const fs=require('fs');fs.writeFileSync(${JSON.stringify(marker)},'yes');fs.writeFileSync(${JSON.stringify(executable)},${JSON.stringify(shim)},{mode:0o755});`);
    const env = { ...process.env, PLUGIN_ROOT: root, HOME: home, USERPROFILE: home, HARNESS_DIR: path.join(root, 'data'), HARNESS_ROOT: root };
    for (const k of Object.keys(env)) if (k.toLowerCase() === 'path') delete env[k];
    env.PATH = [bin, process.platform === 'win32' ? path.join(process.env.SystemRoot, 'System32') : '/usr/bin:/bin'].join(path.delimiter);
    const r = spawnSync(process.execPath, [path.join(root, 'registry/scripts/hooks/run.mjs'), 'session-start'], { cwd: root, env, encoding: 'utf8', timeout: 10000, windowsHide: true });
    assert.equal(existsSync(marker), true, 'missing binary must not prevent the bootstrap from running');
    assert.equal(r.status, 0, r.stderr);
    assert.deepEqual(JSON.parse(r.stdout), { args: ['resume'] });
    // The ordinary missing-executable no-op contract remains in place.
    const missingEnv = { ...env, PATH: process.platform === 'win32' ? path.join(process.env.SystemRoot, 'System32') : '/nonexistent' };
    const missing = spawnSync(process.execPath, [path.join(root, 'registry/scripts/hooks/run.mjs'), 'guard'], { cwd: root, env: missingEnv, encoding: 'utf8', timeout: 10000, windowsHide: true });
    assert.equal(missing.status, 0);
    assert.match(missing.stdout, /epic not found/);
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    rmSync(root, { recursive: true, force: true });
  }
});
