import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const read = relative => readFileSync(new URL('../' + relative, import.meta.url), 'utf8');
const base = 'vendor/libsqlite3-sys-0.30.1/';
const provenance = JSON.parse(read(base + 'EPIC_BACKPORT.json'));
assert.equal(provenance.archive_sha256, '2e99fb7a497b1e3339bc746195567ed8d3e24945ecd636e3619d20b9de9e9149');
assert.equal(provenance.engine_archive_sha256, '14a5f01b87b92705d07b433c06f96cbc764637d81a0bffb5734ce17cf0f9a2b3');
assert.equal(provenance.upstream_fix_commit, 'e5bf119821f25be1d7f10ef0a2edbe175289166e');
assert.equal(provenance.sqlite_version, '3.54.1');
const expected = {
  'sqlite3/sqlite3.c': '0c657666d819f261cc4007526ad57e1669de9949df7a8ea847aa284a2bf0ee52',
  'sqlite3/sqlite3.h': 'd6c84557261893439b341d2e6c35187ae3ea24fbfd843ad94616f72d89242b8c',
  'sqlite3/sqlite3ext.h': '48963abfefb41ac329ef9d706acb5eadba952a99a748285c9ded6b5e0942721b',
  'sqlite3/bindgen_bundled_version.rs': '9eed0560a520a59559ead391dfa41917209d6ea6e5e3b82cdc599f0d1f48f868',
  'sqlite3/bindgen_bundled_version_ext.rs': 'ed38360413f96856d1b7f6c2cd15c9105f90da7a30d2c0f950f31b375812a8e5',
};
assert.deepEqual(provenance.files_sha256, expected);
for (const [file, hash] of Object.entries(expected)) {
  assert.equal(createHash('sha256').update(read(base + file).replaceAll('\r\n', '\n')).digest('hex'), hash,
    `${file} changed: re-review the engine and bindings before accepting this advisory remediation`);
}
const source = read(base + 'sqlite3/sqlite3.c');
assert.match(source, /if\( nOut>sz \)\{\s*memset\(pOut\+sz, 0, nOut-sz\);\s*nOut = sz;/);
assert.match(read('Cargo.toml'), /\[patch\.crates-io\][\s\S]*libsqlite3-sys\s*=\s*\{\s*path\s*=\s*"vendor\/libsqlite3-sys-0\.30\.1"\s*\}/);
const entry = read('Cargo.lock').split('[[package]]').find(block => /^\s*name = "libsqlite3-sys"\s*$/m.test(block));
assert.ok(entry, 'SQLite must remain resolved in the workspace lockfile');
assert.match(entry, /version = "0\.30\.1"/);
assert.doesNotMatch(entry, /source = "registry|checksum = /, 'lockfile must select the reviewed source override');
for (const manifest of ['Cargo.toml', 'src-tauri/Cargo.toml']) {
  assert.doesNotMatch(read(manifest), /["'](?:bundled-)?sqlcipher(?:-vendored-openssl)?["']/,
    'SQLCipher is not covered by the reviewed SQLite source update');
}
console.log('PASS: SQLx retains the 0.30.1 bindings and resolves the reviewed SQLite 3.54.1 engine');
