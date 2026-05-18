<script lang="ts">
  import { onMount } from 'svelte';
  import { getEvolvedSkills } from '../lib/harness.js';
  import type { EvolutionData } from '../lib/harness.js';

  const SEEDING_THRESHOLDS = [
    { type: 'Weak tool', threshold: 'success_rate < 0.6, min 5 observations' },
    { type: 'Weak file type', threshold: 'success_rate < 0.5, min 3 observations' },
    { type: 'High-freq error', threshold: '5+ occurrences' },
    { type: 'Stagnation rollback', threshold: '3 sessions without 5% improvement' },
  ] as const;

  let data = $state<EvolutionData | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  async function load() {
    try {
      data = await getEvolvedSkills();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load evolution data';
    } finally {
      loading = false;
    }
  }

  function dateOnly(ts: string | null | undefined): string {
    if (!ts) return '—';
    return ts.slice(0, 10);
  }

  function patternsText(patterns: unknown): string {
    if (Array.isArray(patterns)) return patterns.join(', ');
    if (typeof patterns === 'string') return patterns;
    return '—';
  }

  let pollInterval: ReturnType<typeof setInterval>;

  onMount(() => {
    load();
    pollInterval = setInterval(load, 30_000);
    return () => clearInterval(pollInterval);
  });
</script>

<div class="screen-header">
  <h2>Eval &amp; Evolve <span class="subtitle-tag">Ring 3</span></h2>
  <p>Observe &#8594; Analyze &#8594; Evolve &#8594; Gate &#8594; Reload self-improvement loop</p>
</div>

{#if loading}
  <div style="color:var(--muted);padding:32px 0;text-align:center;font-family:var(--font-mono);font-size:13px;">
    Loading evolution data…
  </div>
{:else if error}
  <div style="color:var(--danger);padding:16px;background:var(--danger-soft);border-radius:var(--radius);font-size:13px;">
    Error: {error}
  </div>
{:else if data}
  <!-- Stats row -->
  <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:20px;">
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);font-family:var(--font-mono);">
          {data.total_sessions_analyzed}
        </div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">Total Sessions Analyzed</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--orange);font-family:var(--font-mono);">
          {data.patterns_detected}
        </div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">Patterns Detected</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--success);font-family:var(--font-mono);">
          {data.evolved_skills.length}
        </div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">Evolved Skills Active</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--purple);font-family:var(--font-mono);">10</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">Max Skills Cap</div>
      </div>
    </div>
  </div>

  <!-- Evolved Skills -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>Evolved Skills</h3></div>
    <div class="panel-body">
      {#if data.evolved_skills.length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:16px 0;">
          진화된 스킬 없음 — 더 많은 세션이 쌓이면 자동 생성됩니다
        </div>
      {:else}
        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
          {#each data.evolved_skills as skill}
            <div class="skill-card">
              <div class="skill-name" style="color:var(--orange);">{skill.name}</div>
              <div class="skill-desc" style="margin-top:4px;">{skill.skill_md.split('\n')[0]}</div>
              <div style="margin-top:8px;font-size:11px;color:var(--muted);font-family:var(--font-mono);">
                {dateOnly(skill.created_at)}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Evolution History -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>Evolution History</h3></div>
    <div class="panel-body" style="padding:0;">
      {#if data.evolution_history.length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:24px;">
          아직 진화 히스토리가 없습니다
        </div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>Date</th>
              <th>Patterns</th>
              <th>Skills Seeded</th>
              <th>Trend</th>
              <th>Avg Score</th>
            </tr>
          </thead>
          <tbody>
            {#each data.evolution_history as row}
              <tr>
                <td style="font-family:var(--font-mono);font-size:12px;">
                  {dateOnly(row.timestamp as string)}
                </td>
                <td style="font-size:12px;color:var(--fg-secondary);">
                  {patternsText(row.patterns)}
                </td>
                <td style="font-family:var(--font-mono);">
                  {row.skills_seeded ?? '—'}
                </td>
                <td>
                  {#if row.trend === 'improving'}
                    <span class="pill success">improving</span>
                  {:else if row.trend === 'declining'}
                    <span class="pill danger">declining</span>
                  {:else}
                    <span class="pill info">{row.trend ?? '—'}</span>
                  {/if}
                </td>
                <td style="font-family:var(--font-mono);">
                  {typeof row.avg_score === 'number' ? row.avg_score.toFixed(2) : '—'}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>

  <!-- Seeding Thresholds -->
  <div class="panel">
    <div class="panel-header"><h3>Skill Seeding Thresholds</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead>
          <tr><th>Type</th><th>Threshold</th></tr>
        </thead>
        <tbody>
          {#each SEEDING_THRESHOLDS as row}
            <tr>
              <td style="color:var(--fg);">{row.type}</td>
              <td style="font-size:12px;color:var(--fg-secondary);font-family:var(--font-mono);">{row.threshold}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}
