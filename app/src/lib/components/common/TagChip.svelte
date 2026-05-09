<script lang="ts">
  interface Props {
    tag: string;
    selected?: boolean;
    onclick?: () => void;
  }

  let { tag, selected = false, onclick }: Props = $props();
</script>

<button
  {onclick}
  class="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium transition-colors border
    {selected
      ? 'bg-primary/20 text-primary border-primary/30'
      : 'bg-surface-container-high text-on-surface-variant border-outline-variant hover:bg-surface-variant'}"
>
  {tag}
</button>
