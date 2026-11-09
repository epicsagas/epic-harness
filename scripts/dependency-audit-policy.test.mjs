import assert from 'node:assert/strict';
import test from 'node:test';
import { classifyDashboardAudit } from './audit-dashboard-dependencies.mjs';

const report = (advisories, total = Object.keys(advisories).length) => ({
  advisories,
  metadata: { vulnerabilities: { info: 0, low: total, moderate: 0, high: 0, critical: 0 } },
});
const backported = () => ({ module_name: 'deepmerge', github_advisory_id: 'GHSA-gcqx-w48m-w2h4', findings: [{ version: '4.3.1' }] });

test('issue 156 audit exception is limited to the exact patched advisory and version', () => {
  assert.equal(classifyDashboardAudit(report({ one: backported() })).remaining.length, 0);
  assert.equal(classifyDashboardAudit(report({})).remaining.length, 0);
});

test('issue 156 audit gate rejects unrelated advisories and misleading package/version matches', () => {
  for (const advisory of [
    { ...backported(), github_advisory_id: 'GHSA-other-advisory-id' },
    { ...backported(), module_name: 'another-package' },
    { ...backported(), findings: [{ version: '4.2.0' }] },
  ]) assert.equal(classifyDashboardAudit(report({ one: advisory })).remaining.length, 1);
});

test('issue 156 audit gate rejects empty or unsupported reports rather than returning false green', () => {
  assert.throws(() => classifyDashboardAudit({}), /unsupported/i);
  assert.throws(() => classifyDashboardAudit(report({}, 1)), /inconsistent/i);
  assert.throws(() => classifyDashboardAudit(report({ one: backported() }, 0)), /inconsistent/i);
});
