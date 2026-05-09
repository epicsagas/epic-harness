<script lang="ts">
  import { graphData, typeFilters } from '$lib/stores/graph';
  import { getColor } from '$lib/d3/force-graph';

  function toggle(type: string) {
    typeFilters.update((f) => ({ ...f, [type]: !f[type] }));
  }

  const typeCounts = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const n of $graphData.nodes) {
      counts[n.type] = (counts[n.type] ?? 0) + 1;
    }
    return counts;
  });
</script>

<div class="space-y-2">
  <h3 class="text-[11px] font-bold uppercase tracking-wider text-on-surface-variant">Filter by Type</h3>
  {#each Object.entries(typeCounts) as [type, count]}
    <label class="flex items-center gap-3 cursor-pointer group">
      <input
        type="checkbox"
        checked={$typeFilters[type] ?? false}
        onchange={() => toggle(type)}
        class="w-3.5 h-3.5"
        style="accent-color: {getColor(type)}"
      />
      <span class="w-2.5 h-2.5 rounded-full shrink-0" style="background: {getColor(type)}"></span>
      <span class="text-sm text-on-surface-variant group-hover:text-on-surface capitalize flex-1">{type}</span>
      <span class="text-xs text-outline font-mono">{count}</span>
    </label>
  {/each}
</div>
