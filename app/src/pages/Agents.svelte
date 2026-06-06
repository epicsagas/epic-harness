<script lang="ts">
  import { tStore } from '$lib/i18n.js';
  import { getOrchestratorRun, getOrchestratorAgentStatus, dismissAgent, getAgentEvents, getAgentInbox } from '../lib/harness.js';
  import type { OrchestrationRun, OrchAgentDef, OrchAgentStatus, AgentEvent, InboxMessage } from '../lib/harness.js';
  import { getObsSummary } from '../lib/harness.js';
  import type { ObsSummary } from '../lib/harness.js';
  import { selectedProject } from '$lib/stores/project.js';

  let run = $state<OrchestrationRun | null>(null);
  let agentStatuses = $state<Map<string, OrchAgentStatus>>(new Map());
  let obs = $state<ObsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let expandedAgent = $state<string | null>(null);
  let agentEvents = $state<Map<string, AgentEvent[]>>(new Map());
  let agentInboxes = $state<Map<string, InboxMessage[]>>(new Map());

  async function load() {
    try {
      error = null;
      const [orchRun, orchObs] = await Promise.all([
        getOrchestratorRun(),
        getObsSummary(),
      ]);
      run = orchRun;
      obs = orchObs;

      // Load per-agent statuses
      if (orchRun && orchRun.agents) {
        const statusMap = new Map<string, OrchAgentStatus>();
        await Promise.all(
          orchRun.agents.map(async (agent) => {
            try {
              const status = await getOrchestratorAgentStatus(agent.id);
              if (status) statusMap.set(agent.id, status);
            } catch { /* skip missing */ }
          })
        );
        agentStatuses = statusMap;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    $selectedProject;
    loading = true;
    load();
    const id = setInterval(() => {
      if (!document.hidden) load();
    }, 5000);
    return () => clearInterval(id);
  });

  async function toggleEvents(agentId: string) {
    if (expandedAgent === agentId) { expandedAgent = null; return; }
    expandedAgent = agentId;
    const promises: Promise<void>[] = [];
    if (!agentEvents.has(agentId)) {
      promises.push(getAgentEvents(agentId).then(evts => { agentEvents.set(agentId, evts); }));
    }
    if (!agentInboxes.has(agentId)) {
      promises.push(getAgentInbox(agentId).then(msgs => { agentInboxes.set(agentId, msgs); }));
    }
    await Promise.all(promises);
  }

  function statusPillClass(status: string): string {
    switch (status) {
      case 'running': return 'info';
      case 'done': return 'success';
      case 'failed': return 'danger';
      case 'blocked': return 'warning';
      default: return 'info';
    }
  }

  function runStatusPillClass(status: string): string {
    switch (status) {
      case 'running': return 'info';
      case 'complete': return 'success';
      case 'aborted': return 'danger';
      case 'paused': return 'warning';
      default: return 'info';
    }
  }

  function relativeTime(ts: string): string {
    if (!ts) return '--';
    const diff = Math.floor((Date.now() - new Date(ts).getTime()) / 1000);
    if (diff < 0) return 'just now';
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    return `${Math.floor(diff / 3600)}h ago`;
  }

  function progressPercent(agent: OrchAgentDef): number {
    const status = agentStatuses.get(agent.id);
    if (status) return Math.round(status.progress * 100);
    if (agent.status === 'done') return 100;
    return 0;
  }

  const activeCount = $derived(
    run ? run.agents.filter(a => a.status === 'running' || a.status === 'blocked').length : 0
  );
  const completedCount = $derived(
    run ? run.agents.filter(a => a.status === 'done').length : 0
  );
  const failedCount = $derived(
    run ? run.agents.filter(a => a.status === 'failed').length : 0
  );
  const activeAgents = $derived(
    run ? run.agents.filter(a => a.status !== 'done') : []
  );
  const completedAgents = $derived(
    run ? run.agents.filter(a => a.status === 'done') : []
  );

  // Low success tools from obs
  const lowSuccessTools = $derived(
    obs ? obs.tool_stats.filter(t => t.success_rate < 0.85) : []
  );

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
  <h2>{$tStore('pageAgentsLive')}</h2>
  <p>{$tStore('pageAgentsLiveDesc')}</p>
</div>

{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body">
      <span style="color:var(--danger)">{$tStore('loadError')}: {error}</span>
    </div>
  </div>
{/if}

<!-- Orchestration Run Status -->
{#if loading}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('liveAgentStatus')}</h3></div>
    <div class="panel-body">
      <div style="display:flex;gap:12px;flex-wrap:wrap;">
        {#each [1, 2] as _}
          <div class="agent-card" style="flex:1;min-width:200px;">
            <div class="skeleton" style="width:80px;height:16px;border-radius:3px;background:var(--border);margin-bottom:8px;"></div>
            <div class="skeleton" style="width:100%;height:12px;border-radius:3px;background:var(--border);margin-bottom:4px;"></div>
            <div class="skeleton" style="width:60%;height:12px;border-radius:3px;background:var(--border);"></div>
          </div>
        {/each}
      </div>
    </div>
  </div>
{:else if run}
  <!-- Active Orchestration Run -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header">
      <h3>{$tStore('liveAgentStatus')}</h3>
      <span class="pill {runStatusPillClass(run.status)}" style="margin-left:8px;">{run.status}</span>
    </div>
    <div class="panel-body">
      <!-- Run Summary -->
      <div class="stats-grid" style="margin-bottom:16px;">
        <div class="stat-card">
          <div class="stat-label"><span class="dot" style="background:var(--accent)"></span> {$tStore('runIdLabel')}</div>
          <div class="stat-value" style="font-family:var(--font-mono);font-size:14px;">{run.id}</div>
          <div class="stat-sub">{$tStore('orchRunSub')}</div>
        </div>
        <div class="stat-card">
          <div class="stat-label"><span class="dot" style="background:var(--info)"></span> {$tStore('activeLabel')}</div>
          <div class="stat-value">{activeCount}</div>
          <div class="stat-sub">{$tStore('ofAgents', run.agents.length)}</div>
        </div>
        <div class="stat-card">
          <div class="stat-label"><span class="dot" style="background:var(--success)"></span> {$tStore('completedLabel')}</div>
          <div class="stat-value">{completedCount}</div>
          <div class="stat-sub">{$tStore('doneLabel')}</div>
        </div>
        <div class="stat-card">
          <div class="stat-label"><span class="dot" style="background:var(--danger)"></span> {$tStore('failedStatusLabel')}</div>
          <div class="stat-value">{failedCount}</div>
          <div class="stat-sub">{$tStore('failedLabel')}</div>
        </div>
      </div>

      <!-- Agent Cards (active only) -->
      <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:12px;">
        {#each activeAgents as agent}
          {@const status = agentStatuses.get(agent.id)}
          <div class="agent-card">
            <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:6px;">
              <div style="display:flex;align-items:center;gap:6px;">
                <div class="agent-name">{agent.role}</div>
                <span class="pill {statusPillClass(agent.status)}">{agent.status}</span>
              </div>
              <button
                onclick={async () => { await dismissAgent(agent.id); await load(); }}
                title="Dismiss"
                style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);width:20px;height:20px;display:flex;align-items:center;justify-content:center;color:var(--muted);font-size:11px;cursor:pointer;padding:0;line-height:1;"
              >✕</button>
            </div>
            <div class="agent-desc" style="font-size:12px;color:var(--fg-secondary);margin-bottom:8px;">
              {agent.task}
            </div>
            <!-- Progress bar -->
            <div style="background:var(--border);border-radius:4px;height:6px;overflow:hidden;margin-bottom:6px;">
              <div
                style="height:100%;border-radius:4px;transition:width 0.5s ease;background:{agent.status === 'done' ? 'var(--success)' : agent.status === 'failed' ? 'var(--danger)' : 'var(--accent)'};width:{progressPercent(agent)}%;"
              ></div>
            </div>
            <div style="display:flex;align-items:center;justify-content:space-between;font-size:11px;color:var(--muted);">
              <span>{$tStore('agentIdLabel')} <code>{agent.id}</code></span>
              {#if status}
                <span>{$tStore('heartbeatLabel')} {relativeTime(status.last_heartbeat)}</span>
              {:else if agent.completed_at}
                <span>{$tStore('completedAtLabel')} {relativeTime(agent.completed_at)}</span>
              {/if}
            </div>
            {#if agent.satisfies.length > 0}
              <div style="margin-top:6px;">
                {#each agent.satisfies as req}
                  <span class="pill info" style="font-size:10px;margin-right:4px;">{req}</span>
                {/each}
              </div>
            {/if}
            <!-- R4: Event log toggle -->
            <button
              onclick={() => toggleEvents(agent.id)}
              style="margin-top:8px;background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);padding:2px 8px;color:var(--muted);font-size:11px;cursor:pointer;width:100%;"
            >
              {$tStore('agentEventLog')} {expandedAgent === agent.id ? '▲' : '▼'}
            </button>
            {#if expandedAgent === agent.id}
              <div style="margin-top:6px;max-height:150px;overflow-y:auto;font-size:11px;">
                {#if agentEvents.get(agent.id)?.length}
                  <table class="data-table" style="font-size:10px;">
                    <thead><tr><th>{$tStore('colTimestamp')}</th><th>{$tStore('colEventType')}</th></tr></thead>
                    <tbody>
                      {#each agentEvents.get(agent.id) ?? [] as evt}
                        <tr>
                          <td style="font-family:var(--font-mono);color:var(--muted);">{evt.timestamp.slice(11, 19)}</td>
                          <td><code>{evt.event_type}</code></td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {:else}
                  <div style="color:var(--muted);text-align:center;padding:8px;">{$tStore('noEvents')}</div>
                {/if}
              </div>
              <!-- R7: Inbox messages -->
              {#if (agentInboxes.get(agent.id)?.length ?? 0) > 0}
                <div style="margin-top:6px;max-height:120px;overflow-y:auto;font-size:11px;">
                  <div style="color:var(--muted);margin-bottom:4px;font-size:10px;font-weight:600;">{$tStore('agentInboxTitle')} ({agentInboxes.get(agent.id)!.length})</div>
                  {#each agentInboxes.get(agent.id)!.slice(0, 10) as msg}
                    <div style="padding:3px 0;border-bottom:1px solid var(--border);display:flex;gap:6px;align-items:baseline;">
                      <span style="font-family:var(--font-mono);color:var(--muted);font-size:9px;">{msg.timestamp.slice(11, 19)}</span>
                      <span style="color:var(--accent);font-size:10px;">{msg.from} →</span>
                      <span style="color:var(--fg-secondary);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title={msg.message}>{msg.message}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        {/each}
      </div>

      <!-- Completed Agents Table -->
      {#if completedAgents.length > 0}
        <div style="margin-top:16px;">
          <h4 style="margin-bottom:8px;font-size:13px;color:var(--fg-secondary);">{$tStore('completedLabel')} ({completedAgents.length})</h4>
          <table class="data-table">
            <thead>
              <tr>
                <th>{$tStore('colAgents')}</th>
                <th>{$tStore('colStatus')}</th>
                <th>{$tStore('colGoal')}</th>
                <th>{$tStore('completedAtLabel')}</th>
              </tr>
            </thead>
            <tbody>
              {#each completedAgents as agent}
                <tr>
                  <td style="font-family:var(--font-mono);font-size:11px;">{agent.role}</td>
                  <td><span class="pill {statusPillClass(agent.status)}">{agent.status}</span></td>
                  <td style="font-size:12px;color:var(--fg-secondary);max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{agent.task}</td>
                  <td style="font-size:11px;color:var(--muted);">{agent.completed_at ? relativeTime(agent.completed_at) : '--'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

      <!-- Dependency Graph -->
      {#if Object.keys(run.dependency_graph).length > 0}
        <div style="margin-top:16px;">
          <h4 style="margin-bottom:8px;font-size:13px;color:var(--fg-secondary);">{$tStore('depGraphTitle')}</h4>
          <table class="data-table">
            <thead>
              <tr>
                <th>{$tStore('colAgentId')}</th>
                <th>{$tStore('colDependsOn')}</th>
                <th>{$tStore('colStatus')}</th>
              </tr>
            </thead>
            <tbody>
              {#each Object.entries(run.dependency_graph) as [agentId, deps]}
                {@const agentDef = run!.agents.find(a => a.id === agentId)}
                <tr>
                  <td style="font-family:var(--font-mono);font-size:11px;">{agentId}</td>
                  <td style="font-family:var(--font-mono);font-size:11px;color:var(--fg-secondary);">{deps.join(', ')}</td>
                  <td>
                    {#if agentDef}
                      <span class="pill {statusPillClass(agentDef.status)}">{agentDef.status}</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>
{:else}
  <!-- No active orchestration -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('liveAgentStatus')}</h3></div>
    <div class="panel-body">
      <div style="text-align:center;padding:40px 24px;">
        <div style="font-size:48px;margin-bottom:12px;opacity:0.3;">&#9734;</div>
        <div style="font-size:15px;color:var(--fg);margin-bottom:6px;">{$tStore('noActiveOrchestration')}</div>
        <div style="font-size:13px;color:var(--muted);">{$tStore('noActiveOrchestrationHint')}</div>
      </div>
    </div>
  </div>
{/if}

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
