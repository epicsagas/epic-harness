<script lang="ts">
  import { onMount } from 'svelte';
  import { loadGraph, loadStats, selectedNode } from '$lib/stores/graph';
  import GraphCanvas from '$lib/components/graph/GraphCanvas.svelte';
  import SelectionDetail from '$lib/components/graph/SelectionDetail.svelte';
  import TypeFilter from '$lib/components/filters/TypeFilter.svelte';
  import Modal from '$lib/components/common/Modal.svelte';
  import NodeForm from '$lib/components/nodes/NodeForm.svelte';
  import EdgeForm from '$lib/components/nodes/EdgeForm.svelte';
  import { getColor } from '$lib/d3/force-graph';

  let showAddNode = $state(false);
  let showAddEdge = $state(false);
  let graphCanvas: GraphCanvas | undefined = $state();

  onMount(() => {
    loadGraph();
    loadStats();
  });
</script>

<div class="flex-1 relative flex overflow-hidden">
  <!-- Left Filter Rail -->
  <div class="w-56 shrink-0 border-r border-outline-variant bg-surface-container-lowest p-4 overflow-y-auto">
    <TypeFilter />
  </div>

  <!-- Graph Canvas -->
  <div class="flex-1 relative bg-surface-container-lowest">
    <GraphCanvas bind:this={graphCanvas} />

    <!-- Floating Controls -->
    <div class="absolute left-6 bottom-6 flex flex-col gap-2 z-30">
      <div class="glass-panel p-1 rounded-xl flex flex-col gap-1">
        <button onclick={() => graphCanvas?.zoomIn()} class="p-2 text-on-surface hover:bg-primary/20 rounded-lg transition-colors">
          <span class="material-symbols-outlined">add</span>
        </button>
        <button onclick={() => graphCanvas?.zoomOut()} class="p-2 text-on-surface hover:bg-primary/20 rounded-lg transition-colors">
          <span class="material-symbols-outlined">remove</span>
        </button>
        <button onclick={() => graphCanvas?.resetZoom()} class="p-2 text-on-surface hover:bg-primary/20 rounded-lg transition-colors">
          <span class="material-symbols-outlined">center_focus_strong</span>
        </button>
      </div>
    </div>

    <!-- Legend Overlay -->
    <div class="absolute right-6 top-6 z-30">
      <div class="glass-panel p-4 rounded-xl w-48">
        <h3 class="text-[11px] font-bold uppercase tracking-wider text-on-surface-variant mb-3">Graph Legend</h3>
        <div class="space-y-2">
          {#each [
            ['Concept', 'concept'],
            ['Pattern', 'pattern'],
            ['Decision', 'decision'],
            ['Project', 'project'],
            ['Resolution', 'resolution'],
            ['Error', 'error'],
          ] as [label, type] (type)}
            <div class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full" style="background: {getColor(type)}"></span>
              <span class="text-sm">{label}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <!-- Floating Add Buttons -->
    <div class="absolute left-6 top-6 z-30 flex gap-2">
      <button onclick={() => showAddNode = true} class="glass-panel px-3 py-2 rounded-lg text-sm text-on-surface hover:bg-primary/20 transition-colors flex items-center gap-1.5">
        <span class="material-symbols-outlined text-sm">add_circle</span>
        Add Node
      </button>
      <button onclick={() => showAddEdge = true} class="glass-panel px-3 py-2 rounded-lg text-sm text-on-surface hover:bg-primary/20 transition-colors flex items-center gap-1.5">
        <span class="material-symbols-outlined text-sm">add_link</span>
        Add Edge
      </button>
    </div>
  </div>

  <!-- Selection Detail Panel -->
  <SelectionDetail />
</div>

<Modal title="Add Node" open={showAddNode} onclose={() => showAddNode = false}>
  {#snippet children()}
    <NodeForm onclose={() => showAddNode = false} />
  {/snippet}
</Modal>

<Modal title="Add Edge" open={showAddEdge} onclose={() => showAddEdge = false}>
  {#snippet children()}
    <EdgeForm onclose={() => showAddEdge = false} />
  {/snippet}
</Modal>
