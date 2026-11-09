import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('Windows acceptance executes manifest commands through the actual host shell', { skip: process.platform !== 'win32' }, () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-host-acceptance-'));
  try {
    fs.mkdirSync(path.join(root, '.codex-plugin'), { recursive: true });
    fs.mkdirSync(path.join(root, 'registry/scripts/hooks'), { recursive: true });
    fs.mkdirSync(path.join(root, 'fake-bin'));
    fs.copyFileSync(new URL('../../../.codex-plugin/hooks.json', import.meta.url), path.join(root, '.codex-plugin/hooks.json'));
    fs.writeFileSync(path.join(root, 'registry/scripts/hooks/run.mjs'), 'console.log("fixture hook reached");');
    const marker = path.join(root, 'host-shell-used');
    const nodeMarker = path.join(root, 'fake-bin/marker.mjs');
    fs.writeFileSync(nodeMarker, `import fs from 'node:fs';fs.appendFileSync(${JSON.stringify(marker)},'called\\n');`);
    fs.writeFileSync(path.join(root, 'fake-bin/node.cmd'), `@echo off\r\n"${process.execPath}" "${nodeMarker}"\r\n"${process.execPath}" %*\r\n`);
    const env = { ...process.env, PLUGIN_ROOT: root };
    for (const k of Object.keys(env)) if (k.toLowerCase() === 'path') delete env[k];
    env.PATH = path.join(root, 'fake-bin') + path.delimiter + path.join(process.env.SystemRoot, 'System32');
    const result = spawnSync(process.execPath, [fileURLToPath(new URL('../../../examples/codex-live-check/windows-acceptance.mjs', import.meta.url)), 'live'], { cwd: root, env, encoding: 'utf8', timeout: 30000, windowsHide: true });
    assert.equal(result.status, 0, result.stdout + result.stderr);
    assert.equal(fs.existsSync(marker), true, 'argv-only acceptance can hide host shell quoting and .cmd resolution failures');
    const hooks = Object.values(JSON.parse(fs.readFileSync(path.join(root, '.codex-plugin/hooks.json'), 'utf8')).hooks).flat().flatMap(g => g.hooks).length;
    assert.equal(fs.readFileSync(marker, 'utf8').split('called').length - 1, hooks);
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    fs.rmSync(root, { recursive: true, force: true });
  }
});
