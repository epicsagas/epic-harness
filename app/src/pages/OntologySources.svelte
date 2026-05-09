<script lang="ts">
  import { onMount } from 'svelte';
  import { stats, loadStats } from '$lib/stores/graph';
  import { getColor } from '$lib/d3/force-graph';

  onMount(() => {
    loadStats();
  });

  const typeDefaults: Record<string, number> = {
    decision: 0.9,
    resolution: 0.8,
    concept: 0.7,
    project: 0.7,
    pattern: 0.5,
    error: 0.4,
    session: 0.05,
  };
</script>

<div class="flex-1 overflow-y-auto p-6">
  <div class="grid grid-cols-12 gap-6">
    <!-- Schema Manager -->
    <div class="col-span-12 lg:col-span-8 space-y-6">
      <div class="glass-panel rounded-xl p-6">
        <div class="flex justify-between items-center mb-6">
          <h2 class="text-xl font-bold text-on-surface">Schema Manager</h2>
          <div class="flex gap-2">
            <button class="px-4 py-2 border border-outline-variant rounded-lg text-on-surface text-sm hover:bg-surface-variant/30 transition-colors">New Relationship</button>
            <button class="px-4 py-2 bg-primary text-on-primary rounded-lg font-bold text-sm">Add Node Class</button>
          </div>
        </div>

        <div class="grid grid-cols-3 gap-4">
          {#if $stats?.by_type}
            {#each Object.entries($stats.by_type) as [type, count]}
              <div class="p-4 rounded-lg border border-outline-variant bg-surface-container-low hover:border-primary/50 transition-colors group">
                <div class="flex justify-between items-start mb-3">
                  <div class="w-10 h-10 rounded-lg flex items-center justify-center" style="background: {getColor(type)}15">
                    <span class="material-symbols-outlined" style="color: {getColor(type)}">category</span>
                  </div>
                </div>
                <h3 class="font-bold text-on-surface mb-1 capitalize">Entity: {type}</h3>
                <p class="text-sm text-on-surface-variant mb-3">
                  Importance: {typeDefaults[type] ?? 0.5} | Count: {count}
                </p>
              </div>
            {/each}
          {:else}
            <div class="col-span-3 text-center py-8 text-on-surface-variant">
              Loading schema...
            </div>
          {/if}

          <!-- Add Schema -->
          <div class="p-4 rounded-lg border-2 border-dashed border-outline-variant flex flex-col items-center justify-center text-on-surface-variant hover:border-primary hover:text-primary transition-all cursor-pointer">
            <span class="material-symbols-outlined text-3xl mb-2">add_circle</span>
            <p class="text-sm font-bold">Define Schema</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Active Sources -->
    <div class="col-span-12 lg:col-span-4">
      <div class="glass-panel rounded-xl p-6 h-full">
        <h2 class="text-xl font-bold text-on-surface mb-6">Active Sources</h2>
        <div class="space-y-4">
          <div class="p-4 rounded-lg bg-surface-container-lowest border border-outline-variant">
            <div class="flex items-center gap-3 mb-2">
              <div class="w-8 h-8 rounded bg-surface-variant flex items-center justify-center">
                <span class="material-symbols-outlined text-on-surface">database</span>
              </div>
              <div>
                <h4 class="font-bold text-sm">Memory Store</h4>
                <p class="text-[10px] text-outline">SQLite + FTS5</p>
              </div>
              <span class="ml-auto text-[10px] px-1 py-0.5 bg-secondary/10 text-secondary border border-secondary/30 rounded">ACTIVE</span>
            </div>
            <div class="flex justify-between items-center mt-3 pt-3 border-t border-outline-variant">
              <span class="text-[10px] text-on-surface-variant">{$stats?.total_nodes ?? 0} nodes, {$stats?.total_edges ?? 0} edges</span>
            </div>
          </div>

          <button class="w-full py-3 rounded-lg border-2 border-dashed border-outline-variant flex items-center justify-center gap-2 text-on-surface-variant hover:border-primary hover:text-primary transition-all">
            <span class="material-symbols-outlined">add_link</span>
            <span class="text-sm font-bold">Add Data Connector</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</div>
