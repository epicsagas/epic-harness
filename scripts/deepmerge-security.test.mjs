import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFileSync, realpathSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import test from 'node:test';

// Resolve the dependency from its real consumer, rather than a test-only copy.
const pluginRequire = createRequire(realpathSync(new URL('../app/node_modules/@sveltejs/vite-plugin-svelte/package.json', import.meta.url)));
const merge = pluginRequire('deepmerge');
const poison = () => JSON.parse('{"__proto__":{"isAdmin":true}}');
const assertClean = (value) => {
  assert.equal(Object.getPrototypeOf(value), Object.prototype);
  assert.equal(value.isAdmin, undefined);
  assert.equal(Object.hasOwn(value, '__proto__'), false);
};

test('issue 156 installs the reviewed patch through the frozen dependency graph', () => {
  const patch = readFileSync(new URL('../app/patches/deepmerge@4.3.1.patch', import.meta.url), 'utf8').replaceAll('\r\n', '\n');
  const hash = createHash('sha256').update(patch).digest('hex');
  const lock = readFileSync(new URL('../app/pnpm-lock.yaml', import.meta.url), 'utf8');
  const workspace = readFileSync(new URL('../app/pnpm-workspace.yaml', import.meta.url), 'utf8');
  assert.match(workspace, /deepmerge@4\.3\.1: patches\/deepmerge@4\.3\.1\.patch/);
  assert.ok(lock.includes(`hash: ${hash}`), 'lockfile must contain this exact reviewed patch hash');
  assert.ok(pluginRequire.resolve('deepmerge').includes('patch_hash='), 'the real consumer must load the patched installation');
});

test('issue 156 rejects source prototype keys with a null-prototype target', () => {
  const target = Object.assign(Object.create(null), { safe: 1 });
  const result = merge(target, poison());
  assertClean(result);
  assert.equal(result.safe, 1);
  assert.equal(Object.getPrototypeOf(target), null);
});

test('issue 156 rejects nested prototype keys when a target value is null', () => {
  const result = merge({ session: null }, { session: poison() });
  assertClean(result.session);
});

test('issue 156 rejects prototype keys copied from the target as well as source', () => {
  const result = merge(poison(), { safe: 2 });
  assertClean(result);
  assert.equal(result.safe, 2);
});

test('issue 156 rejects constructor and prototype keys at every nested depth', () => {
  const result = merge({}, JSON.parse('{"nested":{"constructor":{"isAdmin":true},"prototype":{"isAdmin":true},"safe":3}}'));
  assertClean(result.nested);
  assert.equal(Object.hasOwn(result.nested, 'constructor'), false);
  assert.equal(Object.hasOwn(result.nested, 'prototype'), false);
  assert.equal(result.nested.safe, 3);
});

test('issue 156 preserves normal nested merging, enumerable symbols and inputs', () => {
  const symbol = Symbol('compiler setting');
  const target = { compiler: { dev: false }, preprocess: ['a'], [symbol]: 'target' };
  const source = { compiler: { runes: true }, preprocess: ['b'], [symbol]: 'source' };
  const result = merge(target, source);
  assert.deepEqual(result.compiler, { dev: false, runes: true });
  assert.deepEqual(result.preprocess, ['a', 'b']);
  assert.equal(result[symbol], 'source');
  assert.deepEqual(target.preprocess, ['a']);
  assert.deepEqual(source.compiler, { runes: true });
});

test('issue 156 preserves custom merge and array merge options used by consumers', () => {
  const result = merge({ names: ['a'], option: 1 }, { names: ['b'], option: 2 }, {
    arrayMerge: (_target, source) => source,
    customMerge: (key) => key === 'option' ? (a, b) => a + b : undefined,
  });
  assert.deepEqual(result.names, ['b']);
  // Primitive custom-merge dispatch is unchanged: deepmerge takes the source.
  assert.equal(result.option, 2);
  assert.deepEqual(merge.all([{ a: 1 }, { b: 2 }]), { a: 1, b: 2 });
  assert.deepEqual(merge({ option: { a: 1 } }, { option: { b: 2 } }, {
    customMerge: key => key === 'option' ? (a, b) => ({ count: Object.keys(a).length + Object.keys(b).length }) : undefined,
  }), { option: { count: 2 } });
});

test('issue 156 protects the packaged UMD entry point too', () => {
  const umd = pluginRequire(path.join(path.dirname(pluginRequire.resolve('deepmerge')), 'umd.js'));
  const result = umd(Object.create(null), poison());
  assertClean(result);
  assert.deepEqual(umd({ a: 1 }, { b: 2 }), { a: 1, b: 2 });
});

test('issue 156 preserves benign source fields when merging null or primitive placeholders', () => {
  assert.deepEqual(merge(null, { safe: 1 }), { safe: 1 });
  assert.deepEqual(merge(4, { safe: 1 }), { safe: 1 });
  const result = merge({ session: null }, { session: { safe: 1, ...poison() } });
  assertClean(result.session);
  assert.equal(result.session.safe, 1);
});

test('issue 156 fails closed when target property introspection throws', () => {
  const target = new Proxy({}, { has() { throw new Error('untrusted introspection'); } });
  const result = merge(target, { ignored: { safe: 1 } });
  assertClean(result);
  assert.deepEqual(result, {});
});
