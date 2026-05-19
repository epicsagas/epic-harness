<script lang="ts">
  interface Props {
    value: string;          // "YYYY-MM-DD" or "YYYY-MM" or ""
    onchange: (v: string) => void;
    showDay?: boolean;      // default true
  }

  let { value, onchange, showDay = true }: Props = $props();

  const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];

  function parse(v: string): { y: string; m: string; d: string } {
    if (!v) return { y: '', m: '', d: '' };
    const parts = v.split('-');
    return { y: parts[0] ?? '', m: parts[1] ?? '', d: parts[2] ?? '' };
  }

  let parts = $derived(parse(value));

  function emit(ny: string, nm: string, nd: string) {
    if (!ny && !nm && !nd) { onchange(''); return; }
    if (showDay) {
      if (ny && nm && nd) onchange(`${ny}-${nm}-${nd}`);
    } else {
      if (ny && nm) onchange(`${ny}-${nm}`);
    }
  }

  const currentYear = new Date().getFullYear();
  const years = Array.from({ length: currentYear - 2023 }, (_, i) => String(2024 + i));

  const daysInMonth = $derived(
    parts.y && parts.m ? new Date(Number(parts.y), Number(parts.m), 0).getDate() : 31
  );

  const SEL = [
    'background:transparent',
    'border:none',
    'color:var(--fg)',
    'font-size:12px',
    'font-family:var(--font-mono)',
    'outline:none',
    'cursor:pointer',
    'padding:2px 4px',
  ].join(';');
</script>

<div style="display:inline-flex;align-items:center;gap:0;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:3px 6px;">
  <select
    value={parts.y}
    onchange={(e) => emit((e.target as HTMLSelectElement).value, parts.m, parts.d)}
    style="{SEL};width:54px;"
  >
    <option value="">YYYY</option>
    {#each years as yr}<option value={yr}>{yr}</option>{/each}
  </select>

  <span style="color:var(--border);font-size:11px;margin:0 1px;">-</span>

  <select
    value={parts.m}
    onchange={(e) => emit(parts.y, (e.target as HTMLSelectElement).value, parts.d)}
    style="{SEL};width:46px;"
  >
    <option value="">MMM</option>
    {#each MONTHS as mon, i}
      <option value={String(i + 1).padStart(2, '0')}>{mon}</option>
    {/each}
  </select>

  {#if showDay}
    <span style="color:var(--border);font-size:11px;margin:0 1px;">-</span>
    <select
      value={parts.d}
      onchange={(e) => emit(parts.y, parts.m, (e.target as HTMLSelectElement).value)}
      style="{SEL};width:38px;"
    >
      <option value="">DD</option>
      {#each Array.from({ length: daysInMonth }, (_, i) => String(i + 1).padStart(2, '0')) as day}
        <option value={day}>{day}</option>
      {/each}
    </select>
  {/if}
</div>
