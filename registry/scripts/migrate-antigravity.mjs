#!/usr/bin/env node
// After updating the plugin, retire the known global workaround. Never
// overwrite another integration's named hooks or MCP configuration.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';

const configDir = path.resolve(process.argv[2] || path.join(homedir(), '.gemini/config'));
const pluginDir = path.resolve(process.argv[3] || path.join(configDir, 'plugins/epic'));
const hooks = JSON.parse(readFileSync(path.join(pluginDir, 'hooks.json'), 'utf8'));
for (const [event, action] of [['PreInvocation', 'resume'], ['PostInvocation', 'reflect']]) {
  if (hooks.epic?.[event]?.length !== 1 || hooks.epic[event][0].command !== `node registry/scripts/hooks/antigravity.mjs ${action}`) {
    throw new Error('Update the Antigravity plugin before migrating global hooks');
  }
}
if (!existsSync(path.join(pluginDir, 'registry/scripts/hooks/antigravity.mjs'))) throw new Error('Missing Antigravity adapter');
const target = path.join(configDir, 'hooks.json');
if (existsSync(target)) {
  const original = readFileSync(target, 'utf8');
  const global = JSON.parse(original);
  for (const [event, action] of [['PreInvocation', 'resume'], ['PostInvocation', 'reflect']]) {
    const handlers = global.epic?.[event];
    if (!handlers) continue;
    if (!Array.isArray(handlers) || handlers.some(handler => {
      const command = handler.command;
      return typeof command !== 'string' || !(command === `epic ${action}` || (command.includes('agy-epic-hook.cjs') && command.endsWith(` ${action}`)));
    })) throw new Error(`Unrecognized global Epic ${event} handler; migration left configuration unchanged`);
    delete global.epic[event];
  }
  if (global.epic && Object.keys(global.epic).length === 0) delete global.epic;
  if (JSON.stringify(global) !== JSON.stringify(JSON.parse(original))) writeFileSync(target, `${JSON.stringify(global, null, 2)}\n`);
}
