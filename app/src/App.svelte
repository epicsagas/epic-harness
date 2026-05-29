<script lang="ts">
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import Topbar from '$lib/components/layout/Topbar.svelte';
  import Dashboard from './pages/Dashboard.svelte';
  import OrbitPipeline from './pages/OrbitPipeline.svelte';
  import Commands from './pages/Commands.svelte';
  import Skills from './pages/Skills.svelte';
  import Agents from './pages/Agents.svelte';
  import Evolution from './pages/Evolution.svelte';
  import Hooks from './pages/Hooks.svelte';
  import Memory from './pages/Memory.svelte';
  import Integrations from './pages/Integrations.svelte';
  import Settings from './pages/Settings.svelte';
  import Toast from '$lib/components/Toast.svelte';

  let sidebarOpen = $state(false);

  const screens: Record<string, any> = {
    dashboard: Dashboard,
    orbit: OrbitPipeline,
    commands: Commands,
    skills: Skills,
    agents: Agents,
    evolution: Evolution,
    hooks: Hooks,
    memory: Memory,
    integrations: Integrations,
    settings: Settings,
  };

  const screenLabels: Record<string, string> = {
    dashboard: 'dashboard',
    orbit: '/orbit pipeline',
    commands: 'pipeline',
    skills: 'auto skills',
    agents: 'agents',
    evolution: 'eval & evolve',
    hooks: 'hooks',
    memory: 'harness-mem',
    integrations: 'integrations',
    settings: 'settings',
  };

  function screenFromPath(): string {
    const seg = window.location.pathname.replace(/^\//, '').split('/')[0];
    return seg in screens ? seg : 'dashboard';
  }

  let currentScreen = $state(screenFromPath());

  function navigate(screen: string) {
    const path = screen === 'dashboard' ? '/' : `/${screen}`;
    history.pushState({}, '', path);
    currentScreen = screen;
    sidebarOpen = false;
  }

  function onPopState() {
    currentScreen = screenFromPath();
  }

  import { onMount } from 'svelte';
  onMount(() => {
    window.addEventListener('popstate', onPopState);
    currentScreen = screenFromPath();
    return () => window.removeEventListener('popstate', onPopState);
  });

  let Page = $derived(screens[currentScreen] ?? Dashboard);
</script>

<div class="mobile-header">
  <button class="hamburger" onclick={() => sidebarOpen = !sidebarOpen}>&#9776;</button>
  <span style="font-weight:600;font-size:14px;">epic-harness</span>
  <span style="width:36px;"></span>
</div>

{#if sidebarOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="sidebar-overlay open" onclick={() => sidebarOpen = false} role="presentation"></div>
{/if}

<div class="app-shell">
  <aside class="sidebar" class:open={sidebarOpen}>
    <Sidebar {currentScreen} onNavigate={navigate} />
  </aside>
  <div class="main-content">
    <Topbar currentLabel={screenLabels[currentScreen]} />
    <div class="screen active">
      <Page />
    </div>
  </div>
</div>
<Toast />
