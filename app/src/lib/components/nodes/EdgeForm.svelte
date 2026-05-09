<script lang="ts">
  import { graphData, addEdge } from '$lib/stores/graph';

  interface Props {
    onclose: () => void;
    preselectedSource?: string;
  }

  let { onclose, preselectedSource }: Props = $props();

  let source = $state(preselectedSource ?? '');
  let target = $state('');
  let relation = $state('');
  let submitting = $state(false);

  async function handleSubmit() {
    if (!source || !target || !relation.trim()) return;
    submitting = true;
    try {
      await addEdge({ source, target, relation: relation.trim() });
      onclose();
    } catch (e) {
      console.error('Failed to create edge:', e);
    } finally {
      submitting = false;
    }
  }
</script>

<div class="space-y-4">
  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Source Node</label>
    <select bind:value={source} class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm">
      <option value="">Select source...</option>
      {#each $graphData.nodes as n (n.id)}
        <option value={n.id}>{n.title || n.id}</option>
      {/each}
    </select>
  </div>

  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Target Node</label>
    <select bind:value={target} class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm">
      <option value="">Select target...</option>
      {#each $graphData.nodes as n (n.id)}
        <option value={n.id}>{n.title || n.id}</option>
      {/each}
    </select>
  </div>

  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Relationship</label>
    <input bind:value={relation} placeholder="e.g. depends_on, relates_to" class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm" />
  </div>

  <div class="flex gap-2 pt-2">
    <button onclick={onclose} class="flex-1 px-4 py-2 rounded-lg border border-outline-variant text-on-surface text-sm hover:bg-surface-variant/30 transition-colors">Cancel</button>
    <button onclick={handleSubmit} disabled={submitting || !source || !target || !relation.trim()} class="flex-1 px-4 py-2 rounded-lg bg-primary text-on-primary text-sm font-bold hover:opacity-90 transition-colors disabled:opacity-50">
      {submitting ? 'Creating...' : 'Create Edge'}
    </button>
  </div>
</div>
