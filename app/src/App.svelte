<script lang="ts">
  import { onMount } from 'svelte';
  import type { Component } from 'svelte';
  import { currentRoute, addRoute, initRouter } from '$lib/router';
  import { initTheme } from '$lib/stores/theme';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import Topbar from '$lib/components/layout/Topbar.svelte';
  import GlobalInsights from './pages/GlobalInsights.svelte';
  import EntityIntelligence from './pages/EntityIntelligence.svelte';
  import KnowledgeExplorer from './pages/KnowledgeExplorer.svelte';
  import OntologySources from './pages/OntologySources.svelte';

  addRoute('/');
  addRoute('/entity/:id');
  addRoute('/explorer');
  addRoute('/ontology');

  onMount(() => {
    initRouter();
    const cleanup = initTheme();
    return cleanup;
  });

  const pages: Record<string, Component> = {
    '/': GlobalInsights,
    '/entity/:id': EntityIntelligence,
    '/explorer': KnowledgeExplorer,
    '/ontology': OntologySources,
  };

  let Page = $derived(pages[$currentRoute.path] ?? GlobalInsights);
</script>

<div class="flex h-screen overflow-hidden bg-background text-on-surface font-body-md">
  <Sidebar route={$currentRoute.path} />

  <main class="flex-1 flex flex-col h-full relative overflow-hidden">
    <Topbar />
    <div class="flex-1 flex flex-col overflow-hidden">
      <Page />
    </div>
  </main>
</div>
