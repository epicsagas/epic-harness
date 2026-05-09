<script lang="ts">
  import { selectedNode, clearSelection, removeNode, updateNode } from '$lib/stores/graph';
  import TypeBadge from '$lib/components/common/TypeBadge.svelte';

  let editing = $state(false);
  let saving = $state(false);
  let editTitle = $state('');
  let editBody = $state('');
  let confirmDelete = $state(false);

  function startEdit() {
    if (!$selectedNode) return;
    editTitle = $selectedNode.title;
    editBody = $selectedNode.body;
    editing = true;
  }

  function cancelEdit() {
    editing = false;
  }

  async function saveEdit() {
    if (!$selectedNode || saving) return;
    saving = true;
    try {
      await updateNode($selectedNode.id, {
        title: editTitle,
        body: editBody,
      });
      editing = false;
    } catch (e) {
      console.error('Failed to save node:', e);
    } finally {
      saving = false;
    }
  }

  function requestDelete() {
    confirmDelete = true;
  }

  function cancelDelete() {
    confirmDelete = false;
  }

  async function handleDelete() {
    if (!$selectedNode) return;
    try {
      await removeNode($selectedNode.id);
      confirmDelete = false;
    } catch (e) {
      console.error('Failed to delete node:', e);
      confirmDelete = false;
    }
  }
</script>

{#if $selectedNode}
  <div class="absolute right-0 top-0 bottom-0 w-96 glass-panel border-l border-outline-variant z-20 flex flex-col overflow-y-auto shadow-xl">
    <div class="p-4 border-b border-outline-variant flex justify-between items-center">
      <h3 class="font-bold text-on-surface text-sm">Node Detail</h3>
      <button onclick={clearSelection} class="p-1.5 rounded-lg text-on-surface-variant hover:bg-surface-variant/50">
        <span class="material-symbols-outlined text-lg">close</span>
      </button>
    </div>

    <div class="p-4 space-y-4 flex-1">
      <div>
        <div class="flex items-center gap-2 mb-2">
          <TypeBadge type={$selectedNode.type} />
          <span class="text-[10px] text-on-surface-variant">{$selectedNode.importance.toFixed(2)}</span>
        </div>
        {#if editing}
          <input bind:value={editTitle} class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm mb-2" />
        {:else}
          <h2 class="text-lg font-bold text-on-surface">{$selectedNode.title}</h2>
        {/if}
      </div>

      {#if $selectedNode.tags.length > 0}
        <div class="flex flex-wrap gap-1.5">
          {#each $selectedNode.tags as tag (tag)}
            <span class="px-2 py-0.5 bg-surface-container-high rounded-full text-[10px] text-on-surface-variant border border-outline-variant">{tag}</span>
          {/each}
        </div>
      {/if}

      <div>
        <h4 class="text-xs font-bold uppercase text-on-surface-variant mb-1">Body</h4>
        {#if editing}
          <textarea bind:value={editBody} rows={10} class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm font-mono"></textarea>
        {:else}
          <pre class="text-sm text-on-surface-variant whitespace-pre-wrap bg-surface-container-lowest p-3 rounded-lg border border-outline-variant max-h-60 overflow-y-auto">{$selectedNode.body}</pre>
        {/if}
      </div>

      <div class="text-[10px] text-outline">
        Created: {new Date($selectedNode.created).toLocaleString()}
      </div>
    </div>

    {#if confirmDelete}
      <div class="p-4 border-t border-error/30 bg-error/5">
        <p class="text-sm text-error mb-3">Delete this node and all connected edges?</p>
        <div class="flex gap-2">
          <button onclick={handleDelete} class="flex-1 px-3 py-2 text-sm rounded-lg bg-error text-on-error font-bold hover:opacity-90 transition-colors">Delete</button>
          <button onclick={cancelDelete} class="flex-1 px-3 py-2 text-sm rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant/30 transition-colors">Cancel</button>
        </div>
      </div>
    {:else}
      <div class="p-4 border-t border-outline-variant flex gap-2">
        {#if editing}
          <button onclick={saveEdit} disabled={saving} class="flex-1 px-3 py-2 text-sm rounded-lg border border-primary/50 text-primary hover:bg-primary/10 transition-colors disabled:opacity-50">
            <span class="material-symbols-outlined text-sm align-middle mr-1">save</span>{saving ? 'Saving...' : 'Save'}
          </button>
          <button onclick={cancelEdit} class="px-3 py-2 text-sm rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant/30 transition-colors">
            <span class="material-symbols-outlined text-sm align-middle mr-1">close</span>Cancel
          </button>
        {:else}
          <button onclick={startEdit} class="flex-1 px-3 py-2 text-sm rounded-lg border border-outline-variant text-on-surface hover:bg-surface-variant/30 transition-colors">
            <span class="material-symbols-outlined text-sm align-middle mr-1">edit</span>Edit
          </button>
        {/if}
        <button onclick={requestDelete} class="px-3 py-2 text-sm rounded-lg border border-error/30 text-error hover:bg-error/10 transition-colors">
          <span class="material-symbols-outlined text-sm align-middle">delete</span>
        </button>
      </div>
    {/if}
  </div>
{/if}
