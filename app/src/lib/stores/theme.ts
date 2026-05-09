import { writable } from 'svelte/store';

type Theme = 'dark' | 'light' | 'system';

function getSystemTheme(): 'dark' | 'light' {
  if (typeof window === 'undefined') return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(theme: Theme) {
  if (typeof document === 'undefined') return;
  const resolved = theme === 'system' ? getSystemTheme() : theme;
  document.documentElement.setAttribute('data-theme', resolved);
}

function getStoredTheme(): Theme {
  if (typeof localStorage === 'undefined') return 'dark';
  return (localStorage.getItem('graphos-theme') as Theme) ?? 'dark';
}

export const theme = writable<Theme>(getStoredTheme());

export function initTheme() {
  const stored = getStoredTheme();
  theme.set(stored);
  applyTheme(stored);

  theme.subscribe((t) => {
    localStorage.setItem('graphos-theme', t);
    applyTheme(t);
  });

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    const current = getStoredTheme();
    if (current === 'system') applyTheme('system');
  });
}

export function setTheme(t: Theme) {
  theme.set(t);
}
