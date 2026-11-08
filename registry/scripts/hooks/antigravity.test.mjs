import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, cpSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const adapter = path.join(root, 'registry/scripts/hooks/antigravity.mjs');

test('Antigravity invocation handlers use the host flat schema once per event', () => {
  const manifest = JSON.parse(readFileSync(path.join(root, 'hooks.json')));
  const plugin = JSON.parse(readFileSync(path.join(root, 'plugin.json')));
  assert.equal(plugin.hooks, undefined, 'Antigravity auto-discovers hooks.json; hooks is an ignored manifest field');
  for (const event of ['PreInvocation', 'PostInvocation']) {
    assert.equal(manifest.epic[event].length, 1);
    assert.equal(typeof manifest.epic[event][0].command, 'string');
    assert.equal(manifest.epic[event][0].hooks, undefined);
  }
  assert.equal(manifest.epic.SessionStart, undefined);
  assert.equal(manifest.epic.Stop, undefined);
  assert.equal(manifest.epic.PreToolUse, undefined);
});

test('workspace, session and documented transcript fields survive safe argv invocation', () => {
  const temp = mkdtempSync(path.join(tmpdir(), 'epic adapter spaces '));
  try {
    const workspace = path.join(temp, 'valid workspace');
    mkdirSync(workspace);
    const script = `const fs=require('fs');const input=JSON.parse(fs.readFileSync(0,'utf8'));fs.appendFileSync('calls.jsonl',JSON.stringify({cwd:process.cwd(),input})+'\\n');console.error('child diagnostic');console.log(JSON.stringify({hookSpecificOutput:{additionalContext:'restored context'}}));process.exit(Number(input.testExit)||0);`;
    for (const action of ['resume', 'reflect']) writeFileSync(path.join(workspace, action), script);
    const input = { workspacePaths: [path.join(temp, 'missing'), path.join(workspace, 'resume'), workspace], conversationId: 'session & echo injected', transcriptPath: path.join(workspace, 'supported.jsonl') };
    const run = (action, payload = JSON.stringify(input), binary = process.execPath) => spawnSync(process.execPath, [adapter, action], { input: payload, encoding: 'utf8', env: { ...process.env, EPIC_BINARY: binary } });
    const resume = run('resume');
    assert.equal(resume.status, 0, resume.stderr);
    assert.deepEqual(JSON.parse(resume.stdout), { injectSteps: [{ systemMessage: { systemMessage: 'restored context' } }] });
    assert.match(resume.stderr, /child diagnostic/);
    const reflect = run('reflect');
    assert.equal(reflect.status, 0, reflect.stderr);
    assert.deepEqual(JSON.parse(reflect.stdout), {});
    const calls = readFileSync(path.join(workspace, 'calls.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
    assert.equal(calls.length, 2);
    assert.equal(calls[0].cwd, workspace);
    assert.equal(calls[0].input.session_id, input.conversationId);
    assert.equal(calls[0].input.hook_event_name, 'SessionStart');
    assert.equal(calls[0].input.transcriptPath, input.transcriptPath);
    assert.equal(calls[0].input.transcript_path, undefined, 'Rust has no supported transcript_path field');
    assert.equal(run('reflect', JSON.stringify({ ...input, testExit: 7 })).status, 7);
    const malformed = run('resume', '{');
    assert.equal(malformed.status, 1);
    assert.deepEqual(JSON.parse(malformed.stdout), {});
    const missing = run('resume', JSON.stringify(input), path.join(temp, 'missing executable'));
    assert.equal(missing.status, 0);
    assert.deepEqual(JSON.parse(missing.stdout), {});
    assert.match(missing.stderr, /not found/);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('migration is repeatable and preserves Orca hooks and unrelated MCP servers', () => {
  const temp = mkdtempSync(path.join(tmpdir(), 'epic migration '));
  try {
    const plugin = path.join(temp, 'plugins/epic');
    mkdirSync(plugin, { recursive: true });
    cpSync(path.join(root, 'registry'), path.join(plugin, 'registry'), { recursive: true });
    cpSync(path.join(root, 'hooks.json'), path.join(plugin, 'hooks.json'));
    const orca = { PreInvocation: [{ command: 'orca-pre.cmd' }], Stop: [{ command: 'orca-stop.cmd' }] };
    writeFileSync(path.join(temp, 'hooks.json'), JSON.stringify({ 'orca-status': orca, epic: { PreInvocation: [{ command: 'node "path with spaces/agy-epic-hook.cjs" resume' }], PostInvocation: [{ command: 'epic reflect' }] } }));
    const mcp = '{"mcpServers":{"other":{"command":"other-server"}}}';
    writeFileSync(path.join(temp, 'mcp_config.json'), mcp);
    const migrate = () => spawnSync(process.execPath, [path.join(root, 'registry/scripts/migrate-antigravity.mjs'), temp, plugin], { encoding: 'utf8' });
    assert.equal(migrate().status, 0);
    const once = readFileSync(path.join(temp, 'hooks.json'), 'utf8');
    assert.deepEqual(JSON.parse(once), { 'orca-status': orca });
    assert.equal(migrate().status, 0);
    assert.equal(readFileSync(path.join(temp, 'hooks.json'), 'utf8'), once);
    assert.equal(readFileSync(path.join(temp, 'mcp_config.json'), 'utf8'), mcp);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

