#!/usr/bin/env node
// Payload text stays on stdin and never becomes shell syntax or arguments.
import { statSync } from 'node:fs';
import { homedir } from 'node:os';
import { spawnSync } from 'node:child_process';

const action = process.argv[2];
let inputText = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { inputText += chunk; });
process.stdin.on('end', () => {
  let input;
  try {
    if (!['resume', 'reflect'].includes(action)) throw new Error('unsupported action');
    input = JSON.parse(inputText);
    if (!input || Array.isArray(input) || typeof input !== 'object') throw new Error('expected a JSON object');
  } catch (error) {
    console.error(`[epic-antigravity] ${error.message}`);
    console.log('{}');
    process.exitCode = 1;
    return;
  }
  const workspace = Array.isArray(input.workspacePaths) && input.workspacePaths.find(candidate => {
    if (typeof candidate !== 'string' || !candidate.trim()) return false;
    try { return statSync(candidate).isDirectory(); } catch { return false; }
  });
  if (workspace) input.cwd = workspace;
  if (typeof input.conversationId === 'string') input.session_id = input.conversationId;
  // Preserve Antigravity's documented transcriptPath. Rust has no supported
  // transcript_path field or Antigravity transcript format adapter.
  input.hook_event_name = action === 'resume' ? 'SessionStart' : 'PostInvocation';
  const result = spawnSync(process.env.EPIC_BINARY || 'epic', [action], {
    cwd: workspace || homedir(), input: JSON.stringify(input), encoding: 'utf8',
    windowsHide: true, timeout: 25_000, maxBuffer: 4 * 1024 * 1024, shell: false,
  });
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error) {
    const missing = result.error.code === 'ENOENT';
    console.error(`[epic-antigravity] ${missing ? 'epic not found' : result.error.message}`);
    console.log('{}');
    process.exitCode = missing ? 0 : 1;
    return;
  }
  process.exitCode = result.status ?? 1;
  try {
    const output = result.stdout.trim() ? JSON.parse(result.stdout) : {};
    const context = output.hookSpecificOutput?.additionalContext;
    console.log(JSON.stringify(action === 'resume' && typeof context === 'string' && context
      ? { injectSteps: [{ systemMessage: { systemMessage: context } }] }
      : {}));
  } catch {
    console.error('[epic-antigravity] child returned invalid JSON');
    console.log('{}');
    if (!process.exitCode) process.exitCode = 1;
  }
});
