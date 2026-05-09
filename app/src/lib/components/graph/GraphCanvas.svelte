<script lang="ts">
  import { onMount } from 'svelte';
  import { renderForceGraph } from '$lib/d3/force-graph';
  import { filteredNodes, filteredEdges, selectNodeById } from '$lib/stores/graph';
  import type { GraphNode } from '$lib/api/types';

  interface Props {
    onNodeClick?: (node: GraphNode) => void;
  }

  let { onNodeClick }: Props = $props();

  let svgEl: SVGSVGElement | undefined = $state();
  let containerEl: HTMLDivElement | undefined = $state();
  let graph: ReturnType<typeof renderForceGraph> | null = null;

  function rerender() {
    if (!svgEl || !containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return;

    graph?.destroy();
    graph = renderForceGraph({
      svgEl,
      width: rect.width,
      height: rect.height,
      nodes: $filteredNodes,
      edges: $filteredEdges,
      onNodeClick: (node) => {
        onNodeClick?.(node);
        selectNodeById(node.id);
      },
    });
  }

  $effect(() => {
    void $filteredNodes;
    void $filteredEdges;
    rerender();
  });

  onMount(() => {
    const observer = new ResizeObserver(() => rerender());
    if (containerEl) observer.observe(containerEl);
    return () => {
      observer.disconnect();
      graph?.destroy();
    };
  });

  // Public API for parent bind:this
  export function zoomIn() { graph?.zoomIn(); }
  export function zoomOut() { graph?.zoomOut(); }
  export function resetZoom() { graph?.resetZoom(); }
</script>

<div bind:this={containerEl} class="absolute inset-0">
  <svg bind:this={svgEl} class="w-full h-full"></svg>
</div>
