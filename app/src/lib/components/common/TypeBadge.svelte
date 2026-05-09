<script lang="ts">
  interface Props {
    type: string;
    size?: 'sm' | 'md';
  }

  let { type, size = 'md' }: Props = $props();

  const colors: Record<string, string> = {
    decision: 'bg-purple-500/20 text-purple-300 border-purple-500/30',
    resolution: 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30',
    concept: 'bg-pink-500/20 text-pink-300 border-pink-500/30',
    project: 'bg-blue-500/20 text-blue-300 border-blue-500/30',
    error: 'bg-red-500/20 text-red-300 border-red-500/30',
    session: 'bg-gray-500/20 text-gray-300 border-gray-500/30',
    pattern: 'bg-cyan-500/20 text-cyan-300 border-cyan-500/30',
    instinct: 'bg-violet-500/20 text-violet-300 border-violet-500/30',
    psychographic: 'bg-amber-500/20 text-amber-300 border-amber-500/30',
  };
</script>

<span
  class="inline-flex items-center rounded-full border font-medium capitalize {colors[type] ?? 'bg-gray-500/20 text-gray-300 border-gray-500/30'} {size === 'sm' ? 'px-2 py-0.5 text-[10px]' : 'px-3 py-1 text-xs'}"
>
  {type}
</span>
