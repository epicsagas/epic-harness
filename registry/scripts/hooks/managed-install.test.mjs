import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import vm from 'node:vm';

const source = fs.readFileSync(new URL('../install.cjs', import.meta.url), 'utf8');
const pin = { schema: 1, repository: 'example/epic-harness', commit: 'a'.repeat(40), automatic_updates: false };

async function bootstrap(marker, laterMarker = false) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-managed-bootstrap-'));
  try {
    const first = path.join(root, 'first binary'), second = path.join(root, 'second binary');
    for (const bin of [first, second]) {
      fs.mkdirSync(bin, { recursive: true });
      fs.writeFileSync(path.join(bin, 'epic-harness.exe'), 'fixture');
    }
    fs.mkdirSync(path.join(root, '.claude-plugin'));
    fs.writeFileSync(path.join(root, '.claude-plugin/plugin.json'), '{"version":"9.9.9"}');
    if (marker !== undefined) fs.writeFileSync(path.join(laterMarker ? second : first, 'epic-managed.json'), JSON.stringify(marker));
    const calls = [], logs = [];
    const context = {
      require(name) {
        if (name === 'child_process') return {
          execSync() { throw new Error('unexpected shell command'); },
          spawnSync(command, args) { calls.push([command, ...args]); return { status: 0, stdout: Buffer.from('epic-harness 0.1.0') }; },
        };
        if (name === 'fs') return fs;
        if (name === 'path') return path;
        if (name === 'os') return { platform: () => 'win32', tmpdir: () => root };
        if (name === 'https') return { get() { throw new Error('unexpected network request'); } };
        throw new Error('unexpected dependency ' + name);
      },
      process: { env: { CLAUDE_PLUGIN_ROOT: root, PATH: [first, second].join(path.delimiter) }, stderr: { write(s) { logs.push(s); } }, exit(code) { throw new Error('unexpected exit ' + code); } },
      __dirname: path.join(root, 'registry/scripts'), Buffer,
    };
    vm.runInNewContext(source.replace('main().catch((e) => {', 'globalThis.completed = main().catch((e) => {'), context);
    await context.completed;
    return { installed: calls.some(([cmd, ...args]) => cmd === 'cargo' && args.includes('epic-harness')), logs: logs.join('') };
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test('bootstrap preserves an explicitly managed source installation even when the plugin is newer', async () => {
  const result = await bootstrap(pin);
  assert.equal(result.installed, false, result.logs);
  assert.match(result.logs, /managed installation/);
});

test('ordinary, invalid, and non-effective markers do not suppress normal updates', async () => {
  for (const marker of [undefined, { ...pin, automatic_updates: true }, { ...pin, commit: 'not-a-commit' }, { ...pin, commit: [pin.commit] }, { ...pin, schema: 2 }, { ...pin, repository: '' }, { ...pin, repository: [pin.repository] }]) {
    assert.equal((await bootstrap(marker)).installed, true, JSON.stringify(marker));
  }
  assert.equal((await bootstrap(pin, true)).installed, true, 'a later PATH binary cannot pin the effective installation');
});
