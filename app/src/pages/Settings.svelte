<script lang="ts">
  import { onMount } from 'svelte';
  import { getHarnessMetrics, type HarnessMetrics } from '../lib/harness.js';

  let metrics = $state<HarnessMetrics | null>(null);
  let loading = $state(true);
  let error = $state('');

  const successWeight = $derived(Number(metrics?.score_weights?.success ?? 0.5));
  const qualityWeight = $derived(Number(metrics?.score_weights?.quality ?? 0.3));
  const costWeight = $derived(Number(metrics?.score_weights?.cost ?? 0.2));

  onMount(async () => {
    try {
      metrics = await getHarnessMetrics();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  const EVOLUTION_CONSTANTS = [
    { name: 'WEAK_TOOL_RATE', value: '0.6', desc: 'Min success rate before skill seeding' },
    { name: 'WEAK_TOOL_MIN_OBS', value: '5', desc: 'Min observations for weak tool detection' },
    { name: 'WEAK_EXT_RATE', value: '0.5', desc: 'File type weakness threshold' },
    { name: 'WEAK_EXT_MIN_OBS', value: '3', desc: 'Min observations for file type' },
    { name: 'HIGH_FREQ_ERROR_MIN', value: '5', desc: 'Error frequency for seeding' },
    { name: 'STAGNATION_LIMIT', value: '3', desc: 'Sessions without improvement before rollback' },
    { name: 'IMPROVEMENT_THRESHOLD', value: '5%', desc: 'Required improvement to reset stagnation' },
    { name: 'MAX_EVOLVED_SKILLS', value: '10', desc: 'Max evolved skills cap' },
    { name: 'REPEATED_ERROR_MIN', value: '3', desc: 'Consecutive same-error threshold' },
    { name: 'FTB_MIN_CYCLES', value: '2', desc: 'Fix-then-break cycle minimum' },
    { name: 'DEBUG_LOOP_MIN', value: '5', desc: 'Long debug loop threshold' },
  ];
</script>

<div class="page">
  <div class="page-header">
    <h1>Settings</h1>
    <p>epic-harness eval weights, evolution tuning, and system info</p>
  </div>

  {#if loading}
    <div class="loading">Loading metrics…</div>
  {:else if error}
    <div class="error-msg">{error}</div>
  {:else}
    <!-- 1. Eval Weights -->
    <div class="card" style="margin-bottom:1rem;">
      <h3>Eval Score Weights</h3>
      <p style="font-size:0.8rem; color:var(--text-secondary); margin-bottom:1rem;">
        Composite: <code>success×{successWeight} + quality×{qualityWeight} + cost×{costWeight}</code>
        &nbsp;·&nbsp; total = {(successWeight + qualityWeight + costWeight).toFixed(1)}
      </p>
      <div style="display:flex; flex-direction:column; gap:0.75rem;">
        {#each [
          { label: 'success', value: successWeight, color: '#22c55e' },
          { label: 'quality', value: qualityWeight, color: '#6366f1' },
          { label: 'cost', value: costWeight, color: '#f59e0b' },
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
      <h3>Evolution Tuning Constants</h3>
      <table class="data-table">
        <thead>
          <tr><th>Constant</th><th>Value</th><th>Description</th></tr>
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
      <h3>System Info</h3>
      <table class="data-table">
        <tbody>
          <tr><td style="color:var(--text-secondary)">HARNESS_DIR</td><td style="font-family:monospace; font-size:0.8rem;">~/.harness/projects/{'{slug}'}</td></tr>
          <tr><td style="color:var(--text-secondary)">Sessions analyzed</td><td>{metrics?.session_count ?? '—'}</td></tr>
          <tr>
            <td style="color:var(--text-secondary)">Current trend</td>
            <td>
              <span style="color:{metrics?.trend === 'improving' ? 'var(--success, #22c55e)' : metrics?.trend === 'declining' ? 'var(--danger, #ef4444)' : 'var(--text-secondary)'};">
                {metrics?.trend ?? '—'}
              </span>
            </td>
          </tr>
          <tr><td style="color:var(--text-secondary)">Stagnation count</td><td>{metrics?.stagnation_count ?? '—'}</td></tr>
          <tr><td style="color:var(--text-secondary)">Version</td><td>0.3.11</td></tr>
        </tbody>
      </table>
    </div>

    <!-- 4. Danger Zone -->
    <div class="card" style="border-color:var(--danger, #ef4444); border-width:1px; border-style:solid;">
      <h3 style="color:var(--danger, #ef4444);">Danger Zone</h3>
      <div style="display:flex; flex-direction:column; gap:0.75rem; margin-top:0.75rem;">
        <div style="display:flex; align-items:center; justify-content:space-between; gap:1rem;">
          <div>
            <div style="font-size:0.88rem; font-weight:500;">Reset Evolution</div>
            <div style="font-size:0.78rem; color:var(--text-secondary);">Removes all evolved skills</div>
          </div>
          <button disabled style="padding:0.35rem 0.9rem; border-radius:4px; border:1px solid var(--border); background:var(--surface); color:var(--text-secondary); cursor:not-allowed; font-size:0.82rem;">
            Reset Evolution
          </button>
        </div>
        <div style="display:flex; align-items:center; justify-content:space-between; gap:1rem;">
          <div>
            <div style="font-size:0.88rem; font-weight:500;">Clear Metrics</div>
            <div style="font-size:0.78rem; color:var(--text-secondary);">Resets score history</div>
          </div>
          <button disabled style="padding:0.35rem 0.9rem; border-radius:4px; border:1px solid var(--border); background:var(--surface); color:var(--text-secondary); cursor:not-allowed; font-size:0.82rem;">
            Clear Metrics
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
