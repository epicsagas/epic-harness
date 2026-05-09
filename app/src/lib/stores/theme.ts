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

export function initTheme(): () => void {
  const stored = getStoredTheme();
  theme.set(stored);
  applyTheme(stored);

  const unsub = theme.subscribe((t) => {
    localStorage.setItem('graphos-theme', t);
    applyTheme(t);
  });

  const mql = window.matchMedia('(prefers-color-scheme: dark)');
  const handler = (e: MediaQueryListEvent) => {
    if (!localStorage.getItem('graphos-theme')) {
      theme.set(e.matches ? 'dark' : 'light');
    }
  };
  mql.addEventListener('change', handler);

  return () => {
    unsub();
    mql.removeEventListener('change', handler);
  };
}

export function setTheme(t: Theme) {
  theme.set(t);
}
