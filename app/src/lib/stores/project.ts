import { writable } from 'svelte/store';

const STORAGE_KEY = 'epic-harness-selected-project';
const DEFAULT_PROJECT = '__all__';

function loadSavedProject(): string {
  if (typeof window === 'undefined') return DEFAULT_PROJECT;
  try {
    return localStorage.getItem(STORAGE_KEY) ?? DEFAULT_PROJECT;
  } catch {
    return DEFAULT_PROJECT;
  }
}

export const projectList = writable<string[]>([]);
export const selectedProject = writable<string>(loadSavedProject());

// Persist selection changes to localStorage
if (typeof window !== 'undefined') {
  selectedProject.subscribe((value) => {
    try { localStorage.setItem(STORAGE_KEY, value); } catch { /* ignore */ }
  });
}

/** Fetch the project list from the backend. */
export async function loadProjects(): Promise<void> {
  try {
    // Tauri runtime: use invoke bridge (no HTTP server available)
    const isTauri = typeof window !== 'undefined' &&
      !!((window as unknown as Record<string, unknown>)['__TAURI_INTERNALS__']);
    if (isTauri) {
      const { invoke } = await import('@tauri-apps/api/core');
      const slugs: string[] = await invoke('list_projects');
      projectList.set(slugs.sort());
      return;
    }
    // Browser / Vite dev: use HTTP endpoint
    const res = await fetch('/api/projects');
    if (!res.ok) return;
    const slugs: string[] = await res.json();
    projectList.set(slugs.sort());
  } catch { /* ignore */ }
}
