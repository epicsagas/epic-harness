import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// This is not a package-wide audit ignore. The CLI first executes behavioral,
// hash, lock and real-consumer checks, then permits this one verified backport.
export function classifyDashboardAudit(report) {
  if (!report || !report.advisories || typeof report.advisories !== 'object'
      || Array.isArray(report.advisories) || !report.metadata?.vulnerabilities) {
    throw new Error('Unsupported dashboard audit report');
  }
  const advisories = Object.values(report.advisories);
  const counts = Object.values(report.metadata.vulnerabilities);
  if (counts.some(n => !Number.isInteger(n) || n < 0)
      || counts.reduce((a, b) => a + b, 0) !== advisories.length) {
    throw new Error('Inconsistent dashboard audit report; refusing empty/partial success');
  }
  const isBackported = advisory => advisory.module_name === 'deepmerge'
    && advisory.github_advisory_id === 'GHSA-gcqx-w48m-w2h4'
    && Array.isArray(advisory.findings) && advisory.findings.length > 0
    && advisory.findings.every(finding => finding.version === '4.3.1');
  return {
    verifiedSourceBackports: advisories.filter(isBackported).map(advisory => ({
      advisory: advisory.github_advisory_id, package: advisory.module_name, version: '4.3.1',
    })),
    remaining: advisories.filter(advisory => !isBackported(advisory)),
  };
}

function main() {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const regression = spawnSync(process.execPath, ['--test', 'scripts/deepmerge-security.test.mjs'], {
    cwd: root, encoding: 'utf8', windowsHide: true,
  });
  process.stdout.write(regression.stdout ?? '');
  process.stderr.write(regression.stderr ?? '');
  if (regression.status !== 0) throw new Error('Reviewed dependency backport verification failed');
  const audit = process.platform === 'win32'
    ? spawnSync('cmd.exe', ['/d', '/s', '/c', 'pnpm audit --json'], {
      cwd: path.join(root, 'app'), encoding: 'utf8', windowsHide: true,
    })
    : spawnSync('pnpm', ['audit', '--json'], { cwd: path.join(root, 'app'), encoding: 'utf8' });
  if (audit.error || audit.signal || ![0, 1].includes(audit.status)) {
    throw new Error('Dashboard dependency audit did not complete successfully');
  }
  const result = classifyDashboardAudit(JSON.parse(audit.stdout));
  console.log(JSON.stringify(result, null, 2));
  if (result.remaining.length > 0) process.exitCode = 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
