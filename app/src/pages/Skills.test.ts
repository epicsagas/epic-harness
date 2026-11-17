/// <reference types="node" />
import { readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, it, expect } from 'vitest';
import { SKILLS } from '../lib/skillCatalog.js';
import en from '../lib/locales/en.js';

const PIPELINE = ['discover', 'spec', 'go', 'audit', 'eval', 'ship', 'orbit', 'evolve', 'team'];

describe('Skills catalog', () => {
  it('lists exactly the shipped skills in the repo skills/ directory', () => {
    const dir = fileURLToPath(new URL('../../../skills', import.meta.url));
    const shipped = readdirSync(dir, { withFileTypes: true })
      .filter((d) => d.isDirectory() && !d.name.startsWith('_'))
      .map((d) => d.name)
      .sort();
    const names = SKILLS.filter((s) => s.name !== '_dispatch').map((s) => s.name).sort();
    expect(names).toEqual(shipped);
  });

  it('_dispatch is first and marked special', () => {
    expect(SKILLS[0].name).toBe('_dispatch');
    expect(SKILLS[0].special).toBe(true);
  });

  it('has exactly the 9 pipeline skills', () => {
    expect(SKILLS.filter((s) => s.pipeline).map((s) => s.name)).toEqual(PIPELINE);
  });

  it('en pageSkillsDesc19 states the pipeline and non-special skill counts', () => {
    const pipeline = SKILLS.filter((s) => s.pipeline).length;
    const total = SKILLS.filter((s) => !s.special).length;
    expect(en.pageSkillsDesc19).toContain(`${total} skills`);
    expect(en.pageSkillsDesc19).toContain(`${pipeline} pipeline`);
  });

  it('every descKey exists in en', () => {
    for (const s of SKILLS) {
      expect(en, s.descKey).toHaveProperty(s.descKey);
    }
  });
});
