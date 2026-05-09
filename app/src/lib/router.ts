import { writable } from 'svelte/store';

export const currentRoute = writable<{ path: string; params: Record<string, string> }>({
  path: '/',
  params: {},
});

const routePatterns: { path: string; pattern: RegExp; paramNames: string[] }[] = [];

function parsePattern(path: string) {
  const paramNames: string[] = [];
  const regex = path.replace(/:([^/]+)/g, (_m, name: string) => {
    paramNames.push(name);
    return '([^/]+)';
  });
  return { path, pattern: new RegExp(`^${regex}$`), paramNames };
}

export function addRoute(path: string) {
  routePatterns.push(parsePattern(path));
}

export function navigate(path: string) {
  window.location.hash = '#' + path;
}

export function initRouter() {
  const resolve = () => {
    const hash = window.location.hash.slice(1) || '/';
    for (const r of routePatterns) {
      const m = hash.match(r.pattern);
      if (m) {
        const params: Record<string, string> = {};
        r.paramNames.forEach((n, i) => (params[n] = m[i + 1]));
        currentRoute.set({ path: r.path, params });
        return;
      }
    }
    currentRoute.set({ path: hash, params: {} });
  };

  window.addEventListener('hashchange', resolve);
  resolve();
}
