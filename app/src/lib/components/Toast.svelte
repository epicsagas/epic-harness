<script lang="ts">
  interface Toast {
    id: number;
    message: string;
    type: 'success' | 'error' | 'info';
  }

  let toasts = $state<Toast[]>([]);
  let counter = 0;

  export function showToast(message: string, type: Toast['type'] = 'success') {
    const id = ++counter;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => {
      toasts = toasts.filter(t => t.id !== id);
    }, 2500);
  }
</script>

<div class="toast-container">
  {#each toasts as toast (toast.id)}
    <div class="toast toast-{toast.type}">
      {#if toast.type === 'success'}✓{:else if toast.type === 'error'}✗{:else}ℹ{/if}
      {toast.message}
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 9999;
    pointer-events: none;
  }
  .toast {
    padding: 0.6rem 1rem;
    border-radius: 6px;
    font-size: 0.85rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    animation: slide-in 0.2s ease;
    pointer-events: auto;
  }
  .toast-success { background: #166534; color: #86efac; border: 1px solid #166534; }
  .toast-error   { background: #7f1d1d; color: #fca5a5; border: 1px solid #7f1d1d; }
  .toast-info    { background: #1e3a5f; color: #93c5fd; border: 1px solid #1e3a5f; }
  @keyframes slide-in {
    from { opacity: 0; transform: translateX(1rem); }
    to   { opacity: 1; transform: translateX(0); }
  }
</style>
