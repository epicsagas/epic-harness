<script lang="ts">
  interface Props {
    currentScreen: string;
    onNavigate: (screen: string) => void;
  }

  let { currentScreen, onNavigate }: Props = $props();

  const navSections = [
    {
      label: 'Overview',
      ringDot: false,
      items: [
        { id: 'dashboard', icon: '■', label: 'Dashboard', badge: null },
        { id: 'orbit', icon: '◉', label: '/orbit Pipeline', badge: 'auto' },
      ],
    },
    {
      label: 'Ring 1 · Commands',
      ringDot: true,
      dotColor: 'var(--accent)',
      items: [
        { id: 'commands', icon: '▶', label: 'Commands', badge: '3' },
      ],
    },
    {
      label: 'Ring 2 · Auto Skills',
      ringDot: true,
      dotColor: 'var(--purple)',
      items: [
        { id: 'skills', icon: '✧', label: 'Skills', badge: '19' },
      ],
    },
    {
      label: 'Agents',
      ringDot: true,
      dotColor: 'var(--teal)',
      items: [
        { id: 'agents', icon: '★', label: 'Live Agents', badge: 'live' },
      ],
    },
    {
      label: 'Ring 3 · Evolution',
      ringDot: true,
      dotColor: 'var(--orange)',
      items: [
        { id: 'evolution', icon: '↻', label: 'Eval & Evolve', badge: null },
      ],
    },
    {
      label: 'Ring 0 · Autopilot',
      ringDot: true,
      dotColor: 'var(--success)',
      items: [
        { id: 'hooks', icon: '⚡', label: 'Hooks', badge: null },
      ],
    },
    {
      label: 'Data Layer',
      ringDot: false,
      items: [
        { id: 'memory', icon: '◆', label: 'harness-mem', badge: null },
        { id: 'integrations', icon: '☷', label: 'Integrations', badge: '6' },
      ],
    },
    {
      label: 'System',
      ringDot: false,
      items: [
        { id: 'settings', icon: '⚙', label: 'Settings', badge: null },
      ],
    },
  ];
</script>

<div class="sidebar-brand">
  <div class="logo-icon">EH</div>
  <h1>epic-harness</h1>
  <span class="version">v{__APP_VERSION__}</span>
</div>

<nav class="sidebar-nav">
  {#each navSections as section}
    <div class="nav-section-label">
      {#if section.ringDot}
        <span class="ring-dot" style="background:{section.dotColor}"></span>
      {/if}
      {section.label}
    </div>
    {#each section.items as item}
      <!-- svelte-ignore a11y_invalid_attribute -->
      <a
        class="nav-item"
        class:active={currentScreen === item.id}
        href="#"
        onclick={(e) => { e.preventDefault(); onNavigate(item.id); }}
      >
        <span class="nav-icon">{item.icon}</span>
        {item.label}
        {#if item.badge}
          <span class="badge">{item.badge}</span>
        {/if}
      </a>
    {/each}
  {/each}
</nav>

<div class="sidebar-footer">
  <span class="status-dot"></span>
  <span class="status-text">all systems nominal</span>
</div>
