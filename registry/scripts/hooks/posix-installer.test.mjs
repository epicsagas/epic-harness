import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const shell = process.platform === 'win32' ? 'C:/Program Files/Git/bin/bash.exe' : '/bin/sh';
test('POSIX release installer preserves both executable interfaces and unrelated files on repeat', { skip: !fs.existsSync(shell) }, () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-posix-install-'));
  try {
    const bin = path.join(root, 'fake-bin');fs.mkdirSync(bin);
    const installer = fileURLToPath(new URL('../../../install.sh', import.meta.url)).replaceAll('\\', '/');
    fs.writeFileSync(path.join(bin, 'uname'), '#!/bin/sh\nif [ "$1" = -m ]; then echo x86_64; else echo Linux; fi\n', { mode: 0o755 });
    fs.writeFileSync(path.join(bin, 'curl'), '#!/bin/sh\nshift\ncase "$1" in */releases/latest) echo \'{"tag_name":"v9.9.9"}\';; *) cp "$FIXTURE/${1##*/}" "$3";; esac\n', { mode: 0o755 });
    const env = { ...process.env, HOME: path.join(root, 'home'), USERPROFILE: path.join(root, 'home'), HARNESS_DIR: path.join(root, 'data'), HARNESS_ROOT: root, FIXTURE: root.replaceAll('\\', '/'), INSTALL_DIR: path.join(root, 'install').replaceAll('\\', '/') };
    const setup = `set -eu
cd "$FIXTURE"
mkdir -p payload/epic-harness-x86_64-unknown-linux-gnu install home/.config/epic-harness data
printf off >home/.config/epic-harness/telemetry-consent
printf '[dashboard]\\nport=0\\nauto_open=false\\n' >data/config.toml
printf '#!/bin/sh\\necho epic-harness 9.9.9\\n' >payload/epic-harness-x86_64-unknown-linux-gnu/epic
cp payload/epic-harness-x86_64-unknown-linux-gnu/epic payload/epic-harness-x86_64-unknown-linux-gnu/epic-harness
tar -cJf epic-harness-x86_64-unknown-linux-gnu.tar.xz -C payload epic-harness-x86_64-unknown-linux-gnu
sha256sum epic-harness-x86_64-unknown-linux-gnu.tar.xz >epic-harness-x86_64-unknown-linux-gnu.tar.xz.sha256
printf 'unrelated' >install/unrelated.txt
`;
    const ready = spawnSync(shell, ['-c', setup], { cwd: root, env, encoding: 'utf8', timeout: 20000, windowsHide: true });
    assert.equal(ready.status, 0, ready.stderr);
    for (let repeat = 0; repeat < 2; repeat++) {
      const r = spawnSync(shell, ['-c', `if command -v cygpath >/dev/null 2>&1; then FIXTURE="$(cygpath -u "$FIXTURE")"; INSTALL_DIR="$(cygpath -u "$INSTALL_DIR")"; export FIXTURE INSTALL_DIR; fi; export PATH="$FIXTURE/fake-bin:$PATH"; exec sh "$1"`, 'installer-fixture', installer], { cwd: root, env, encoding: 'utf8', timeout: 30000, windowsHide: true });
      assert.equal(r.status, 0, r.stdout + r.stderr);
      assert.equal(fs.existsSync(path.join(root, 'install/epic-harness')), true, 'MCP/bootstrap also require the long executable name');
      assert.equal(fs.readFileSync(path.join(root, 'install/epic'), 'utf8'), fs.readFileSync(path.join(root, 'install/epic-harness'), 'utf8'));
      assert.equal(fs.readFileSync(path.join(root, 'install/unrelated.txt'), 'utf8'), 'unrelated');
    }
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
    fs.rmSync(root, { recursive: true, force: true });
  }
});
