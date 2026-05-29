<script lang="ts">
  import { onMount } from 'svelte';
  import { getHarnessMetrics, getObsSummary } from '../lib/harness.js';
  import type { HarnessMetrics, ObsSummary } from '../lib/harness.js';
  import { tStore } from '$lib/i18n.js';

  let metrics = $state<HarnessMetrics | null>(null);
  let obs = $state<ObsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const totalFailures = $derived(
    obs ? obs.recent_sessions.reduce((sum, s) => sum + s.failures, 0) : 0
  );

  const topTools = $derived(
    obs ? [...obs.tool_stats].sort((a, b) => b.calls - a.calls).slice(0, 5) : []
  );

  // circumference = 2 * pi * 42 ≈ 264
  const evalRingOffset = $derived(
    metrics ? Math.round(264 * (1 - metrics.avg_score)) : 264
  );

  async function load() {
    try {
      error = null;
      const [m, o] = await Promise.all([getHarnessMetrics(), getObsSummary()]);
      metrics = m;
      obs = o;
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

  function fmtScore(v: number): string {
    return v.toFixed(3);
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
<div class="stats-grid">
  <!-- Sessions -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--success)"></span> {$tStore('statSessions')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:40px;height:28px;border-radius:4px;background:var(--border);"></div>
    {:else}
      <div class="stat-value">{metrics?.session_count ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statSessionsSub')}</div>
  </div>
  <!-- Avg Score -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--accent)"></span> {$tStore('statAvgScore')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:60px;height:28px;border-radius:4px;background:var(--border);"></div>
    {:else}
      <div class="stat-value">{metrics ? fmtScore(metrics.avg_score) : '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statAvgScoreSub')}</div>
  </div>
  <!-- Trend -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--teal)"></span> {$tStore('statTrend')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:80px;height:28px;border-radius:4px;background:var(--border);"></div>
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
      <div class="stat-value skeleton" style="width:30px;height:28px;border-radius:4px;background:var(--border);"></div>
    {:else}
      <div class="stat-value">{metrics?.stagnation_count ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statStagnationSub')}</div>
  </div>
  <!-- Total Calls -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--purple)"></span> {$tStore('statTotalCalls')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:50px;height:28px;border-radius:4px;background:var(--border);"></div>
    {:else}
      <div class="stat-value">{obs?.total_tool_calls ?? '--'}</div>
    {/if}
    <div class="stat-sub">{$tStore('statTotalCallsSub')}</div>
  </div>
  <!-- Failures -->
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--danger)"></span> {$tStore('statFailures')}</div>
    {#if loading}
      <div class="stat-value skeleton" style="width:30px;height:28px;border-radius:4px;background:var(--border);"></div>
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
    </div>
  </div>

  <div class="panel">
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
            <div class="skeleton" style="width:60%;height:13px;border-radius:3px;background:var(--border);"></div>
            <div class="skeleton" style="width:30%;height:11px;border-radius:3px;background:var(--border);"></div>
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
