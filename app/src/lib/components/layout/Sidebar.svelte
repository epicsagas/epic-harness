<script lang="ts">
  import { navigate } from '$lib/router';

  interface Props {
    route: string;
  }

  let { route }: Props = $props();

  const navItems = [
    { icon: 'dashboard', label: 'Insights', path: '/' },
    { icon: 'hub', label: 'Explorer', path: '/explorer' },
    { icon: 'analytics', label: 'Intelligence', path: '/entity' },
    { icon: 'schema', label: 'Ontology', path: '/ontology' },
  ];

  function isActive(path: string): boolean {
    if (path === '/') return route === '/' || route === '';
    return route.startsWith(path);
  }

  function handleNav(path: string, e: MouseEvent) {
    e.preventDefault();
    navigate(path);
  }
</script>

<aside class="flex flex-col h-full w-64 border-r border-outline-variant bg-surface-container-lowest shrink-0 overflow-y-auto">
  <div class="p-6">
    <h1 class="text-2xl font-bold text-primary font-[Inter]">GraphOS</h1>
    <p class="text-sm text-on-surface-variant mt-1">Intelligence Suite</p>
  </div>

  <nav class="flex-1 px-4 space-y-1" aria-label="Main navigation">
    <button
      onclick={() => navigate('/')}
      class="w-full flex items-center justify-center gap-2 bg-primary text-on-primary font-bold py-3 px-6 rounded-lg hover:opacity-90 transition-all active:scale-95 duration-150"
    >
      <span class="material-symbols-outlined">add</span>
      <span>New Analysis</span>
    </button>

    <ul class="mt-6 space-y-1 list-none p-0" role="list">
      {#each navItems as item (item.path)}
        <li>
          <a
            href={'#' + item.path}
            onclick={(e) => handleNav(item.path, e)}
            aria-current={isActive(item.path) ? 'page' : undefined}
            class="flex items-center gap-3 p-3 rounded-lg transition-colors text-body-md no-underline
              {isActive(item.path)
                ? 'text-primary border-r-2 border-primary bg-primary/5 font-bold'
                : 'text-on-surface-variant hover:bg-surface-variant/50'}"
          >
            <span class="material-symbols-outlined">{item.icon}</span>
            <span>{item.label}</span>
          </a>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="mt-auto p-4 space-y-1 border-t border-outline-variant">
    <button class="w-full flex items-center gap-3 p-3 rounded-lg text-on-surface-variant hover:bg-surface-variant/50 transition-colors" aria-label="Settings">
      <span class="material-symbols-outlined">settings</span>
      <span class="text-sm">Settings</span>
    </button>
    <button class="w-full flex items-center gap-3 p-3 rounded-lg text-on-surface-variant hover:bg-surface-variant/50 transition-colors" aria-label="Support">
      <span class="material-symbols-outlined">help</span>
      <span class="text-sm">Support</span>
    </button>
  </div>
</aside>
