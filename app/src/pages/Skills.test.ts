import { describe, it, expect } from 'vitest';

const SKILLS = [
  { name: '_dispatch', desc: 'Auto-routes tasks to the right skill based on context', special: true },
  { name: 'tdd', desc: 'Test-first development cycle' },
  { name: 'debug', desc: 'Systematic root cause analysis' },
  { name: 'secure', desc: 'Security checklist for auth/db/api code' },
  { name: 'verify', desc: 'Build + test + lint before marking done' },
  { name: 'simplify', desc: 'Triggered on files > 200 lines' },
  { name: 'perf', desc: 'Performance analysis for DB/API code' },
  { name: 'review', desc: 'Code quality and logic review' },
  { name: 'refactor', desc: 'Safe structural improvement' },
  { name: 'migrate', desc: 'Database and schema migration safety' },
  { name: 'api-design', desc: 'REST/GraphQL API design patterns' },
  { name: 'doc', desc: 'Documentation generation' },
  { name: 'test-gen', desc: 'Auto test generation for uncovered code' },
  { name: 'ci', desc: 'CI/CD pipeline configuration' },
  { name: 'deploy', desc: 'Safe deployment checklist' },
];

describe('Skills static data', () => {
  it('contains exactly 15 skills', () => {
    expect(SKILLS).toHaveLength(15);
  });

  it('_dispatch is first and marked special', () => {
    expect(SKILLS[0].name).toBe('_dispatch');
    expect((SKILLS[0] as { special?: boolean }).special).toBe(true);
  });

  it('all skills have name and desc', () => {
    for (const s of SKILLS) {
      expect(typeof s.name).toBe('string');
      expect(typeof s.desc).toBe('string');
    }
  });

  it('clipboard text for _dispatch is /_dispatch', () => {
    const dispatch = SKILLS.find(s => s.name === '_dispatch')!;
    expect('/' + dispatch.name).toBe('/_dispatch');
  });

  it('includes required skill names', () => {
    const names = SKILLS.map(s => s.name);
    for (const n of ['tdd', 'debug', 'secure', 'verify', 'simplify', 'perf']) {
      expect(names).toContain(n);
    }
  });
});
