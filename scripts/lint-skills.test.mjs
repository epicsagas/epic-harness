import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const script = new URL('./lint-skills.mjs', import.meta.url);
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic skill lint '));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}
function write(root, name, content) {
  fs.mkdirSync(path.join(root, name), { recursive: true });
  fs.writeFileSync(path.join(root, name, 'SKILL.md'), content);
}
function run(root, ...args) {
  return spawnSync(process.execPath, [fileURLToPath(script), '--root', root, ...args], { encoding: 'utf8' });
}
const valid = (name, description = 'Use when tests fail.') => `---\nname: ${name}\ndescription: "${description}"\n---\n# Body\n`;

test('skill lint discovers actual published skills and handles roots containing spaces', t => {
  const root = fixture(t);
  write(root, '_dispatch', valid('_dispatch'));
  write(root, 'debug', valid('debug'));
  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /2 skills checked/);
});

test('missing and empty skill roots fail instead of producing false green', t => {
  const root = fixture(t);
  for (const dir of [root, path.join(root, 'missing')]) assert.notEqual(run(dir).status, 0);
});

test('missing SKILL.md is an error', t => {
  const root = fixture(t);
  fs.mkdirSync(path.join(root, 'debug'));
  assert.match(run(root).stderr, /missing SKILL.md/i);
});

test('required fields must be in closed frontmatter, not in the body', t => {
  const root = fixture(t);
  for (const content of [
    '# Body\nname: debug\ndescription: fake',
    '---\nname: debug\ndescription: missing close',
    '---\nname: debug\n---\ndescription: fake',
    '---\nname: debug\ndescription: ""\n---\n# Body',
    '---\nname: debug\ndescription: "unterminated\n---\n# Body',
  ]) {
    write(root, 'debug', content);
    assert.notEqual(run(root).status, 0, content);
  }
});

test('directory/name mismatches and duplicate required fields fail', t => {
  const root = fixture(t);
  for (const content of [valid('other'), valid('debug').replace('name: debug', 'name: debug\nname: debug')]) {
    write(root, 'debug', content);
    assert.notEqual(run(root).status, 0);
  }
});

test('CSO warning examines only the description and stays advisory', t => {
  const root = fixture(t);
  write(root, 'debug', valid('debug') + 'workflow step 1 phase 2 dispatch\n');
  assert.doesNotMatch(run(root).stdout, /CSO WARNING/);
  write(root, 'debug', valid('debug', 'A workflow with step 1.'));
  const result = run(root);
  assert.equal(result.status, 0);
  assert.match(result.stdout, /CSO WARNING/);
});

test('folded descriptions, quoted names, and CRLF are accepted', t => {
  const root = fixture(t);
  write(root, 'debug', "---\r\nname: 'debug'\r\ndescription: >-\r\n  Use when tests fail\r\n  or runtime errors occur.\r\nmodes:\r\n  - inspect\r\n---\r\n# Body\r\n");
  assert.equal(run(root).status, 0);
});

test('dry-run validates the same files and does not mutate them', t => {
  const root = fixture(t);
  const original = valid('debug');
  write(root, 'debug', original);
  const result = run(root, '--dry-run');
  assert.equal(result.status, 0);
  assert.match(result.stdout, /Dry-run/);
  assert.equal(fs.readFileSync(path.join(root, 'debug', 'SKILL.md'), 'utf8'), original);
});

test('quoted comments and plain trigger strings are accepted, non-string fields rejected', t => {
  const root = fixture(t);
  for (const description of ['"Use when tests fail." # comment', "'Use when tests fail.' # comment", '3+ failures trigger diagnosis']) {
    write(root, 'debug', `---\nname: debug\ndescription: ${description}\n---\n# Body`);
    assert.equal(run(root).status, 0, description);
  }
  for (const description of ['true', 'null', '123', '[one, two]']) {
    write(root, 'debug', `---\nname: debug\ndescription: ${description}\n---\n# Body`);
    assert.notEqual(run(root).status, 0, description);
  }
});
