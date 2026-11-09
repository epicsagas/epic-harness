import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('../../../', import.meta.url));
const manifest = name => JSON.parse(fs.readFileSync(path.join(repository, name, 'hooks.json'), 'utf8'));

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-prompt-dispatch-'));
  return { root, remove() {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('epic-prompt-dispatch-'));
    fs.rmSync(root, { recursive: true, force: true });
  } };
}

for (const [name, variable] of [['.claude-plugin', 'CLAUDE_PLUGIN_ROOT'], ['.codex-plugin', 'PLUGIN_ROOT']]) {
  test(`${name} prompt transport preserves literal host roots through the real shell`, () => {
    const owned = fixture();
    try {
      const plugin = path.join(owned.root, 'plugin %EPIC_UNSET% & (scope) with spaces');
      const scripts = path.join(plugin, 'registry/scripts/hooks');
      fs.mkdirSync(scripts, { recursive: true });
      fs.writeFileSync(path.join(scripts, 'run.mjs'), 'console.log(JSON.stringify(process.argv.slice(2)));');
      const command = manifest(name).hooks.UserPromptSubmit[0].hooks[0].command;
      const env = { ...process.env, [variable]: plugin };
      delete env.EPIC_UNSET;
      const result = process.platform === 'win32'
        ? spawnSync('cmd.exe', ['/D', '/S', '/C', command], { env, encoding: 'utf8', windowsVerbatimArguments: true, timeout: 10000 })
        : spawnSync('/bin/sh', ['-c', command], { env, encoding: 'utf8', timeout: 10000 });
      assert.equal(result.status, 0, result.stdout + result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), ['dispatch']);
    } finally { owned.remove(); }
  });
}

test('released-artifact acceptance retains lifecycle checks and defers new prompt dispatch to source tests', () => {
  const owned = fixture();
  try {
    const root = owned.root;
    fs.mkdirSync(path.join(root, '.codex-plugin'));
    fs.mkdirSync(path.join(root, 'registry/scripts/hooks'), { recursive: true });
    const current = manifest('.codex-plugin');
    fs.writeFileSync(path.join(root, '.codex-plugin/hooks.json'), JSON.stringify({ hooks: {
      SessionStart: current.hooks.SessionStart,
      UserPromptSubmit: current.hooks.UserPromptSubmit,
    } }));
    fs.writeFileSync(path.join(root, 'registry/scripts/hooks/run.mjs'),
      "import fs from 'node:fs'; const sub=process.argv[2]; fs.appendFileSync(process.env.EPIC_FIXTURE_CALL_LOG,sub+'\\n'); if(sub==='dispatch'){console.error('released artifact has no dispatch command');process.exitCode=23;} else console.log('legacy lifecycle reached');");
    const log = path.join(root, 'calls.txt');
    const result = spawnSync(process.execPath, [path.join(repository, 'examples/codex-live-check/windows-acceptance.mjs'), 'live'], {
      cwd: root, env: { ...process.env, PLUGIN_ROOT: root, EPIC_FIXTURE_CALL_LOG: log },
      encoding: 'utf8', timeout: 30000,
    });
    assert.equal(result.status, 0, result.stdout + result.stderr);
    assert.match(result.stdout, /source-built/);
    assert.equal(fs.readFileSync(log, 'utf8'), 'session-start\n');
  } finally { owned.remove(); }
});
