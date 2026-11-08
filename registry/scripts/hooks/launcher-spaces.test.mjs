import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
for (const [manifest, variable] of [['.claude-plugin/hooks.json', 'CLAUDE_PLUGIN_ROOT'], ['.codex-plugin/hooks.json', 'PLUGIN_ROOT'], ['.grok-plugin/hooks.json', 'GROK_PLUGIN_ROOT']]) {
  test(`${manifest} commands survive a plugin directory containing spaces through the real host shell`, () => {
    const root = mkdtempSync(path.join(tmpdir(), 'epic plugin space '));
    try {
      const script = path.join(root, 'registry/scripts/hooks');
      mkdirSync(script, { recursive: true });
      writeFileSync(path.join(script, 'run.mjs'), 'console.log(JSON.stringify(process.argv.slice(2)));');
      const hooks = JSON.parse(readFileSync(path.join(repo, manifest), 'utf8')).hooks;
      for (const handler of Object.values(hooks).flat().flatMap(group => group.hooks)) {
        // The temporary directory and shipped command are controlled test data;
        // this deliberately exercises the shell the host uses, not argv bypass.
        const command = handler.command.replaceAll(`\u0024{${variable}}`, root);
        const result = process.platform === 'win32'
          ? spawnSync('cmd.exe', ['/D', '/S', '/C', command], { encoding: 'utf8', windowsVerbatimArguments: true })
          : spawnSync('/bin/sh', ['-c', command], { encoding: 'utf8' });
        assert.equal(result.status, 0, `${command}\n${result.stderr}`);
        assert.deepEqual(JSON.parse(result.stdout), [handler.command.split(' ').at(-1)]);
      }
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
}
