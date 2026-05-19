<script lang="ts">
  import { onMount } from 'svelte';
  import { getEvolvedSkills } from '../lib/harness.js';
  import type { EvolutionData } from '../lib/harness.js';
  import DateRangePicker from '$lib/components/DateRangePicker.svelte';
  import { tStore } from '$lib/i18n.js';

  const SEEDING_THRESHOLDS = [
    { typeKey: 'seedTypeWeakTool',       threshKey: 'seedThreshWeakTool' },
    { typeKey: 'seedTypeWeakFileType',   threshKey: 'seedThreshWeakExt' },
    { typeKey: 'seedTypeHighFreqError',  threshKey: 'seedThreshHighFreq' },
    { typeKey: 'seedTypeStagnationRollback', threshKey: 'seedThreshStagnation' },
  ] as const;

  let data  = $state<EvolutionData | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  type RawRow = Record<string, unknown>;

  function deriveTrend(rows: RawRow[], idx: number): 'improving' | 'declining' | 'stable' {
    if (idx === 0) return 'stable';
    const prev = rows[idx - 1]['avg_score'] as number ?? 0;
    const cur  = rows[idx]['avg_score']  as number ?? 0;
    const delta = cur - prev;
    if (delta > 0.01) return 'improving';
    if (delta < -0.01) return 'declining';
    return 'stable';
  }

  function patternSummary(row: RawRow): string {
    const fp = row['failure_patterns'];
    if (Array.isArray(fp) && fp.length > 0) return fp.join(', ');
    const ep = row['error_patterns'];
    if (ep && typeof ep === 'object' && Object.keys(ep).length > 0)
      return Object.keys(ep).join(', ');
    const summary = row['analysis_summary'] as string ?? '';
    return summary || '—';
  }

  const enriched = $derived((): RawRow[] => {
    if (!data) return [];
    return data.evolution_history.map((r, i) => ({
      ...r,
      _trend: deriveTrend(data!.evolution_history as RawRow[], i),
      _patterns: patternSummary(r as RawRow),
    }));
  });

  // ── filters
  let filterDateFrom = $state('');
  let filterDateTo   = $state('');
  let filterTrend   = $state('');
  let filterPattern = $state('');

  const filteredHistory = $derived((): RawRow[] => {
    let list = enriched();
    if (filterDateFrom)
      list = list.filter(r => (r['timestamp'] as string ?? '').slice(0, 10) >= filterDateFrom);
    if (filterDateTo)
      list = list.filter(r => (r['timestamp'] as string ?? '').slice(0, 10) <= filterDateTo);
    if (filterTrend)
      list = list.filter(r => r['_trend'] === filterTrend);
    if (filterPattern.trim()) {
      const q = filterPattern.trim().toLowerCase();
      list = list.filter(r => (r['_patterns'] as string).toLowerCase().includes(q));
    }
    return list;
  });

  // ── pagination
  const PAGE_SIZE = 10;
  let histPage = $state(1);
  const histTotalPages = $derived(Math.max(1, Math.ceil(filteredHistory().length / PAGE_SIZE)));
  const histPageItems  = $derived(filteredHistory().slice((histPage - 1) * PAGE_SIZE, histPage * PAGE_SIZE));

  $effect(() => { filterDateFrom; filterDateTo; filterTrend; filterPattern; histPage = 1; });

  const hasFilter = $derived(!!(filterDateFrom || filterDateTo || filterTrend || filterPattern.trim()));
  function clearFilters() { filterDateFrom = ''; filterDateTo = ''; filterTrend = ''; filterPattern = ''; }

  const totalPatterns = $derived(
    (data?.evolution_history ?? []).reduce((acc, r) => {
      const fp = (r as RawRow)['failure_patterns'];
      return acc + (Array.isArray(fp) ? fp.length : 0);
    }, 0)
  );

  async function load() {
    try {
      data = await getEvolvedSkills();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function dateOnly(ts: unknown): string {
    if (!ts || typeof ts !== 'string') return '—';
    return ts.slice(0, 10);
  }

  let pollInterval: ReturnType<typeof setInterval>;
  onMount(() => {
    load();
    pollInterval = setInterval(load, 30_000);
    return () => clearInterval(pollInterval);
  });
</script>

<div class="screen-header">
  <h2>{$tStore('pageEvolve')} <span class="subtitle-tag">Ring 3</span></h2>
  <p>{$tStore('pageEvolveDesc')}</p>
</div>

{#if loading}
  <div style="color:var(--muted);padding:32px 0;text-align:center;font-family:var(--font-mono);font-size:13px;">
    {$tStore('loadingEvolution')}
  </div>
{:else if error}
  <div style="color:var(--danger);padding:16px;background:var(--danger-soft);border-radius:var(--radius);font-size:13px;">
    {$tStore('errorPrefix')}: {error}
  </div>
{:else if data}

  <!-- Stats -->
  <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:20px;">
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);font-family:var(--font-mono);">{data.total_sessions_analyzed}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statSessionsAnalyzed')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--orange);font-family:var(--font-mono);">{totalPatterns}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statFailurePatterns')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--success);font-family:var(--font-mono);">{data.evolved_skills.length}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statEvolvedSkills')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--purple);font-family:var(--font-mono);">10</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statMaxSkillsCap')}</div>
      </div>
    </div>
  </div>

  <!-- Evolved Skills -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('evolvedSkillsTitle')}</h3></div>
    <div class="panel-body">
      {#if data.evolved_skills.length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:16px 0;">
          {$tStore('noEvolvedSkills')}
        </div>
      {:else}
        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
          {#each data.evolved_skills as skill}
            <div class="skill-card">
              <div class="skill-name" style="color:var(--orange);">{skill.name}</div>
              <div class="skill-desc" style="margin-top:4px;">{skill.skill_md.split('\n')[0].replace(/^#\s*/, '')}</div>
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
    <div class="panel-header">
      <h3>{$tStore('evolutionHistoryTitle')}
        <span style="font-size:11px;font-weight:400;color:var(--muted);margin-left:8px;">
          {$tStore('filterCountFmt', filteredHistory().length, data.evolution_history.length)}
        </span>
      </h3>
    </div>

    <!-- Filter bar -->
    <div style="padding:10px 16px;border-bottom:1px solid var(--border);display:flex;gap:8px;flex-wrap:wrap;align-items:center;">
      <DateRangePicker
        from={filterDateFrom}
        to={filterDateTo}
        onchange={(f, t) => { filterDateFrom = f; filterDateTo = t; }}
      />
      <select
        bind:value={filterTrend}
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--fg);font-size:12px;outline:none;"
      >
        <option value="">{$tStore('allTrend')}</option>
        <option value="improving">{$tStore('trendImproving')}</option>
        <option value="stable">{$tStore('trendStable')}</option>
        <option value="declining">{$tStore('trendDeclining')}</option>
      </select>
      <input
        type="text"
        placeholder={$tStore('patternSearch')}
        bind:value={filterPattern}
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--fg);font-size:12px;width:200px;outline:none;"
      />
      {#if hasFilter}
        <button
          onclick={clearFilters}
          style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--muted);font-size:12px;cursor:pointer;"
        >{$tStore('reset')}</button>
      {/if}
    </div>

    <div class="panel-body" style="padding:0;">
      {#if filteredHistory().length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:24px;">
          {hasFilter ? $tStore('noHistoryFilter') : $tStore('noHistoryEmpty')}
        </div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>{$tStore('colDate')}</th>
              <th>{$tStore('colSummaryPatterns')}</th>
              <th style="text-align:right;">{$tStore('colObs')}</th>
              <th style="text-align:right;">{$tStore('colSkills')}</th>
              <th>{$tStore('colTrend')}</th>
              <th style="text-align:right;">{$tStore('colAvgScore')}</th>
            </tr>
          </thead>
          <tbody>
            {#each histPageItems as row}
              <tr>
                <td style="font-family:var(--font-mono);font-size:12px;white-space:nowrap;">
                  {dateOnly(row['timestamp'])}
                </td>
                <td style="font-size:12px;color:var(--fg-secondary);max-width:220px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;"
                    title={row['_patterns'] as string}>
                  {row['_patterns']}
                </td>
                <td style="font-family:var(--font-mono);font-size:12px;text-align:right;color:var(--muted);">
                  {row['observations'] ?? '—'}
                </td>
                <td style="font-family:var(--font-mono);text-align:right;">
                  {row['skills_seeded'] ?? 0}
                </td>
                <td>
                  {#if row['_trend'] === 'improving'}
                    <span class="pill success">{$tStore('trendImproving')}</span>
                  {:else if row['_trend'] === 'declining'}
                    <span class="pill danger">{$tStore('trendDeclining')}</span>
                  {:else}
                    <span class="pill info">{$tStore('trendStable')}</span>
                  {/if}
                </td>
                <td style="font-family:var(--font-mono);text-align:right;">
                  {typeof row['avg_score'] === 'number' ? (row['avg_score'] as number).toFixed(3) : '—'}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        <!-- Pagination -->
        {#if histTotalPages > 1}
          <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-top:1px solid var(--border);">
            <span style="font-size:12px;color:var(--muted);">
              {(histPage-1)*PAGE_SIZE+1}–{Math.min(histPage*PAGE_SIZE, filteredHistory().length)} / {filteredHistory().length}
            </span>
            <div style="display:flex;gap:4px;">
              <button onclick={() => histPage = 1} disabled={histPage === 1}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===1?'default':'pointer'};">«</button>
              <button onclick={() => histPage--} disabled={histPage === 1}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===1?'default':'pointer'};">‹</button>
              {#each Array.from({length: histTotalPages}, (_, i) => i + 1).filter(n => Math.abs(n - histPage) <= 2) as n}
                <button onclick={() => histPage = n}
                  style="padding:3px 8px;border:1px solid {n===histPage?'var(--accent)':'var(--border)'};border-radius:var(--radius-sm);background:{n===histPage?'var(--accent-soft)':'var(--bg)'};color:{n===histPage?'var(--accent)':'var(--fg)'};font-size:12px;cursor:pointer;font-weight:{n===histPage?'600':'400'};">{n}</button>
              {/each}
              <button onclick={() => histPage++} disabled={histPage === histTotalPages}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===histTotalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===histTotalPages?'default':'pointer'};">›</button>
              <button onclick={() => histPage = histTotalPages} disabled={histPage === histTotalPages}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===histTotalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===histTotalPages?'default':'pointer'};">»</button>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>

  <!-- Seeding Thresholds -->
  <div class="panel">
    <div class="panel-header"><h3>{$tStore('seedingThresholdsTitle')}</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>{$tStore('colType')}</th><th>{$tStore('colThreshold')}</th></tr></thead>
        <tbody>
          {#each SEEDING_THRESHOLDS as row}
            <tr>
              <td style="color:var(--fg);">{$tStore(row.typeKey)}</td>
              <td style="font-size:12px;color:var(--fg-secondary);font-family:var(--font-mono);">{$tStore(row.threshKey)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}
