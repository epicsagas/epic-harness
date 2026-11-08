<script lang="ts">
  import { tStore } from '$lib/i18n.js';
  import type { LiveActivity, ActivityRun, OrchAgentDef } from '$lib/harness.js';
  let { activity, loading, now }: { activity: LiveActivity | null; loading: boolean; now: number } = $props();
  const sessions = $derived((activity?.sessions ?? []).filter(session => {
    const age = (now - Date.parse(session.last_seen)) / 1000;
    return age >= 0 && age <= (activity?.activity_window_seconds ?? 300);
  }));
  function ago(timestamp: string | null | undefined): string {
    if (!timestamp) return '--';
    const age = Math.max(0, Math.floor((now - Date.parse(timestamp)) / 1000));
    if (!Number.isFinite(age)) return '--';
    return age < 60 ? `${age}s ago` : age < 3600 ? `${Math.floor(age / 60)}m ago` : `${Math.floor(age / 3600)}h ago`;
  }
  function progress(run: ActivityRun, agent: OrchAgentDef): number {
    const value = run.agent_statuses[agent.id]?.progress ?? (agent.status === 'done' ? 1 : 0);
    return Number.isFinite(value) ? Math.max(0, Math.min(100, Math.round(value * 100))) : 0;
  }
  function stale(run: ActivityRun, agent: OrchAgentDef): boolean {
    const heartbeat = run.agent_statuses[agent.id]?.last_heartbeat ?? agent.started_at;
    return agent.status === 'running' && (!heartbeat || now - Date.parse(heartbeat) > 1800000);
  }
</script>

<section class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>{$tStore('recentCodingSessions')}</h3><span class="pill info">{sessions.length}</span></div>
  <div class="panel-body">
    <p class="activity-note">{$tStore('recentCodingSessionsHint')}</p>
    {#if loading}
      <p>{$tStore('loading')}</p>
    {:else if !activity}
      <p class="activity-note">{$tStore('liveActivityUnavailable')}</p>
    {:else if sessions.length === 0}
      <p class="activity-note">{$tStore('noRecentCodingSessions')}</p>
    {:else}
      <div class="activity-grid">
        {#each sessions as session (`${session.project}:${session.session_id}`)}
          <article class="agent-card">
            <div class="agent-name">{session.project || $tStore('unscopedProject')}</div>
            <code class="activity-id">{session.session_id}</code>
            <p class="activity-note">{$tStore('lastRecordedActivity')}: {ago(session.last_seen)}</p>
            <span class="pill info">{session.tool_calls} {$tStore('colToolCalls')}</span>
          </article>
        {/each}
      </div>
    {/if}
  </div>
</section>

{#each activity?.runs ?? [] as run (`${run.project}:${run.id}`)}
  <section class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('trackedSubagents')} · {run.project || $tStore('unscopedProject')}</h3><code>{run.id}</code></div>
    <div class="panel-body">
      <div class="activity-grid">
        {#each run.agents as agent (agent.id)}
          {@const status = run.agent_statuses[agent.id]}
          <article class="agent-card">
            <div style="display:flex;justify-content:space-between;gap:8px;">
              <div class="agent-name">{agent.role}</div>
              <span class="pill {agent.status === 'done' ? 'success' : agent.status === 'failed' ? 'danger' : 'info'}">{agent.status}</span>
            </div>
            <p>{agent.task}</p>
            <code class="activity-id">{agent.id}</code>
            <div role="progressbar" aria-label={agent.id} aria-valuenow={progress(run, agent)} aria-valuemin="0" aria-valuemax="100" class="activity-progress">
              <div style={`width:${progress(run, agent)}%;height:100%;background:var(--accent);`}></div>
            </div>
            <p class="activity-note">{status?.phase ?? agent.status} · {$tStore('heartbeatLabel')}: {ago(status?.last_heartbeat ?? agent.started_at)}</p>
            {#if stale(run, agent)}<span class="pill warning">{$tStore('staleAgentHeartbeat')}</span>{/if}
            {#if agent.satisfies?.length}<p class="activity-note">{agent.satisfies.join(', ')}</p>{/if}
          </article>
        {/each}
      </div>
      {#if Object.keys(run.dependency_graph ?? {}).length}
        <details style="margin-top:16px;">
          <summary>{$tStore('depGraphTitle')}</summary>
          {#each Object.entries(run.dependency_graph) as [agent, dependencies]}
            <p class="activity-note"><code>{agent}</code>: {dependencies.join(', ')}</p>
          {/each}
        </details>
      {/if}
    </div>
  </section>
{:else}
  {#if !loading && activity}
    <section class="panel" style="margin-bottom:16px;">
      <div class="panel-header"><h3>{$tStore('trackedSubagents')}</h3></div>
      <div class="panel-body activity-note">{$tStore('noTrackedSubagents')}</div>
    </section>
  {/if}
{/each}

<style>
  .activity-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(260px,1fr)); gap:12px; }
  .activity-note { color:var(--muted); font-size:12px; }
  .activity-id { display:block; overflow-wrap:anywhere; font-size:11px; color:var(--fg-secondary); }
  .activity-progress { height:6px; background:var(--border); border-radius:4px; overflow:hidden; margin-top:10px; }
</style>
