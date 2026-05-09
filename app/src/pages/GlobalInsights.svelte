<script lang="ts">
  import { onMount } from 'svelte';
  import { stats, graphData, loadStats, loadGraph } from '$lib/stores/graph';
  import { getColor } from '$lib/d3/force-graph';

  onMount(() => {
    loadStats();
    loadGraph();
  });
</script>

<div class="flex-1 overflow-y-auto p-6">
  <!-- Hero Metrics -->
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
    <div class="glass-panel p-6 rounded-xl flex flex-col">
      <div class="flex justify-between items-start mb-2">
        <span class="text-on-surface-variant uppercase tracking-wider text-[11px]">Total Entities</span>
        <span class="material-symbols-outlined text-primary">data_object</span>
      </div>
      <div class="flex items-baseline gap-2">
        <span class="text-2xl font-bold">{$stats?.total_nodes?.toLocaleString() ?? '0'}</span>
      </div>
      <div class="h-1 bg-surface-variant mt-auto rounded-full overflow-hidden">
        <div class="h-full bg-primary w-3/4 rounded-full"></div>
      </div>
    </div>

    <div class="glass-panel p-6 rounded-xl flex flex-col">
      <div class="flex justify-between items-start mb-2">
        <span class="text-on-surface-variant uppercase tracking-wider text-[11px]">Total Relationships</span>
        <span class="material-symbols-outlined text-secondary">account_tree</span>
      </div>
      <div class="flex items-baseline gap-2">
        <span class="text-2xl font-bold">{$stats?.total_edges?.toLocaleString() ?? '0'}</span>
      </div>
      <div class="h-1 bg-surface-variant mt-auto rounded-full overflow-hidden">
        <div class="h-full bg-secondary w-1/2 rounded-full"></div>
      </div>
    </div>

    <div class="glass-panel p-6 rounded-xl flex flex-col">
      <div class="flex justify-between items-start mb-2">
        <span class="text-on-surface-variant uppercase tracking-wider text-[11px]">Avg Importance</span>
        <span class="material-symbols-outlined text-tertiary">hub</span>
      </div>
      <div class="flex items-baseline gap-2">
        <span class="text-2xl font-bold">{$stats?.avg_importance?.toFixed(3) ?? '0'}</span>
      </div>
      <div class="h-1 bg-surface-variant mt-auto rounded-full overflow-hidden">
        <div class="h-full bg-tertiary w-1/4 rounded-full"></div>
      </div>
    </div>

    <div class="glass-panel p-6 rounded-xl flex flex-col">
      <div class="flex justify-between items-start mb-2">
        <span class="text-on-surface-variant uppercase tracking-wider text-[11px]">Node Types</span>
        <span class="material-symbols-outlined text-primary">speed</span>
      </div>
      <div class="flex items-baseline gap-2">
        <span class="text-2xl font-bold">{Object.keys($stats?.by_type ?? {}).length}</span>
        <span class="text-on-surface-variant text-sm">defined</span>
      </div>
      <div class="h-1 bg-surface-variant mt-auto rounded-full overflow-hidden">
        <div class="h-full bg-primary-container w-2/3 rounded-full"></div>
      </div>
    </div>
  </div>

  <!-- Bento Grid -->
  <div class="grid grid-cols-12 gap-6">
    <!-- Graph Preview -->
    <div class="col-span-12 lg:col-span-8 glass-panel rounded-xl flex flex-col relative overflow-hidden h-[400px]">
      <div class="p-4 flex justify-between items-center border-b border-outline-variant/30">
        <h3 class="font-bold text-on-surface flex items-center gap-2">
          <span class="material-symbols-outlined text-secondary text-sm">share</span>
          Spatial Node Distribution
        </h3>
      </div>
      <div class="flex-1 relative bg-[radial-gradient(circle_at_center,_var(--color-surface-container)_0%,_var(--color-surface-container-lowest)_100%)] overflow-hidden flex items-center justify-center">
        {#if $graphData.nodes.length > 0}
          <div class="text-on-surface-variant text-sm">
            {$graphData.nodes.length} nodes, {$graphData.edges.length} edges loaded
          </div>
        {:else}
          <div class="text-on-surface-variant text-sm">No graph data available</div>
        {/if}
      </div>
    </div>

    <!-- Type Distribution -->
    <div class="col-span-12 lg:col-span-4 glass-panel rounded-xl flex flex-col overflow-hidden">
      <div class="p-4 flex justify-between items-center border-b border-outline-variant/30">
        <h3 class="font-bold text-on-surface flex items-center gap-2">
          <span class="material-symbols-outlined text-tertiary text-sm">star</span>
          Type Distribution
        </h3>
      </div>
      <div class="flex-1 overflow-y-auto p-4 space-y-3">
        {#if $stats?.by_type}
          {#each Object.entries($stats.by_type) as [type, count]}
            <div class="flex items-center justify-between p-3 bg-surface-container rounded-lg">
              <div class="flex items-center gap-3">
                <div class="w-8 h-8 rounded-full flex items-center justify-center" style="background: {getColor(type)}20; color: {getColor(type)}">
                  <span class="material-symbols-outlined text-sm">category</span>
                </div>
                <span class="text-sm font-medium capitalize">{type}</span>
              </div>
              <span class="font-mono text-secondary font-bold text-sm">{count}</span>
            </div>
          {/each}
        {:else}
          <div class="text-on-surface-variant text-sm text-center py-8">No type data</div>
        {/if}
      </div>
    </div>
  </div>
</div>
