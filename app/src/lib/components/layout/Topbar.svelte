<script lang="ts">
  import { search, searchResults } from '$lib/stores/graph';
  import { navigate } from '$lib/router';

  let searchQuery = $state('');
  let showResults = $state(false);

  async function handleSearch() {
    if (searchQuery.trim().length < 2) {
      showResults = false;
      return;
    }
    await search(searchQuery);
    showResults = true;
  }

  function selectResult(id: string) {
    showResults = false;
    searchQuery = '';
    navigate(`/entity/${id}`);
  }

  function handleBlur() {
    setTimeout(() => showResults = false, 200);
  }
</script>

<header class="flex justify-between items-center px-6 w-full sticky top-0 z-50 bg-surface/80 backdrop-blur-md h-16 border-b border-outline-variant">
  <div class="flex items-center gap-6">
    <div class="relative">
      <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-on-surface-variant pointer-events-none">search</span>
      <input
        type="text"
        class="bg-surface-container-low border border-outline-variant rounded-full pl-10 pr-12 py-1.5 text-sm focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all w-64 lg:w-96"
        placeholder="Global Entity Search... (Cmd+K)"
        bind:value={searchQuery}
        oninput={handleSearch}
        onfocus={() => { if ($searchResults.length > 0) showResults = true; }}
        onblur={handleBlur}
      />
      <span class="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] font-mono text-outline-variant border border-outline-variant px-1 rounded">Cmd+K</span>

      {#if showResults && $searchResults.length > 0}
        <div class="absolute top-full left-0 right-0 mt-2 glass-panel rounded-xl border border-outline-variant shadow-xl max-h-80 overflow-y-auto z-50">
          {#each $searchResults as result}
            <button
              onclick={() => selectResult(result.id)}
              class="w-full flex items-center gap-3 px-4 py-3 hover:bg-surface-variant/30 transition-colors text-left"
            >
              <span class="material-symbols-outlined text-on-surface-variant text-sm">article</span>
              <div class="flex-1 min-w-0">
                <div class="text-sm font-medium text-on-surface truncate">{result.title}</div>
                <div class="text-[10px] text-on-surface-variant capitalize">{result.type}</div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-4">
    <div class="flex items-center gap-2 mr-6">
      <button class="p-2 rounded-full text-on-surface-variant hover:text-primary transition-colors hover:bg-surface-variant/50">
        <span class="material-symbols-outlined">history</span>
      </button>
      <button class="p-2 rounded-full text-on-surface-variant hover:text-primary transition-colors hover:bg-surface-variant/50 relative">
        <span class="material-symbols-outlined">notifications</span>
        <span class="absolute top-2 right-2 w-2 h-2 bg-error rounded-full"></span>
      </button>
    </div>
    <button class="flex items-center gap-2 px-4 py-1.5 rounded-lg border border-outline-variant text-on-surface hover:bg-surface-variant transition-colors text-sm font-bold">
      <span class="material-symbols-outlined text-sm">visibility</span>
      Focus Mode
    </button>
    <button class="flex items-center gap-2 px-4 py-1.5 rounded-lg bg-primary text-on-primary hover:opacity-90 transition-colors text-sm font-bold">
      <span class="material-symbols-outlined text-sm">download</span>
      Export
    </button>
  </div>
</header>
