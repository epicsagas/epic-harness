<script lang="ts">
  import { currentRoute } from '$lib/router';
  import { selectNodeById, selectedNode, graphData } from '$lib/stores/graph';
  import TypeBadge from '$lib/components/common/TypeBadge.svelte';
  import { getColor } from '$lib/d3/force-graph';

  $effect(() => {
    const id = $currentRoute.params?.id;
    if (id) selectNodeById(id);
  });

  let neighbors = $derived.by(() => {
    if (!$selectedNode) return [];
    const id = $selectedNode.id;
    return $graphData.edges
      .filter((e) => e.source === id || e.target === id)
      .map((e) => ({
        id: e.source === id ? e.target : e.source,
        relation: e.relation,
      }));
  });

  let neighborNodes = $derived.by(() => {
    const ids = new Set(neighbors.map((n) => n.id));
    return $graphData.nodes.filter((n) => ids.has(n.id));
  });
</script>

<div class="flex-1 overflow-y-auto p-6 space-y-6">
  {#if $selectedNode}
    <!-- Entity Header -->
    <section class="flex flex-col md:flex-row md:items-end justify-between gap-4 border-b border-outline-variant pb-6">
      <div class="space-y-2">
        <div class="flex items-center gap-2 text-secondary">
          <span class="material-symbols-outlined text-sm">corporate_fare</span>
          <span class="uppercase tracking-widest text-xs">Node Detail</span>
        </div>
        <h1 class="text-4xl font-bold tracking-tight">{$selectedNode.title}</h1>
        <div class="flex items-center gap-3 mt-2">
          <TypeBadge type={$selectedNode.type} />
          <span class="text-sm text-on-surface-variant">Importance: {$selectedNode.importance.toFixed(2)}</span>
          <span class="text-sm text-on-surface-variant">Access: {$selectedNode.access_count}</span>
        </div>
      </div>
    </section>

    <!-- Content Grid -->
    <div class="grid grid-cols-12 gap-6">
      <!-- Properties -->
      <div class="col-span-12 lg:col-span-4 space-y-6">
        <div class="glass-panel p-6 rounded-xl">
          <h3 class="font-bold text-on-surface flex items-center gap-2 mb-4">
            <span class="material-symbols-outlined text-primary">list_alt</span>
            Properties
          </h3>
          <div class="space-y-3">
            <div class="flex justify-between py-2 border-b border-outline-variant/30">
              <span class="text-sm text-on-surface-variant">Type</span>
              <span class="text-sm font-medium capitalize">{$selectedNode.type}</span>
            </div>
            <div class="flex justify-between py-2 border-b border-outline-variant/30">
              <span class="text-sm text-on-surface-variant">Importance</span>
              <span class="text-sm font-mono">{$selectedNode.importance.toFixed(3)}</span>
            </div>
            <div class="flex justify-between py-2 border-b border-outline-variant/30">
              <span class="text-sm text-on-surface-variant">Created</span>
              <span class="text-sm">{new Date($selectedNode.created).toLocaleDateString()}</span>
            </div>
            <div class="flex justify-between py-2">
              <span class="text-sm text-on-surface-variant">Updated</span>
              <span class="text-sm">{new Date($selectedNode.updated).toLocaleDateString()}</span>
            </div>
          </div>

          {#if $selectedNode.tags.length > 0}
            <div class="mt-4">
              <h4 class="text-xs font-bold uppercase text-on-surface-variant mb-2">Tags</h4>
              <div class="flex flex-wrap gap-1.5">
                {#each $selectedNode.tags as tag}
                  <span class="px-2 py-0.5 bg-surface-container-high rounded-full text-[10px] text-on-surface-variant border border-outline-variant">{tag}</span>
                {/each}
              </div>
            </div>
          {/if}

          {#if $selectedNode.projects.length > 0}
            <div class="mt-4">
              <h4 class="text-xs font-bold uppercase text-on-surface-variant mb-2">Projects</h4>
              <div class="flex flex-wrap gap-1.5">
                {#each $selectedNode.projects as proj}
                  <span class="px-2 py-0.5 bg-primary/10 rounded-full text-[10px] text-primary border border-primary/20">{proj}</span>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>

      <!-- Body + Connections -->
      <div class="col-span-12 lg:col-span-8 space-y-6">
        <div class="glass-panel p-6 rounded-xl">
          <h3 class="font-bold text-on-surface flex items-center gap-2 mb-4">
            <span class="material-symbols-outlined text-secondary text-sm">description</span>
            Body
          </h3>
          <pre class="text-sm text-on-surface-variant whitespace-pre-wrap bg-surface-container-lowest p-4 rounded-lg border border-outline-variant max-h-80 overflow-y-auto font-mono">{$selectedNode.body || '(empty)'}</pre>
        </div>

        <div class="glass-panel p-6 rounded-xl">
          <h3 class="font-bold text-on-surface flex items-center gap-2 mb-4">
            <span class="material-symbols-outlined text-tertiary text-sm">share</span>
            Connections ({neighbors.length})
          </h3>
          {#if neighborNodes.length > 0}
            <div class="space-y-2">
              {#each neighborNodes as n}
                <div class="flex items-center justify-between p-3 bg-surface-container rounded-lg">
                  <div class="flex items-center gap-3">
                    <span class="w-3 h-3 rounded-full shrink-0" style="background: {getColor(n.type)}"></span>
                    <span class="text-sm font-medium">{n.title || n.id}</span>
                    <span class="text-xs text-on-surface-variant capitalize">{n.type}</span>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="text-on-surface-variant text-sm text-center py-4">No connections</div>
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <section class="flex flex-col md:flex-row md:items-end justify-between gap-4 border-b border-outline-variant pb-6">
      <div class="space-y-2">
        <div class="flex items-center gap-2 text-secondary">
          <span class="material-symbols-outlined text-sm">corporate_fare</span>
          <span class="uppercase tracking-widest text-xs">Node Detail</span>
        </div>
        <h1 class="text-4xl font-bold tracking-tight">Entity Intelligence</h1>
        <p class="text-on-surface-variant max-w-2xl">
          Select a node from the Explorer or search to view detailed entity information, connections, and lineage.
        </p>
      </div>
    </section>

    <div class="grid grid-cols-12 gap-6">
      <div class="col-span-12 lg:col-span-4 space-y-6">
        <div class="glass-panel p-6 rounded-xl">
          <h3 class="font-bold text-on-surface flex items-center gap-2 mb-4">
            <span class="material-symbols-outlined text-primary">list_alt</span>
            Properties
          </h3>
          <div class="text-on-surface-variant text-sm">No node selected</div>
        </div>
      </div>
      <div class="col-span-12 lg:col-span-8 space-y-6">
        <div class="glass-panel rounded-xl overflow-hidden h-[400px] flex items-center justify-center">
          <div class="text-on-surface-variant text-sm">Select a node to view its graph</div>
        </div>
      </div>
    </div>
  {/if}
</div>
