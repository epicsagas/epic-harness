import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

// ZIP stored entries keep the fixture independent of a compression module or
// external archiver. Extraction still uses the production PowerShell command.
function zip(entries) {
  const records = [], directory = [];let offset = 0;
  for (const [name, value] of entries) {
    const filename = Buffer.from(name), content = Buffer.from(value);
    let crc = 0xffffffff;
    for (const b of content) { crc ^= b;for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
    crc = (crc ^ 0xffffffff) >>> 0;
    const local = Buffer.alloc(30);local.writeUInt32LE(0x04034b50);local.writeUInt16LE(20, 4);local.writeUInt16LE(33, 12);local.writeUInt32LE(crc, 14);local.writeUInt32LE(content.length, 18);local.writeUInt32LE(content.length, 22);local.writeUInt16LE(filename.length, 26);
    records.push(local, filename, content);
    const central = Buffer.alloc(46);central.writeUInt32LE(0x02014b50);central.writeUInt16LE(20, 4);central.writeUInt16LE(20, 6);central.writeUInt16LE(33, 14);central.writeUInt32LE(crc, 16);central.writeUInt32LE(content.length, 20);central.writeUInt32LE(content.length, 24);central.writeUInt16LE(filename.length, 28);central.writeUInt32LE(offset, 42);directory.push(central, filename);
    offset += local.length + filename.length + content.length;
  }
  const central = Buffer.concat(directory), end = Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(entries.length, 8);end.writeUInt16LE(entries.length, 10);end.writeUInt32LE(central.length, 12);end.writeUInt32LE(offset, 16);
  return Buffer.concat([...records, central, end]);
}

test('Windows installer reports locked replacement and hash errors without claiming success', { skip: process.platform !== 'win32' }, () => {
  const source = fs.readFileSync(new URL('../../../install.ps1', import.meta.url), 'utf8').replace(/param\([\s\S]*?\r?\n\)/, '');
  for (const scenario of ['fresh', 'repeat', 'locked', 'bad-hash']) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'epic-installer-'));
    try {
      const script = path.join(root, 'fixture.ps1');
      fs.writeFileSync(path.join(root, 'fixture.zip'), zip(['epic.exe', 'epic-harness.exe'].map(name => [name, 'SYNTHETIC NEW BINARY'])));
      const prefix = String.raw`
$fixtureRoot = $env:EPIC_INSTALL_FIXTURE
$scenario = $env:EPIC_INSTALL_SCENARIO
$InstallDir = Join-Path $fixtureRoot install
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
$fixtureZip = Join-Path $fixtureRoot fixture.zip
$fixtureHash = (Get-FileHash -LiteralPath $fixtureZip -Algorithm SHA256).Hash.ToLower()
function Invoke-RestMethod { return @{ tag_name='v9.9.9' } }
function Invoke-WebRequest {
 param([string]$Uri,[string]$OutFile,[switch]$UseBasicParsing)
 if ($Uri.EndsWith('.sha256')) {
   if ($scenario -eq 'bad-hash') { Set-Content -LiteralPath $OutFile -Value ('0'*64 + ' fixture.zip') }
   else { Set-Content -LiteralPath $OutFile -Value ($fixtureHash + ' fixture.zip') }
 } else { Copy-Item -LiteralPath $fixtureZip -Destination $OutFile }
}
if ($scenario -ne 'fresh') { Set-Content -LiteralPath (Join-Path $InstallDir epic.exe) -Value 'SYNTHETIC OLD BINARY' -NoNewline }
if ($scenario -eq 'locked') { $ownedLock = [IO.File]::Open((Join-Path $InstallDir epic.exe),[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None) }
`;
      fs.writeFileSync(script, prefix + '\n' + source);
      const home = path.join(root, 'home'), data = path.join(root, 'data');
      fs.mkdirSync(path.join(home, '.config/epic-harness'), { recursive: true });fs.mkdirSync(data);
      fs.writeFileSync(path.join(home, '.config/epic-harness/telemetry-consent'), 'off');
      fs.writeFileSync(path.join(data, 'config.toml'), '[dashboard]\nport=0\nauto_open=false\n');
      const env = { ...process.env, HOME: home, USERPROFILE: home, HARNESS_DIR: data, HARNESS_ROOT: root, TEMP: root, TMP: root, EPIC_INSTALL_FIXTURE: root, EPIC_INSTALL_SCENARIO: scenario };
      for (const k of Object.keys(env)) if (k.toLowerCase() === 'path') delete env[k];
      env.PATH = path.join(root, 'install') + path.delimiter + path.join(process.env.SystemRoot, 'System32');
      const result = spawnSync(path.join(process.env.SystemRoot, 'System32/WindowsPowerShell/v1.0/powershell.exe'), ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', script], { env, cwd: root, windowsHide: true, encoding: 'utf8', timeout: 45000 });
      const contents = fs.existsSync(path.join(root, 'install/epic.exe')) ? fs.readFileSync(path.join(root, 'install/epic.exe'), 'utf8') : null;
      if (scenario === 'locked' || scenario === 'bad-hash') {
        assert.notEqual(result.status, 0, `${scenario}: ${result.stdout}\n${result.stderr}`);
        assert.doesNotMatch(result.stdout, /Installed:/, 'a failed copy/hash check must never claim installation');
        assert.equal(contents, 'SYNTHETIC OLD BINARY');
      } else {
        assert.equal(result.status, 0, `${scenario}: ${result.stderr}`);
        assert.equal(contents, 'SYNTHETIC NEW BINARY');
        assert.match(result.stdout, /Installed:/);
      }
    } finally {
      assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
});
