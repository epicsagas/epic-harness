<script lang="ts">
  import { getHarnessMetrics, getSessionSnapshots, getGlobalPatterns, type HarnessMetrics, type SessionSnapshotData, type GlobalPattern } from '../lib/harness.js';
  import { tStore } from '$lib/i18n.js';
  import { selectedProject } from '$lib/stores/project.js';

  let metrics = $state<HarnessMetrics | null>(null);
  let snapshots = $state<SessionSnapshotData[]>([]);
  let patterns = $state<GlobalPattern[]>([]);
  let loading = $state(true);
  let error = $state('');
  let staleGeneration = 0;

  // Score weights are compile-time constants defined in common.rs
  const successWeight = 0.5;
  const qualityWeight = 0.3;
  const costWeight = 0.2;

  function dateOnly(ts: string | null | undefined): string {
    if (!ts) return '—';
    return ts.slice(0, 10);
  }

  async function loadSettings(generation: number) {
    try {
      loading = true;
      const [m, s, p] = await Promise.all([
        getHarnessMetrics(),
        getSessionSnapshots(),
        getGlobalPatterns(),
      ]);
      // Discard stale results if project changed while loading
      if (generation !== staleGeneration) return;
      metrics = m;
      snapshots = s;
      patterns = p;
    } catch (e) {
      if (generation !== staleGeneration) return;
      error = String(e);
    } finally {
      if (generation === staleGeneration) loading = false;
    }
  }

  $effect(() => {
    const _project = $selectedProject; // reactive dependency
    const gen = ++staleGeneration;
    loadSettings(gen);
  });

  const EVOLUTION_CONSTANTS = [
    { name: 'WEAK_TOOL_RATE',        value: '0.6',  desc: 'Min success rate before skill seeding' },
    { name: 'WEAK_TOOL_MIN_OBS',     value: '5',    desc: 'Min observations for weak tool detection' },
    { name: 'WEAK_EXT_RATE',         value: '0.5',  desc: 'File type weakness threshold' },
    { name: 'WEAK_EXT_MIN_OBS',      value: '3',    desc: 'Min observations for file type' },
    { name: 'HIGH_FREQ_ERROR_MIN',   value: '5',    desc: 'Error frequency for seeding' },
    { name: 'STAGNATION_LIMIT',      value: '3',    desc: 'Sessions without improvement before rollback' },
    { name: 'IMPROVEMENT_THRESHOLD', value: '5%',   desc: 'Required improvement to reset stagnation' },
    { name: 'MAX_EVOLVED_SKILLS',    value: '10',   desc: 'Max evolved skills cap' },
    { name: 'REPEATED_ERROR_MIN',    value: '3',    desc: 'Consecutive same-error threshold' },
    { name: 'FTB_MIN_CYCLES',        value: '2',    desc: 'Fix-then-break cycle minimum' },
    { name: 'DEBUG_LOOP_MIN',        value: '5',    desc: 'Long debug loop threshold' },
  ];
</script>

<div class="page">
  <div class="page-header">
    <h1>{$tStore('pageSettings')}</h1>
    <p>{$tStore('pageSettingsDesc')}</p>
  </div>

  {#if loading}
    <div class="loading">{$tStore('loadingMetrics')}</div>
  {:else if error}
    <div class="error-msg">{error}</div>
  {:else}
    <!-- 1. Eval Weights -->
    <div class="card" style="margin-bottom:1rem;">
      <h3>{$tStore('evalWeightsTitle')}</h3>
      <p style="font-size:0.8rem; color:var(--text-secondary); margin-bottom:1rem;">
        Composite: <code>success×{successWeight} + quality×{qualityWeight} + cost×{costWeight}</code>
        &nbsp;·&nbsp; total = {(successWeight + qualityWeight + costWeight).toFixed(1)}
      </p>
      <div style="display:flex; flex-direction:column; gap:0.75rem;">
        {#each [
          { label: 'success', value: successWeight, color: '#22c55e' },
          { label: 'quality', value: qualityWeight, color: '#6366f1' },
          { label: 'cost',    value: costWeight,    color: '#f59e0b' },
        ] as w}
          <div>
            <div style="display:flex; justify-content:space-between; font-size:0.82rem; margin-bottom:0.25rem;">
              <span style="color:var(--text-secondary);">{w.label}</span>
              <span style="font-family:monospace;">{w.value.toFixed(2)}</span>
            </div>
            <div style="height:8px; background:var(--surface); border-radius:4px; overflow:hidden;">
              <div style="height:100%; width:{Math.round(w.value * 100)}%; background:{w.color}; border-radius:4px; transition:width 0.4s;"></div>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <!-- 2. Evolution Tuning constants -->
    <div class="card" style="margin-bottom:1rem;">
      <h3>{$tStore('evolutionTuningTitle')}</h3>
      <table class="data-table">
        <thead>
          <tr>
            <th>{$tStore('colConstant')}</th>
            <th>{$tStore('colValue')}</th>
            <th>{$tStore('colDescription')}</th>
          </tr>
        </thead>
        <tbody>
          {#each EVOLUTION_CONSTANTS as c}
            <tr>
              <td style="font-family:monospace; font-size:0.8rem;">{c.name}</td>
              <td style="font-family:monospace; text-align:right;">{c.value}</td>
              <td style="color:var(--text-secondary); font-size:0.82rem;">{c.desc}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- 3. System Info -->
    <div class="card" style="margin-bottom:1rem;">
      <h3>{$tStore('systemInfoTitle')}</h3>
      <table class="data-table">
        <tbody>
          <tr><td style="color:var(--text-secondary)">HARNESS_DIR</td><td style="font-family:monospace; font-size:0.8rem;">~/.harness/projects/{'{slug}'}</td></tr>
          <tr><td style="color:var(--text-secondary)">{$tStore('labelSessionsAnalyzed')}</td><td>{metrics?.session_count ?? '—'}</td></tr>
          <tr>
            <td style="color:var(--text-secondary)">{$tStore('labelCurrentTrend')}</td>
            <td>
              <span style="color:{metrics?.trend === 'improving' ? 'var(--success, #22c55e)' : metrics?.trend === 'declining' ? 'var(--danger, #ef4444)' : 'var(--text-secondary)'};">
                {metrics?.trend ?? '—'}
              </span>
            </td>
          </tr>
          <tr><td style="color:var(--text-secondary)">{$tStore('labelStagnationCount')}</td><td>{metrics?.stagnation_count ?? '—'}</td></tr>
          {#if metrics?.last_error_context}
            <tr>
              <td style="color:var(--text-secondary)">{$tStore('labelLastErrorContext')}</td>
              <td style="font-family:monospace;font-size:0.8rem;color:var(--danger, #ef4444);max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;"
                  title={metrics.last_error_context}>{metrics.last_error_context}</td>
            </tr>
          {/if}
          <tr><td style="color:var(--text-secondary)">{$tStore('labelVersion')}</td><td>{__APP_VERSION__}</td></tr>
        </tbody>
      </table>
    </div>

    <!-- R4: Session Snapshots -->
    {#if snapshots.length > 0}
      <div class="card" style="margin-bottom:1rem;">
        <h3>{$tStore('sessionSnapshotsTitle')}</h3>
        <table class="data-table">
          <thead>
            <tr>
              <th>{$tStore('colDate')}</th>
              <th>{$tStore('colType')}</th>
              <th>{$tStore('colSummary')}</th>
              <th style="text-align:right;">{$tStore('colContextUsage')}</th>
            </tr>
          </thead>
          <tbody>
            {#each snapshots.slice(0, 10) as snap}
              <tr>
                <td style="font-family:monospace;font-size:0.8rem;">{dateOnly(snap.timestamp)}</td>
                <td><span class="pill info" style="font-size:10px;">{snap.type}</span></td>
                <td style="color:var(--text-secondary);font-size:0.82rem;max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;"
                    title={snap.summary}>{snap.summary || '—'}</td>
                <td style="font-family:monospace;text-align:right;">
                  {#if snap.context_usage != null}
                    <span style="color:{snap.context_usage > 0.8 ? 'var(--danger, #ef4444)' : snap.context_usage > 0.5 ? 'var(--warning)' : 'var(--success, #22c55e)'};">
                      {(snap.context_usage * 100).toFixed(0)}%
                    </span>
                  {:else}—{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    <!-- R5: Global Patterns -->
    {#if patterns.length > 0}
      <div class="card" style="margin-bottom:1rem;">
        <h3>{$tStore('globalPatternsTitle')}</h3>
        <table class="data-table">
          <thead>
            <tr>
              <th>{$tStore('colDate')}</th>
              <th>{$tStore('colProject')}</th>
              <th>{$tStore('colSuccessRate')}</th>
              <th>{$tStore('colWeakTools')}</th>
            </tr>
          </thead>
          <tbody>
            {#each patterns.slice(0, 10) as p}
              <tr>
                <td style="font-family:monospace;font-size:0.8rem;">{dateOnly(p.timestamp)}</td>
                <td><code style="font-size:0.8rem;">{p.project}</code></td>
                <td>
                  <span class="pill {p.success_rate >= 0.8 ? 'success' : p.success_rate >= 0.5 ? 'warning' : 'danger'}">
                    {(p.success_rate * 100).toFixed(0)}%
                  </span>
                </td>
                <td style="font-size:0.82rem;color:var(--text-secondary);">
                  {#if p.weak_tools && p.weak_tools.length > 0}
                    {#each p.weak_tools as tool, i}
                      <code>{tool}</code>{#if i < p.weak_tools.length - 1}, {/if}
                    {/each}
                  {:else}—{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    <!-- 4. Danger Zone -->
    <div class="card" style="border-color:var(--danger, #ef4444); border-width:1px; border-style:solid;">
      <h3 style="color:var(--danger, #ef4444);">{$tStore('dangerZoneTitle')}</h3>
      <div style="display:flex; flex-direction:column; gap:0.75rem; margin-top:0.75rem;">
        <div style="display:flex; align-items:center; justify-content:space-between; gap:1rem;">
          <div>
            <div style="font-size:0.88rem; font-weight:500;">{$tStore('resetEvolutionLabel')}</div>
            <div style="font-size:0.78rem; color:var(--text-secondary);">{$tStore('resetEvolutionDesc')}</div>
          </div>
          <button disabled style="padding:0.35rem 0.9rem; border-radius:4px; border:1px solid var(--border); background:var(--surface); color:var(--text-secondary); cursor:not-allowed; font-size:0.82rem;">
            {$tStore('resetEvolutionLabel')}
          </button>
        </div>
        <div style="display:flex; align-items:center; justify-content:space-between; gap:1rem;">
          <div>
            <div style="font-size:0.88rem; font-weight:500;">{$tStore('clearMetricsLabel')}</div>
            <div style="font-size:0.78rem; color:var(--text-secondary);">{$tStore('clearMetricsDesc')}</div>
          </div>
          <button disabled style="padding:0.35rem 0.9rem; border-radius:4px; border:1px solid var(--border); background:var(--surface); color:var(--text-secondary); cursor:not-allowed; font-size:0.82rem;">
            {$tStore('clearMetricsLabel')}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
