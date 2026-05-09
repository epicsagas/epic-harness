<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    title: string;
    open: boolean;
    onclose: () => void;
    children?: import('svelte').Snippet;
  }

  let { title, open, onclose, children }: Props = $props();
  let dialogRef: HTMLDivElement | undefined = $state();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) onclose();
  }

  $effect(() => {
    if (open) {
      // Focus the dialog container
      setTimeout(() => dialogRef?.focus(), 0);
    }
  });

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if open}
  <div class="fixed inset-0 z-[100] flex items-center justify-center" role="dialog" aria-modal="true" aria-labelledby="modal-title">
    <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" onclick={onclose}></div>
    <div bind:this={dialogRef} tabindex="-1" class="relative glass-panel rounded-2xl p-6 w-full max-w-lg mx-4 shadow-2xl border border-outline-variant outline-none">
      <div class="flex justify-between items-center mb-4">
        <h2 id="modal-title" class="text-lg font-bold text-on-surface">{title}</h2>
        <button onclick={onclose} class="p-1.5 rounded-lg text-on-surface-variant hover:bg-surface-variant/50 transition-colors" aria-label="Close dialog">
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      {@render children?.()}
    </div>
  </div>
{/if}
