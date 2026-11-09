// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import fs from 'node:fs';
import Skills from './Skills.svelte';
afterEach(() => cleanup());
it('shows exactly the shipped skill registry and copies real command names', async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  const view = render(Skills);
  const cards = [...view.container.querySelectorAll<HTMLElement>('.skill-card')];
  const names = cards.map(card => card.dataset.skill ?? card.querySelector('.skill-name')?.firstChild?.textContent?.trim());
  const shipped = fs.readdirSync('../skills', { withFileTypes: true }).filter(d => d.isDirectory()).map(d => d.name).sort();
  expect([...names].sort()).toEqual(shipped);
  const audit = cards.find(card => card.dataset.skill === 'audit')!;
  await fireEvent.click(audit);
  expect(writeText).toHaveBeenCalledWith('/audit');
  expect(names).not.toContain('review');
});
