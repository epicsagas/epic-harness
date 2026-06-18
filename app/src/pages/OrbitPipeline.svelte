<script lang="ts">
  import { onMount } from 'svelte';
  import { getOrbitPipelines, dismissOrbitPipeline } from '../lib/harness.js';
  import type { OrbitPipeline } from '../lib/harness.js';
  import DateRangePicker from '$lib/components/DateRangePicker.svelte';
  import { tStore, t } from '$lib/i18n.js';

  let pipelines = $state<OrbitPipeline[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // ── Update-flash: glow a section's border when its data changes.
  let flash = $state<Record<string, boolean>>({});
  const flashPrev: Record<string, string> = {};
  function canon(value: unknown): string {
    if (value === null || typeof value !== 'object') return JSON.stringify(value);
    if (Array.isArray(value)) return '[' + value.map(canon).join(',') + ']';
    const obj = value as Record<string, unknown>;
    return '{' + Object.keys(obj).sort().map(k => JSON.stringify(k)+':'+canon(obj[k])).join(',') + '}';
  }
  function flashIfChanged(key: string, payload: unknown): void {
    const sig = canon(payload);
    if (flashPrev[key] !== undefined && flashPrev[key] !== sig) {
      flash[key] = false;
      queueMicrotask(() => { flash[key] = true; });
      window.setTimeout(() => { flash[key] = false; }, 1700);
    }
    flashPrev[key] = sig;
  }

  // ── filters
  let filterGoal = $state('');
  let filterProject = $state('');
  let filterStatus = $state('');
  let filterDateFrom = $state('');
  let filterDateTo = $state('');

  // ── pagination
  const PAGE_SIZE = 15;
  let page = $state(1);

  const runningPipelines = $derived(pipelines.filter(p => p.status === 'running'));

  const filtered = $derived(() => {
    let list = pipelines.filter(p => p.status !== 'running');
    if (filterGoal.trim())
      list = list.filter(p => (p.goal_slug ?? '').toLowerCase().includes(filterGoal.trim().toLowerCase()));
    if (filterProject.trim())
      list = list.filter(p => (p._project ?? '').toLowerCase().includes(filterProject.trim().toLowerCase()));
    if (filterStatus)
      list = list.filter(p => p.status === filterStatus);
    if (filterDateFrom)
      list = list.filter(p => p.started_at >= filterDateFrom);
    if (filterDateTo)
      list = list.filter(p => p.started_at.slice(0, 10) <= filterDateTo);
    return list;
  });

  const totalPages = $derived(Math.max(1, Math.ceil(filtered().length / PAGE_SIZE)));
  const pageItems = $derived(filtered().slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE));

  // reset page when filters change
  $effect(() => {
    filterGoal; filterProject; filterStatus; filterDateFrom; filterDateTo;
    page = 1;
  });

  const PHASES = ['spec', 'go', 'check', 'ship', 'complete'] as const;
  type Phase = typeof PHASES[number];

  async function load() {
    try {
      error = null;
      pipelines = await getOrbitPipelines();
      flashIfChanged('pipelines', pipelines);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    const id = setInterval(load, 30000);
    return () => clearInterval(id);
  });

  function clearFilters() {
    filterGoal = '';
    filterProject = '';
    filterStatus = '';
    filterDateFrom = '';
    filterDateTo = '';
  }

  const hasFilter = $derived(
    filterGoal || filterProject || filterStatus || filterDateFrom || filterDateTo
  );

  function statusBadgeClass(status: OrbitPipeline['status']): string {
    const map: Record<string, string> = {
      running: 'info', complete: 'success', failed: 'danger',
      paused: 'warning', aborted: 'muted', timeout: 'danger',
    };
    return map[status] ?? 'muted';
  }

  function phaseIndex(phase: string): number {
    return PHASES.indexOf(phase as Phase);
  }

  function durationMinutes(startedAt: string, updatedAt: string): number {
    return Math.round((new Date(updatedAt).getTime() - new Date(startedAt).getTime()) / 60000);
  }

  function fmtDatetime(iso: string): string {
    try {
      return new Date(iso).toLocaleString('ko-KR', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
    } catch { return iso; }
  }

  function shortId(id: string): string { return id.slice(0, 8); }

  function deadlineRemaining(deadline: string | null): string {
    if (!deadline) return '--';
    const diff = Math.round((new Date(deadline).getTime() - Date.now()) / 60000);
    return diff <= 0 ? t('expired') : t('minutesLeft', diff);
  }

  function phaseStageClass(phase: string, currentPhase: string, phaseHistory: OrbitPipeline['phase_history']): string {
    const done = phaseHistory.map(h => h.phase);
    if (done.includes(phase)) return 'completed';
    if (phase === currentPhase) return 'running';
    return 'pending';
  }

  function phaseIcon(phase: string): string {
    return ({ spec: '☰', go: '▶', check: '✓', ship: '↑', complete: '↻' } as Record<string,string>)[phase] ?? phase;
  }

  function phaseColor(phase: string): string {
    return ({ spec: 'var(--accent)', go: 'var(--accent)', check: 'var(--success)', ship: 'var(--purple)', complete: 'var(--teal)' } as Record<string,string>)[phase] ?? 'var(--muted)';
  }

  function projectLabel(raw: string | undefined): string {
    return raw?.replace(/-[a-f0-9]{6}$/, '') ?? '--';
  }

  async function dismiss(id: string) {
    try {
      await dismissOrbitPipeline(id);
      pipelines = pipelines.filter(p => p.id !== id);
    } catch { /* ignore */ }
  }
</script>

<div class="screen-header">
  <h2>/orbit <span class="subtitle-tag">Autonomous Pipeline</span></h2>
  <p>spec → go → check → ship → evolve — single-command spec-to-PR execution</p>
</div>

{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body"><span style="color:var(--danger)">{$tStore('loadError')}: {error}</span></div>
  </div>
{/if}

<!-- Entry Mode Selection -->
<div class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>Entry Mode</h3></div>
  <div class="panel-body">
    <div class="grid-3">
      <div class="cmd-card" style="border-left:3px solid var(--accent);">
        <div class="cmd-name" style="font-size:13px;">Interactive</div>
        <div class="cmd-desc">{$tStore('interactiveDesc')}</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill info">unclear</span></div>
      </div>
      <div class="cmd-card" style="border-left:3px solid var(--purple);">
        <div class="cmd-name" style="font-size:13px;">Council</div>
        <div class="cmd-desc">{$tStore('councilDesc')}</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill purple">complex</span></div>
      </div>
      <div class="cmd-card" style="border-left:3px solid var(--teal);">
        <div class="cmd-name" style="font-size:13px;">Direct</div>
        <div class="cmd-desc">{$tStore('directDesc')}</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill teal">clear</span></div>
      </div>
    </div>
  </div>
</div>

<!-- Running Pipelines -->
{#if loading}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('runningPipelines')}</h3></div>
    <div class="panel-body" style="color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
  </div>
{:else if runningPipelines.length > 0}
  {#each runningPipelines as p}
    <div class="panel" style="margin-bottom:16px;border-left:3px solid var(--accent);" class:hx-flash={flash.pipelines}>
      <div class="panel-header">
        <h3><span class="pill info" style="margin-right:8px;">RUNNING</span>{p.goal_slug ?? $tStore('noGoal')}</h3>
        <div class="panel-actions" style="display:flex;align-items:center;gap:8px;">
          <span style="font-size:11px;color:var(--muted);font-family:var(--font-mono);">
            ID: {shortId(p.id)} · Mode: {p.mode ?? '--'} · Check fails: {p.check_fail_count}
          </span>
          <button
            onclick={() => dismiss(p.id)}
            title="Dismiss"
            style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);width:22px;height:22px;display:flex;align-items:center;justify-content:center;color:var(--muted);font-size:13px;cursor:pointer;padding:0;line-height:1;"
          >✕</button>
        </div>
      </div>
      <div class="panel-body">
        <div class="pipeline-flow">
          {#each PHASES as phase, i}
            {#if i > 0}<div class="pipeline-arrow {phaseIndex(p.phase) > i - 1 ? 'done' : ''}"></div>{/if}
            {@const sc = phaseStageClass(phase, p.phase, p.phase_history)}
            <div class="pipeline-stage {sc}">
              <div class="stage-icon" style="{sc === 'completed' || sc === 'running' ? `background:${phaseColor(phase)}22;border-color:${phaseColor(phase)};color:${phaseColor(phase)};` : ''}">{phaseIcon(phase)}</div>
              <div class="stage-label">{phase}</div>
            </div>
          {/each}
        </div>
        <div style="margin-top:12px;font-size:11px;color:var(--muted);font-family:var(--font-mono);display:flex;gap:24px;flex-wrap:wrap;">
          <span>{$tStore('startedAt')}: {fmtDatetime(p.started_at)}</span>
          {#if p.deadline}<span>{$tStore('deadlineAt')}: {fmtDatetime(p.deadline)} ({deadlineRemaining(p.deadline)})</span>{/if}
          <span>{$tStore('checkFails')}: {p.check_fail_count}/3</span>
        </div>
      </div>
    </div>
  {/each}
{/if}

<!-- Pipeline History -->
<div class="panel">
  <div class="panel-header">
    <h3>Pipeline History
      {#if !loading}
        <span style="font-size:11px;font-weight:400;color:var(--muted);margin-left:8px;">
          {$tStore('filterCountOrbit', filtered().length, pipelines.filter(p=>p.status!=='running').length)}
        </span>
      {/if}
    </h3>
  </div>

  <!-- Filter bar -->
  <div style="padding:12px 16px;border-bottom:1px solid var(--border);display:flex;gap:8px;flex-wrap:wrap;align-items:center;">
    <input
      type="text"
      placeholder={$tStore('searchGoal')}
      bind:value={filterGoal}
      style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:5px 10px;color:var(--fg);font-size:12px;width:140px;outline:none;"
    />
    <input
      type="text"
      placeholder={$tStore('searchProject')}
      bind:value={filterProject}
      style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:5px 10px;color:var(--fg);font-size:12px;width:140px;outline:none;"
    />
    <select
      bind:value={filterStatus}
      style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:5px 10px;color:var(--fg);font-size:12px;outline:none;"
    >
      <option value="">{$tStore('allStatus')}</option>
      <option value="complete">complete</option>
      <option value="failed">failed</option>
      <option value="aborted">aborted</option>
      <option value="paused">paused</option>
      <option value="timeout">timeout</option>
    </select>
    <DateRangePicker
      from={filterDateFrom}
      to={filterDateTo}
      onchange={(f, t) => { filterDateFrom = f; filterDateTo = t; }}
    />
    {#if hasFilter}
      <button
        onclick={clearFilters}
        style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);padding:5px 10px;color:var(--muted);font-size:12px;cursor:pointer;"
      >{$tStore('reset')}</button>
    {/if}
  </div>

  <div class="panel-body" style="padding:0;">
    {#if loading}
      <div style="padding:16px;color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
    {:else if filtered().length === 0}
      <div style="padding:32px;text-align:center;color:var(--muted);">
        <div style="font-size:28px;margin-bottom:8px;">📋</div>
        <div>{hasFilter ? $tStore('noResults') : $tStore('noPipelines')}</div>
        {#if hasFilter}
          <button onclick={clearFilters} style="margin-top:8px;background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 12px;color:var(--muted);font-size:12px;cursor:pointer;">{$tStore('reset')}</button>
        {/if}
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>ID</th>
            <th>Project</th>
            <th>Goal</th>
            <th>Mode</th>
            <th>Status</th>
            <th>Started</th>
            <th>Duration</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each pageItems as p}
            <tr>
              <td style="font-family:var(--font-mono);font-size:11px;color:var(--muted)">{shortId(p.id)}</td>
              <td style="font-size:11px;color:var(--fg-secondary);max-width:110px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title={p._project}>{projectLabel(p._project)}</td>
              <td style="color:var(--fg);max-width:160px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title={p.goal_slug ?? ''}>{p.goal_slug ?? '--'}</td>
              <td style="color:var(--fg-secondary)">{p.mode ?? '--'}</td>
              <td><span class="pill {statusBadgeClass(p.status)}">{p.status}</span></td>
              <td style="font-family:var(--font-mono);font-size:11px;white-space:nowrap;">{fmtDatetime(p.started_at)}</td>
              <td style="font-family:var(--font-mono);text-align:right;">{$tStore('durationMin', durationMinutes(p.started_at, p.updated_at))}</td>
              <td>
                <button
                  onclick={() => dismiss(p.id)}
                  title="Dismiss"
                  style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);width:20px;height:20px;display:flex;align-items:center;justify-content:center;color:var(--muted);font-size:11px;cursor:pointer;padding:0;line-height:1;"
                >✕</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>

      <!-- Pagination -->
      <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-top:1px solid var(--border);">
        <span style="font-size:12px;color:var(--muted);">
          {(page-1)*PAGE_SIZE+1}–{Math.min(page*PAGE_SIZE, filtered().length)} / {filtered().length}
        </span>
        <div style="display:flex;gap:4px;align-items:center;">
          <button onclick={() => page = 1} disabled={page === 1}
            style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{page===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{page===1?'default':'pointer'};">«</button>
          <button onclick={() => page--} disabled={page === 1}
            style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{page===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{page===1?'default':'pointer'};">‹</button>
          {#each Array.from({length: totalPages}, (_, i) => i + 1).filter(n => Math.abs(n - page) <= 2) as n}
            <button onclick={() => page = n}
              style="padding:3px 8px;border:1px solid {n===page?'var(--accent)':'var(--border)'};border-radius:var(--radius-sm);background:{n===page?'var(--accent-soft)':'var(--bg)'};color:{n===page?'var(--accent)':'var(--fg)'};font-size:12px;cursor:pointer;font-weight:{n===page?'600':'400'};">{n}</button>
          {/each}
          <button onclick={() => page++} disabled={page === totalPages}
            style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{page===totalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{page===totalPages?'default':'pointer'};">›</button>
          <button onclick={() => page = totalPages} disabled={page === totalPages}
            style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{page===totalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{page===totalPages?'default':'pointer'};">»</button>
        </div>
      </div>
    {/if}
  </div>
</div>

<!-- Safety Mechanisms -->
<div class="grid-2" style="margin-top:16px;">
  <div class="panel">
    <div class="panel-header"><h3>Pipeline State Schema</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>Field</th><th>Value</th></tr></thead>
        <tbody>
          <tr><td style="color:var(--muted)">id</td><td style="color:var(--fg)">{'{timestamp}'}</td></tr>
          <tr><td style="color:var(--muted)">mode</td><td style="color:var(--fg)">interactive | council | direct</td></tr>
          <tr><td style="color:var(--muted)">phase</td><td style="color:var(--accent)">spec → go → check → ship</td></tr>
          <tr><td style="color:var(--muted)">status</td><td style="color:var(--fg)">running | complete | failed | timeout | aborted</td></tr>
          <tr><td style="color:var(--muted)">check_fail_count</td><td style="color:var(--fg)">0..3</td></tr>
          <tr><td style="color:var(--muted)">deadline</td><td style="color:var(--fg)">now + 30min</td></tr>
          <tr><td style="color:var(--muted)">worktree_name</td><td style="color:var(--fg)">isolated build env</td></tr>
        </tbody>
      </table>
    </div>
  </div>
  <div class="panel">
    <div class="panel-header"><h3>Safety Mechanisms</h3></div>
    <div class="panel-body">
      <ul class="activity-list">
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--success)"></span>
          <div class="activity-content">
            <div class="activity-title">Concurrent orbit guard</div>
            <div class="activity-time">{$tStore('safetyGuard')}</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--accent)"></span>
          <div class="activity-content">
            <div class="activity-title">Deadline enforcement</div>
            <div class="activity-time">{$tStore('safetyDeadline')}</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--warning)"></span>
          <div class="activity-content">
            <div class="activity-title">Crash recovery</div>
            <div class="activity-time">{$tStore('safetyCrash')}</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--purple)"></span>
          <div class="activity-content">
            <div class="activity-title">Worktree safety</div>
            <div class="activity-time">{$tStore('safetyWorktree')}</div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</div>
