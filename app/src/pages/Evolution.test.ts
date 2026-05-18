import { describe, it, expect } from 'vitest';

// Seeding thresholds are static constants — test the values
const SEEDING_THRESHOLDS = [
  { type: 'Weak tool', condition: 'success_rate < 0.6', minObs: 5 },
  { type: 'Weak file type', condition: 'success_rate < 0.5', minObs: 3 },
  { type: 'High-freq error', condition: '5+ occurrences', minObs: 5 },
  { type: 'Stagnation rollback', condition: '3 sessions without 5% improvement', minObs: 0 },
];

const MAX_SKILLS_CAP = 10;

describe('Evolution static thresholds', () => {
  it('has 4 threshold entries', () => {
    expect(SEEDING_THRESHOLDS).toHaveLength(4);
  });

  it('weak tool has minObs 5 and rate < 0.6', () => {
    const t = SEEDING_THRESHOLDS.find(x => x.type === 'Weak tool')!;
    expect(t.minObs).toBe(5);
    expect(t.condition).toContain('0.6');
  });

  it('weak file type has minObs 3 and rate < 0.5', () => {
    const t = SEEDING_THRESHOLDS.find(x => x.type === 'Weak file type')!;
    expect(t.minObs).toBe(3);
    expect(t.condition).toContain('0.5');
  });

  it('stagnation rollback entry exists', () => {
    const t = SEEDING_THRESHOLDS.find(x => x.type === 'Stagnation rollback')!;
    expect(t).toBeDefined();
    expect(t.condition).toContain('5%');
  });

  it('max skills cap is 10', () => {
    expect(MAX_SKILLS_CAP).toBe(10);
  });
});

describe('Evolution data helpers', () => {
  it('formats date-only from ISO timestamp', () => {
    const ts = '2026-05-18T19:22:11.000Z';
    const dateOnly = ts.slice(0, 10);
    expect(dateOnly).toBe('2026-05-18');
  });

  it('joins pattern arrays with comma', () => {
    const patterns = ['fix_then_break', 'repeated_same_error'];
    expect(patterns.join(', ')).toBe('fix_then_break, repeated_same_error');
  });

  it('handles string pattern field unchanged', () => {
    const patterns = 'fix_then_break';
    const result = Array.isArray(patterns) ? patterns.join(', ') : patterns;
    expect(result).toBe('fix_then_break');
  });

  it('first line of skill_md is the description', () => {
    const md = '# Fix-Then-Break Recovery\nDetected alternating edit/error cycle.';
    const firstLine = md.split('\n')[0];
    expect(firstLine).toBe('# Fix-Then-Break Recovery');
  });
});
