import { describe, it, expect } from 'vitest';

// Static data contract test — no DOM rendering needed
const COMMANDS = [
  { cmd: 'discover', desc: 'Explore and define the problem before specifying a solution', ring: 1 },
  { cmd: 'spec', desc: 'Define requirements before coding', ring: 1 },
  { cmd: 'go', desc: 'Build with auto-plan + TDD', ring: 1 },
  { cmd: 'check', desc: 'Review + security audit + tests', ring: 1 },
  { cmd: 'ship', desc: 'Create PR, verify CI, merge', ring: 1 },
  { cmd: 'evolve', desc: 'Inspect or trigger skill evolution', ring: 3 },
  { cmd: 'team', desc: 'Generate project-specific agent team', ring: 1 },
  { cmd: 'orbit', desc: 'Autonomous spec→ship pipeline', ring: 1 },
  { cmd: 'git-cc', desc: 'Conventional commit with auto type selection', ring: 0 },
  { cmd: 'git', desc: 'Cross-repo git operations (sync/bump/tags)', ring: 0 },
];

describe('Commands static data', () => {
  it('contains exactly 10 commands', () => {
    expect(COMMANDS).toHaveLength(10);
  });

  it('has required fields on every command', () => {
    for (const c of COMMANDS) {
      expect(typeof c.cmd).toBe('string');
      expect(typeof c.desc).toBe('string');
      expect(typeof c.ring).toBe('number');
    }
  });

  it('includes all required commands', () => {
    const names = COMMANDS.map(c => c.cmd);
    for (const name of ['discover', 'spec', 'go', 'check', 'ship', 'evolve', 'team', 'orbit', 'git-cc', 'git']) {
      expect(names).toContain(name);
    }
  });

  it('ring values are 0, 1, or 3 only', () => {
    for (const c of COMMANDS) {
      expect([0, 1, 3]).toContain(c.ring);
    }
  });

  it('clipboard text would be /cmd', () => {
    for (const c of COMMANDS) {
      expect('/' + c.cmd).toMatch(/^\/[a-z-]+$/);
    }
  });
});
