<script lang="ts">
  import { onMount } from 'svelte';
  import { getObsSummary } from '../lib/harness.js';
  import type { ObsSummary } from '../lib/harness.js';
  import { tStore } from '$lib/i18n.js';

  let obs = $state<ObsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const lowSuccessTools = $derived(
    obs ? obs.tool_stats.filter(t => t.success_rate < 0.85) : []
  );

  async function load() {
    try {
      error = null;
      obs = await getObsSummary();
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

  function truncate(s: string, len: number): string {
    return s.length > len ? s.slice(0, len) + '…' : s;
  }

  function relativeTime(ts: string): string {
    const diff = Math.floor((Date.now() - new Date(ts).getTime()) / 1000);
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    return `${Math.floor(diff / 3600)}h ago`;
  }

  function scorePillClass(score: number): string {
    if (score >= 0.9) return 'success';
    if (score >= 0.7) return 'warning';
    return 'danger';
  }

  function fmtScore(v: number): string {
    return v.toFixed(3);
  }
</script>

<div class="screen-header">
  <h2>{$tStore('pageAgents')}</h2>
  <p>{$tStore('pageAgentsDesc')}</p>
</div>

{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body">
      <span style="color:var(--danger)">{$tStore('loadError')}: {error}</span>
    </div>
  </div>
{/if}

<!-- Active Agents -->
<div class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>{$tStore('activeAgents')}</h3></div>
  <div class="panel-body">
    {#if loading}
      <div style="display:flex;gap:12px;flex-wrap:wrap;">
        {#each [1, 2] as _}
          <div class="agent-card" style="flex:1;min-width:200px;">
            <div class="skeleton" style="width:80px;height:16px;border-radius:3px;background:var(--border);margin-bottom:8px;"></div>
            <div class="skeleton" style="width:100%;height:12px;border-radius:3px;background:var(--border);margin-bottom:4px;"></div>
            <div class="skeleton" style="width:60%;height:12px;border-radius:3px;background:var(--border);"></div>
          </div>
        {/each}
      </div>
    {:else if obs && obs.active_agents.length > 0}
      <div style="display:flex;gap:12px;flex-wrap:wrap;">
        {#each obs.active_agents as agent}
          <div class="agent-card" style="flex:1;min-width:200px;">
            <div class="agent-name">{agent.name}</div>
            <div class="agent-role" style="margin-bottom:6px;">{$tStore('lastTool')} <code>{agent.last_tool}</code></div>
            <div class="agent-desc" style="font-size:12px;color:var(--fg-secondary);margin-bottom:8px;">
              {truncate(agent.last_action, 50)}
            </div>
            <div style="display:flex;align-items:center;justify-content:space-between;">
              <span class="pill {scorePillClass(agent.score)}">{$tStore('scoreLabel')} {fmtScore(agent.score)}</span>
              <span style="font-size:11px;color:var(--muted);">{relativeTime(agent.timestamp)}</span>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div style="text-align:center;padding:24px;color:var(--muted);">
        <div style="font-size:28px;margin-bottom:6px;">🤖</div>
        <div>{$tStore('noActiveAgent')}</div>
      </div>
    {/if}
  </div>
</div>

<!-- Internal Agent Cards (static) -->
<div class="grid-2">
  <div class="agent-card">
    <div class="agent-name">Builder</div>
    <div class="agent-role">epic:builder</div>
    <div class="agent-desc">{$tStore('agentBuilderDesc')}</div>
    <div style="margin-top:10px;"><span class="pill success">TDD</span><span class="pill info">/go</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Reviewer</div>
    <div class="agent-role">epic:reviewer</div>
    <div class="agent-desc">{$tStore('agentReviewerDesc')}</div>
    <div style="margin-top:10px;"><span class="pill warning">review</span><span class="pill info">/check</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Auditor</div>
    <div class="agent-role">epic:auditor</div>
    <div class="agent-desc">{$tStore('agentAuditorDesc')}</div>
    <div style="margin-top:10px;"><span class="pill danger">security</span><span class="pill info">/check</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Planner</div>
    <div class="agent-role">epic:planner</div>
    <div class="agent-desc">{$tStore('agentPlannerDesc')}</div>
    <div style="margin-top:10px;"><span class="pill purple">planning</span><span class="pill info">/go</span></div>
  </div>
</div>

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

<!-- /team Orchestration Patterns (static) -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>{$tStore('teamPatternsTitle')}</h3></div>
  <div class="panel-body" style="padding:0;">
    <table class="data-table">
      <thead>
        <tr>
          <th>{$tStore('colPattern')}</th>
          <th>{$tStore('colAgents')}</th>
          <th>{$tStore('colBestFor')}</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td style="color:var(--fg)">Pipeline</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">{$tStore('patternPipelineDesc')}</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Fan-out</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">{$tStore('patternFanOutDesc')}</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Expert-Pool</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">{$tStore('patternExpertPoolDesc')}</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Producer-Reviewer</td>
          <td>2-3</td>
          <td style="color:var(--fg-secondary)">{$tStore('patternProducerReviewerDesc')}</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Supervisor</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">{$tStore('patternSupervisorDesc')}</td>
        </tr>
      </tbody>
    </table>
  </div>
</div>

<!-- Session-based Agent Activity -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>{$tStore('sessionActivity')}</h3></div>
  <div class="panel-body" style="padding:0;">
    {#if loading}
      <div style="padding:16px;color:var(--muted);font-size:13px;">{$tStore('loading')}</div>
    {:else if obs && obs.recent_sessions.length > 0}
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
          {#each obs.recent_sessions as session}
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
