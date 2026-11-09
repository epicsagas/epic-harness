<script lang="ts">
  import { tStore } from '$lib/i18n.js';
  import { getLiveActivity, getObsSummary } from '../lib/harness.js';
  import type { LiveActivity, ObsSummary } from '../lib/harness.js';
  import LiveActivityView from '$lib/components/LiveActivity.svelte';
  import { selectedProject } from '$lib/stores/project.js';
  let activity = $state<LiveActivity | null>(null);
  let obs = $state<ObsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let now = $state(Date.now());
  const history = $derived(activity?.recent_sessions ?? []);
  const lowSuccessTools = $derived(obs?.tool_stats.filter(tool => tool.success_rate < 0.85) ?? []);
  $effect(() => {
    const project = $selectedProject;
    let cancelled = false;
    let pending = false;
    activity = null;
    obs = null;
    loading = true;
    error = null;
    async function refresh() {
      if (pending) return;
      pending = true;
      try {
        const [nextActivity, nextObs] = await Promise.all([getLiveActivity(project), getObsSummary()]);
        if (!cancelled) { activity = nextActivity; obs = nextObs; error = null; }
      } catch (reason) {
        if (!cancelled) error = reason instanceof Error ? reason.message : String(reason);
      } finally {
        pending = false;
        if (!cancelled) { loading = false; now = Date.now(); }
      }
    }
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => { cancelled = true; clearInterval(timer); };
  });
  function scorePillClass(score: number): string {
    return score >= 0.9 ? 'success' : score >= 0.7 ? 'warning' : 'danger';
  }
  function fmtScore(value: number): string { return value.toFixed(3); }
</script>
<div class="screen-header">
  <h2>{$tStore('pageAgentsLive')}</h2>
  <p>{$tStore('pageAgentsLiveDesc')}</p>
</div>
{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body" role="alert" style="color:var(--danger)">{$tStore('loadError')}: {error}</div>
  </div>
{/if}
<LiveActivityView {activity} {loading} {now} />

<!-- Low Success Tools -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>{$tStore('lowSuccessTools')}</h3></div>
  <div class="panel-body" style="padding:0;">
    {#if loading}
      <div style="padding:16px;color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
    {:else if lowSuccessTools.length === 0}
      <div style="padding:24px;text-align:center;color:var(--success);">
        <span class="pill success">{$tStore('allToolsOk')}</span>
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>{$tStore('colTool')}</th>
            <th>{$tStore('colCalls')}</th>
            <th>{$tStore('colSuccessRate')}</th>
            <th>{$tStore('colAvgScore')}</th>
            <th>{$tStore('colStatus')}</th>
          </tr>
        </thead>
        <tbody>
          {#each lowSuccessTools as stat}
            <tr>
              <td style="color:var(--fg)">{stat.tool}</td>
              <td>{stat.calls}</td>
              <td>
                <span class="pill {stat.success_rate >= 0.75 ? 'warning' : 'danger'}">
                  {(stat.success_rate * 100).toFixed(0)}%
                </span>
              </td>
              <td style="font-family:var(--font-mono)">{fmtScore(stat.avg_score)}</td>
              <td><span class="pill warning">{$tStore('needsAttention')}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<!-- Session Activity -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>{$tStore('sessionActivity')}</h3></div>
  <div class="panel-body" style="padding:0;">
    {#if loading}
      <div style="padding:16px;color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
    {:else if history.length > 0}
      <table class="data-table">
        <thead>
          <tr>
            <th>{$tStore('colSessionId')}</th>
            <th>{$tStore('colDate')}</th>
            <th>{$tStore('colToolCalls')}</th>
            <th>{$tStore('colAvgScore')}</th>
            <th>{$tStore('colFailures')}</th>
          </tr>
        </thead>
        <tbody>
          {#each history as session}
            <tr>
              <td style="font-family:var(--font-mono);font-size:11px;color:var(--muted)">{session.session_id}</td>
              <td style="font-family:var(--font-mono)">{session.date}</td>
              <td>{session.tool_calls}</td>
              <td>
                <span class="pill {scorePillClass(session.avg_score)}">{fmtScore(session.avg_score)}</span>
              </td>
              <td>
                {#if session.failures === 0}
                  <span class="pill success">0</span>
                {:else}
                  <span class="pill {session.failures <= 2 ? 'warning' : 'danger'}">{session.failures}</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div style="padding:24px;text-align:center;color:var(--muted);">{$tStore('noRecentSession')}</div>
    {/if}
  </div>
</div>
