<script lang="ts">
  import { getHarnessMetrics, getObsSummary } from '../lib/harness.js';
  import type { HarnessMetrics, ObsSummary } from '../lib/harness.js';
  import { tStore } from '$lib/i18n.js';
  import { selectedProject } from '$lib/stores/project.js';

  let metrics = $state<HarnessMetrics | null>(null);
  let obs = $state<ObsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let loadGeneration = 0;

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

  const totalFailures = $derived(
    obs ? obs.recent_sessions.reduce((sum, s) => sum + s.failures, 0) : 0
  );

  const topTools = $derived(
    obs ? [...obs.tool_stats].sort((a, b) => b.calls - a.calls).slice(0, 5) : []
  );

  // R5: Sparkline from score_history
  const sparklinePoints = $derived(() => {
    if (!metrics?.score_history?.length) return '';
    const entries = metrics.score_history.slice(-20);
    const w = 200, h = 40, pad = 4;
    const max = Math.max(...entries.map(e => e.avg_score), 0.01);
    const min = Math.min(...entries.map(e => e.avg_score));
    const range = max - min || 0.01;
    return entries.map((e, i) => {
      const x = pad + (i / Math.max(entries.length - 1, 1)) * (w - 2 * pad);
      const y = h - pad - ((e.avg_score - min) / range) * (h - 2 * pad);
      return `${x},${y}`;
    }).join(' ');
  });

  // R1: Latest dimension averages
  const latestDims = $derived(() => {
    if (!metrics?.score_history?.length) return null;
    const last = metrics.score_history[metrics.score_history.length - 1];
    return last.dimension_averages ?? null;
  });

  // circumference = 2 * pi * 42 ≈ 264
  const evalRingOffset = $derived(
    metrics ? Math.round(264 * (1 - metrics.avg_score)) : 264
  );

  async function load(generation: number) {
    try {
      error = null;
      const [m, o] = await Promise.all([getHarnessMetrics(), getObsSummary()]);
      if (generation !== loadGeneration) return;
      metrics = m;
      obs = o;
      flashIfChanged('metrics', metrics);
      flashIfChanged('obs', obs);
    } catch (e) {
      if (generation !== loadGeneration) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (generation === loadGeneration) loading = false;
    }
  }

  $effect(() => {
    const _project = $selectedProject; // reactive dependency
    const gen = ++loadGeneration;
    loading = true;
    load(gen);
    const id = setInterval(() => { if (!document.hidden) load(gen); }, 30000);
    return () => clearInterval(id);
  });

  function fmtScore(v: number): string {
    return v.toFixed(3);
  }

  function dateOnly(ts: string | null | undefined): string {
    if (!ts) return '—';
    return ts.slice(0, 10);
  }

  function trendPillClass(trend: string): string {
    if (trend === 'improving') return 'success';
    if (trend === 'declining') return 'danger';
    return 'info';
  }
</script>

<div class="screen-header">
  <h2>{$tStore('pageDashboard')}</h2>
  <p>{$tStore('pageDashboardDesc')}</p>
</div>

<!-- 4-Ring Status (static) -->
<div class="grid-4" style="margin-bottom:24px;">
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--success-soft);color:var(--success);">0</div>
    <div class="ring-info">
      <div class="ring-title">Autopilot</div>
      <div class="ring-desc">{$tStore('ring0Desc')}</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--accent-soft);color:var(--accent);">1</div>
    <div class="ring-info">
      <div class="ring-title">Pipeline</div>
      <div class="ring-desc">{$tStore('ring1Desc')}</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--purple-soft);color:var(--purple);">2</div>
    <div class="ring-info">
      <div class="ring-title">Auto Skills</div>
      <div class="ring-desc">{$tStore('ring2Desc')}</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--orange-soft);color:var(--orange);">3</div>
    <div class="ring-info">
      <div class="ring-title">Evolution</div>
      <div class="ring-desc">{$tStore('ring3Desc')}</div>
    </div>
  </div>
</div>

{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body">
      <span style="color:var(--danger)">{$tStore('loadError')}: {error}</span>
    </div>
  </div>
{/if}

<!-- Key Metrics -->
<div class="stats-grid" class:hx-flash={flash.metrics}>
  <!-- Sessions -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--success)"></span> {$tStore('statSessions')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:40px;height:28px;"></div>
    {:else}
      <div class="stat-value">{metrics?.session_count ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statSessionsSub')}</div>
  </div>
  <!-- Avg Score -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--accent)"></span> {$tStore('statAvgScore')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:60px;height:28px;"></div>
    {:else}
      <div class="stat-value">{metrics ? fmtScore(metrics.avg_score) : '--'}</div>
    {/if}
    <!-- R5: Sparkline -->
    {#if metrics?.score_history && metrics.score_history.length >= 2}
      <svg width="200" height="40" style="display:block;margin-top:6px;">
        <polyline fill="none" stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"
                  points={sparklinePoints()} />
      </svg>
    {/if}
    <div class="stat-sub">{$tStore('statAvgScoreSub')}</div>
  </div>
  <!-- Trend -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--teal)"></span> {$tStore('statTrend')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:80px;height:28px;"></div>
    {:else}
      <div class="stat-value">
        {#if metrics}
          <span class="pill {trendPillClass(metrics.trend ?? 'stable')}">{metrics.trend ?? 'stable'}</span>
        {:else}
          --
        {/if}
      </div>
    {/if}
    <div class="stat-sub">{$tStore('statTrendSub')}</div>
  </div>
  <!-- Stagnation -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--warning)"></span> {$tStore('statStagnation')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:30px;height:28px;"></div>
    {:else}
      <div class="stat-value">{metrics?.stagnation_count ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statStagnationSub')}</div>
  </div>
  <!-- Best Score (R2) -->
  {#if metrics?.best_score != null}
    <div class="stat-card">
      <div class="stat-label"><span class="dot" style="background:var(--teal)"></span> {$tStore('statBestScore')}</div>
      <div class="stat-value">{fmtScore(metrics.best_score)}</div>
      {#if metrics.best_session}
        <div class="stat-sub mono-sm">{dateOnly(metrics.best_session)}</div>
      {/if}
    </div>
  {/if}
  <!-- Total Calls -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--purple)"></span> {$tStore('statTotalCalls')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:50px;height:28px;"></div>
    {:else}
      <div class="stat-value">{obs?.total_tool_calls ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statTotalCallsSub')}</div>
  </div>
  <!-- Failures -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--danger)"></span> {$tStore('statFailures')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:30px;height:28px;"></div>
    {:else}
      <div class="stat-value">{obs ? totalFailures : '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('failuresSub')}</div>
  </div>
</div>

<!-- Eval Ring + Tool Stats -->
<div class="grid-2" style="margin-bottom:16px;">
  <div class="panel">
    <div class="panel-header">
      <h3>{$tStore('evalScoreTitle')}</h3>
    </div>
    <div class="panel-body">
      <div style="display:flex;gap:32px;align-items:center;justify-content:center;">
        <div class="usage-ring-container">
          <svg width="90" height="90" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--border)" stroke-width="6"/>
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--accent)" stroke-width="6"
                    stroke-dasharray="264"
                    stroke-dashoffset={loading ? 264 : evalRingOffset}
                    stroke-linecap="round"
                    transform="rotate(-90 50 50)"/>
          </svg>
          <div class="usage-ring-label">
            <div class="ring-value">
              {#if loading}
                <span style="color:var(--muted)">...</span>
              {:else}
                {metrics ? fmtScore(metrics.avg_score) : '--'}
              {/if}
            </div>
            <div class="ring-text">{$tStore('statAvgScore')}</div>
          </div>
        </div>
      </div>
      <div style="margin-top:16px;font-size:11px;color:var(--muted);text-align:center;font-family:var(--font-mono);">
        composite = 0.5 &times; success + 0.3 &times; quality + 0.2 &times; cost
        {#if metrics}
          = <strong style="color:var(--fg)">{fmtScore(metrics.avg_score)}</strong>
        {/if}
      </div>
      <!-- R1: Dimension bars -->
      {#if latestDims()}
        {@const dims = latestDims()}
        <div style="margin-top:14px;display:flex;flex-direction:column;gap:6px;">
          {#each [
            { key: 'tool_success', label: $tStore('dimToolSuccess'), color: 'var(--success)' },
            { key: 'output_quality', label: $tStore('dimOutputQuality'), color: 'var(--accent)' },
            { key: 'execution_cost', label: $tStore('dimExecCost'), color: 'var(--teal)' },
          ] as dim}
            {@const val = dims![dim.key] ?? 0}
            <div style="display:flex;align-items:center;gap:8px;font-size:11px;">
              <span style="width:100px;color:var(--muted);text-align:right;">{dim.label}</span>
              <div style="flex:1;height:6px;background:var(--border);border-radius:3px;overflow:hidden;">
                <div style="width:{Math.round(val * 100)}%;height:100%;background:{dim.color};border-radius:3px;transition:width 0.3s;"></div>
              </div>
              <span style="font-family:var(--font-mono);width:36px;text-align:right;">{val.toFixed(2)}</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="panel" class:hx-flash={flash.obs}>
    <div class="panel-header">
      <h3>{$tStore('toolStatsTitle')}</h3>
    </div>
    <div class="panel-body" style="padding:0;">
      {#if loading}
        <div style="padding:16px;color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>{$tStore('colTool')}</th>
              <th>{$tStore('colCalls')}</th>
              <th>{$tStore('colSuccessRate')}</th>
              <th>{$tStore('colAvgScore')}</th>
            </tr>
          </thead>
          <tbody>
            {#each topTools as stat}
              <tr>
                <td style="color:var(--fg)">{stat.tool}</td>
                <td>{stat.calls}</td>
                <td>
                  <span class="pill {stat.success_rate >= 0.9 ? 'success' : stat.success_rate >= 0.75 ? 'warning' : 'danger'}">
                    {(stat.success_rate * 100).toFixed(0)}%
                  </span>
                </td>
                <td style="font-family:var(--font-mono)">{fmtScore(stat.avg_score)}</td>
              </tr>
            {:else}
              <tr><td colspan="4" style="color:var(--muted);">{$tStore('noData')}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
</div>

<!-- R2: Failure Categories -->
{#if obs?.failure_categories && obs.failure_categories.length > 0}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header">
      <h3>{$tStore('failureCategoriesTitle')}</h3>
    </div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead>
          <tr>
            <th>{$tStore('colFailureType')}</th>
            <th>{$tStore('colCalls')}</th>
          </tr>
        </thead>
        <tbody>
          {#each obs.failure_categories as fc}
            <tr>
              <td style="color:var(--fg)"><code>{fc.category}</code></td>
              <td>
                <span class="pill {fc.count >= 10 ? 'danger' : fc.count >= 5 ? 'warning' : 'info'}">{fc.count}</span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}

<!-- Activity Log -->
<div class="panel">
  <div class="panel-header">
    <h3>{$tStore('recentActivityTitle')}</h3>
  </div>
  <div class="panel-body">
    {#if loading}
      {#each [1, 2, 3] as _}
        <div class="activity-item" style="margin-bottom:10px;">
          <span class="activity-dot" style="background:var(--border)"></span>
          <div style="flex:1;display:flex;flex-direction:column;gap:4px;">
            <div class="skeleton" style="width:60%;height:13px;"></div>
            <div class="skeleton" style="width:30%;height:11px;"></div>
          </div>
        </div>
      {/each}
    {:else if obs && obs.recent_sessions.length > 0}
      <ul class="activity-list">
        {#each obs.recent_sessions as session}
          <li class="activity-item">
            <span class="activity-dot"
              style="background:{session.failures === 0 ? 'var(--success)' : session.failures <= 2 ? 'var(--warning)' : 'var(--danger)'}">
            </span>
            <div class="activity-content">
              <div class="activity-title">
                {$tStore('sessionLabel')} <code>{session.session_id}</code>
                &mdash; {session.tool_calls} {$tStore('callsAvgScore')} {fmtScore(session.avg_score)}
                {#if session.failures > 0}
                  <span class="pill danger" style="margin-left:6px;">{session.failures} {$tStore('failuresLabel')}</span>
                {/if}
              </div>
              <div class="activity-time">{session.date}</div>
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <p style="color:var(--muted);font-size:13px;">{$tStore('recentSessionNone')}</p>
    {/if}
  </div>
</div>
