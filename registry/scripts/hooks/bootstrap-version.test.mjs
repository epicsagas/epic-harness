import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import vm from 'node:vm';

const source = fs.readFileSync(new URL('../install.cjs', import.meta.url), 'utf8');
async function bootstrap(variable, version) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-bootstrap-version-'));
  try {
    fs.mkdirSync(path.join(root, '.codex-plugin'), { recursive: true });
    fs.mkdirSync(path.join(root, '.claude-plugin'), { recursive: true });
    fs.mkdirSync(path.join(root, 'cwd'), { recursive: true });
    fs.mkdirSync(path.join(root, 'registry/scripts'), { recursive: true });
    fs.writeFileSync(path.join(root, '.codex-plugin/plugin.json'), JSON.stringify({ version }));
    fs.writeFileSync(path.join(root, '.claude-plugin/plugin.json'), JSON.stringify({ version }));
    const calls = [];
    const context = {
      require(name) {
        if (name === 'child_process') return {
          execSync() { throw new Error('unexpected shell command'); },
          spawnSync(command, args) {
            calls.push([command, ...args]);
            return { status: 0, stdout: Buffer.from('epic-harness 0.1.0'), stderr: Buffer.alloc(0) };
          },
        };
        if (name === 'fs') return { ...fs, readFileSync(file, ...args) { return fs.readFileSync(path.isAbsolute(file) ? file : path.join(root, 'cwd', file), ...args); } };
        if (name === 'path') return path;
        if (name === 'os') return { platform: () => 'win32', tmpdir: () => root };
        if (name === 'https') return { get() { throw new Error('unexpected network request'); } };
        throw new Error('unexpected dependency ' + name);
      },
      process: { env: { [variable]: root }, stderr: { write() {} }, exit(code) { throw new Error('unexpected exit ' + code); } },
      __dirname: path.join(root, 'registry/scripts'), Buffer,
    };
    vm.runInNewContext(source.replace('main().catch((e) => {', 'globalThis.completed = main().catch((e) => {'), context);
    await context.completed;
    return calls;
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test('Codex PLUGIN_ROOT and script-root discovery use the plugin version, independent of cwd', async () => {
  for (const variable of ['PLUGIN_ROOT', 'CLAUDE_PLUGIN_ROOT', 'GROK_PLUGIN_ROOT']) {
    const calls = await bootstrap(variable, '9.9.9');
    assert.ok(calls.some(([cmd, ...args]) => cmd === 'cargo' && args.includes('epic-harness')), `${variable} must update an older binary`);
  }
  const noUpdate = await bootstrap('PLUGIN_ROOT', '0.0.1');
  assert.equal(noUpdate.some(([cmd, ...args]) => cmd === 'cargo' && args.includes('epic-harness')), false, 'older plugin must not downgrade binary');
});
