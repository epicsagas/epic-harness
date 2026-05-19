<script lang="ts">
  interface Props {
    from: string;   // "YYYY-MM-DD" or ""
    to: string;     // "YYYY-MM-DD" or ""
    onchange: (from: string, to: string) => void;
  }

  let { from, to, onchange }: Props = $props();

  let open = $state(false);
  let hovered = $state('');

  // calendar view state
  const today = new Date();
  let viewYear  = $state(today.getFullYear());
  let viewMonth = $state(today.getMonth()); // 0-based

  // picking phase: 0 = no selection, 1 = from picked (waiting for to)
  let phase = $state(0);
  let pendingFrom = $state('');

  const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
  const DAYS   = ['Su','Mo','Tu','We','Th','Fr','Sa'];

  function toYMD(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
  }

  function parseYMD(s: string): Date | null {
    if (!s) return null;
    const [y, m, d] = s.split('-').map(Number);
    return new Date(y, m - 1, d);
  }

  function calDays(year: number, month: number): (string | null)[] {
    const first = new Date(year, month, 1).getDay();
    const last  = new Date(year, month + 1, 0).getDate();
    const cells: (string | null)[] = Array(first).fill(null);
    for (let d = 1; d <= last; d++) {
      cells.push(toYMD(new Date(year, month, d)));
    }
    return cells;
  }

  function prevMonth() {
    if (viewMonth === 0) { viewMonth = 11; viewYear--; }
    else viewMonth--;
  }
  function nextMonth() {
    if (viewMonth === 11) { viewMonth = 0; viewYear++; }
    else viewMonth++;
  }

  function clickDay(day: string) {
    if (phase === 0) {
      pendingFrom = day;
      phase = 1;
    } else {
      let f = pendingFrom;
      let t = day;
      if (f > t) { [f, t] = [t, f]; }
      phase = 0;
      pendingFrom = '';
      open = false;
      onchange(f, t);
    }
  }

  function inRange(day: string): boolean {
    const f = phase === 1 ? pendingFrom : from;
    const t = phase === 1 ? (hovered || '') : to;
    if (!f || !t) return false;
    const lo = f < t ? f : t;
    const hi = f < t ? t : f;
    return day > lo && day < hi;
  }

  function isEdge(day: string): 'from' | 'to' | null {
    const f = phase === 1 ? pendingFrom : from;
    const t = phase === 1 ? (hovered || '') : to;
    if (!f && !t) return null;
    const lo = f < t ? f : t;
    const hi = f < t ? t : f;
    if (day === lo) return 'from';
    if (day === hi && lo !== hi) return 'to';
    return null;
  }

  function label(): string {
    if (!from && !to) return '날짜 범위 선택';
    if (from && !to) return `${from} ~`;
    if (!from && to) return `~ ${to}`;
    return `${from}  ~  ${to}`;
  }

  function clear(e: MouseEvent) {
    e.stopPropagation();
    phase = 0;
    pendingFrom = '';
    open = false;
    onchange('', '');
  }

  function toggleOpen() {
    open = !open;
    if (open) {
      // init view to the 'from' date or today
      const ref = parseYMD(from) ?? today;
      viewYear  = ref.getFullYear();
      viewMonth = ref.getMonth();
      phase = 0;
      pendingFrom = '';
    }
  }

  const cells = $derived(calDays(viewYear, viewMonth));
</script>

<div style="position:relative;display:inline-block;">
  <!-- trigger button -->
  <button
    onclick={toggleOpen}
    style="display:inline-flex;align-items:center;gap:6px;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:{from||to?'var(--fg)':'var(--muted)'};font-size:12px;font-family:var(--font-mono);cursor:pointer;white-space:nowrap;"
  >
    <span>📅</span>
    <span>{label()}</span>
    {#if from || to}
      <span
        role="button"
        tabindex="0"
        onclick={clear}
        onkeydown={(e) => e.key === 'Enter' && clear(e as unknown as MouseEvent)}
        style="color:var(--muted);font-size:11px;line-height:1;cursor:pointer;"
      >✕</span>
    {/if}
  </button>

  {#if open}
    <!-- backdrop -->
    <div
      role="presentation"
      onclick={() => { open = false; phase = 0; pendingFrom = ''; }}
      style="position:fixed;inset:0;z-index:999;"
    ></div>

    <!-- calendar popup -->
    <div style="position:absolute;top:calc(100% + 4px);left:0;z-index:1000;background:var(--surface,#161b22);border:1px solid var(--border);border-radius:var(--radius);padding:12px;min-width:260px;box-shadow:0 8px 24px rgba(0,0,0,.4);">

      <!-- hint -->
      <div style="font-size:11px;color:var(--muted);text-align:center;margin-bottom:8px;font-family:var(--font-mono);">
        {phase === 0 ? '시작일 클릭' : '종료일 클릭'}
      </div>

      <!-- month nav -->
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;">
        <button onclick={prevMonth} style="background:none;border:none;color:var(--fg);font-size:16px;cursor:pointer;padding:2px 6px;">‹</button>
        <span style="font-size:13px;font-weight:600;color:var(--fg);font-family:var(--font-mono);">
          {viewYear} / {MONTHS[viewMonth]}
        </span>
        <button onclick={nextMonth} style="background:none;border:none;color:var(--fg);font-size:16px;cursor:pointer;padding:2px 6px;">›</button>
      </div>

      <!-- weekday header -->
      <div style="display:grid;grid-template-columns:repeat(7,1fr);gap:2px;margin-bottom:4px;">
        {#each DAYS as d}
          <div style="text-align:center;font-size:10px;color:var(--muted);font-family:var(--font-mono);padding:2px 0;">{d}</div>
        {/each}
      </div>

      <!-- day cells -->
      <div style="display:grid;grid-template-columns:repeat(7,1fr);gap:2px;">
        {#each cells as cell}
          {#if cell === null}
            <div></div>
          {:else}
            {@const edge = isEdge(cell)}
            {@const ranged = inRange(cell)}
            {@const dayNum = Number(cell.slice(8))}
            <button
              onclick={() => clickDay(cell)}
              onmouseenter={() => { if (phase === 1) hovered = cell; }}
              onmouseleave={() => { if (phase === 1) hovered = ''; }}
              style="
                padding:4px 2px;border:none;border-radius:4px;cursor:pointer;font-size:12px;font-family:var(--font-mono);text-align:center;
                background:{edge ? 'var(--accent)' : ranged ? 'var(--accent-soft,rgba(88,166,255,.15))' : 'transparent'};
                color:{edge ? '#fff' : ranged ? 'var(--accent)' : 'var(--fg)'};
                font-weight:{edge ? '700' : '400'};
              "
            >{dayNum}</button>
          {/if}
        {/each}
      </div>

      <!-- footer: phase indicator + clear -->
      {#if phase === 1}
        <div style="margin-top:10px;font-size:11px;color:var(--accent);font-family:var(--font-mono);text-align:center;">
          시작: {pendingFrom}
        </div>
      {/if}
    </div>
  {/if}
</div>
